use crate::db::Db;
use crate::models::*;
use crate::{downloads, remote, swap};
use std::cmp::Ordering;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

fn family_label(f: Family) -> &'static str {
    match f {
        Family::Dlss => "DLSS",
        Family::DlssG => "DLSS Frame Generation",
        Family::DlssD => "DLSS Ray Reconstruction",
        Family::Xess => "Intel XeSS",
        Family::XessFg => "Intel XeSS Frame Gen",
        Family::Fsr => "AMD FSR",
    }
}

/// One full cycle: refresh remote data, notify about new releases,
/// then run the auto-update pass. Returns a human-readable summary.
pub async fn run_cycle(app: &AppHandle) -> String {
    let db = app.state::<Db>();
    let settings = db.get_settings().unwrap_or_default();

    // Steam replaces capsule art in place (seasonal art etc.), so re-resolve
    // covers every cycle. Runs before the manifest refresh, which bails on
    // failure and would otherwise skip this while the manifest is unreachable.
    let _ = remote::refresh_covers(&db).await;

    // Defend user-chosen versions against game updates (works offline too,
    // as long as the chosen build is already in the local library).
    for msg in reapply_pass(&db).await {
        notify(app, "Version re-applied", &msg);
    }

    let outcome = match remote::refresh(&db).await {
        Ok(o) => o,
        Err(e) => return format!("refresh failed: {e:#}"),
    };

    // --- notifications for brand-new releases -------------------------------
    if settings.notify_on_new_release {
        let games = db.get_games().unwrap_or_default();
        for rel in &outcome.new_releases {
            let eligible = games
                .iter()
                .filter(|g| {
                    g.dlls.iter().any(|d| {
                        d.family == rel.family
                            && version_cmp(&rel.version, &d.version) == Ordering::Greater
                    })
                })
                .count();
            let body = if eligible > 0 {
                format!(
                    "{} {} is out — {} of your games can be upgraded.",
                    family_label(rel.family),
                    rel.version,
                    eligible
                )
            } else {
                format!("{} {} is out.", family_label(rel.family), rel.version)
            };
            notify(app, "New DLL release", &body);
            let _ = db.mark_notified(rel.family, &rel.version);
        }
    }

    // --- auto-update pass -----------------------------------------------------
    let auto_summary = auto_update_pass(app).await;

    // Let an open window refresh its data.
    let _ = app.emit("uplift://refreshed", ());

    match (outcome.new_releases.len(), auto_summary.as_str()) {
        (0, "") => "up to date".into(),
        (n, "") => format!("{n} new release(s)"),
        (n, s) => format!("{n} new release(s); {s}"),
    }
}

/// Re-apply user-chosen DLL versions that a game update silently reverted.
/// Steam and friends restore stock DLLs on every patch; this pass reads the
/// real on-disk versions of every family with a remembered choice and swaps
/// the chosen build back in. Guards: per-game `reapply` pref, anti-cheat
/// flag, game not running, filename match, verified download.
pub async fn reapply_pass(db: &Db) -> Vec<String> {
    let mut messages = vec![];
    let desired = db.get_desired().unwrap_or_default();
    if desired.is_empty() {
        return messages;
    }
    let games = db.get_games().unwrap_or_default();
    let anticheat = remote::cached_anticheat(db);

    for (game_id, family, version) in desired {
        let Some(game) = games.iter().find(|g| g.id == game_id) else {
            continue;
        };
        if !game.prefs.reapply {
            continue;
        }
        if remote::anticheat_for(&anticheat, &game.name, game.steam_appid).is_some() {
            continue;
        }
        // Compare the actual PE versions on disk — the DB may be stale.
        let reverted: Vec<_> = game
            .dlls
            .iter()
            .filter(|d| d.family == family)
            .filter(|d| {
                crate::dll::read_file_version(Path::new(&d.path))
                    .map(|v| v != version)
                    .unwrap_or(false)
            })
            .collect();
        if reverted.is_empty() {
            continue;
        }
        if swap::game_is_running(Path::new(&game.install_dir)) {
            continue;
        }
        let Ok(source) = downloads::download_release(db, family, &version).await else {
            continue;
        };
        let source_name = Path::new(&source)
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let mut applied = false;
        for dll in reverted
            .iter()
            .filter(|d| d.file_name.to_ascii_lowercase() == source_name)
        {
            let plan = swap::SwapPlan {
                db,
                game_id,
                game_install_dir: Path::new(&game.install_dir),
                family,
                dll_path: Path::new(&dll.path),
                source_path: Path::new(&source),
                to_version: &version,
                automatic: true,
            };
            if swap::swap(plan).is_ok() {
                let _ = db.update_dll_version(game_id, &dll.path, &version);
                applied = true;
            }
        }
        if applied {
            let _ = db.start_crashwatch(game_id);
            messages.push(format!(
                "{} reverted {} by a game update — re-applied {}",
                game.name,
                family_label(family),
                version
            ));
        }
    }
    messages
}

/// Swap every eligible DLL of every game that opted in. Guards, in order:
/// per-game opt-in, version pin, anti-cheat flag, game not running,
/// verified download. Anything failing a guard is skipped silently —
/// auto-update must never nag.
async fn auto_update_pass(app: &AppHandle) -> String {
    let db = app.state::<Db>();
    let games = match db.get_games() {
        Ok(g) => g,
        Err(_) => return String::new(),
    };
    let releases = db.get_releases().unwrap_or_default();
    let anticheat = remote::cached_anticheat(&db);
    let mut swapped = 0u32;
    let mut swapped_names: Vec<String> = vec![];

    for game in games.iter().filter(|g| g.prefs.auto_update) {
        // Any anti-cheat flag at all disables automatic swapping.
        if remote::anticheat_for(&anticheat, &game.name, game.steam_appid).is_some() {
            continue;
        }
        for dll in &game.dlls {
            if game
                .prefs
                .pins
                .get(dll.family.as_str())
                .map(|p| !p.is_empty())
                .unwrap_or(false)
            {
                continue; // pinned — user chose this version deliberately
            }
            let Some(latest) = releases
                .iter()
                .filter(|r| r.family == dll.family)
                .max_by(|a, b| version_cmp(&a.version, &b.version))
            else {
                continue;
            };
            if version_cmp(&latest.version, &dll.version) != Ordering::Greater {
                continue;
            }
            let Ok(source) =
                downloads::download_release(&db, dll.family, &latest.version).await
            else {
                continue;
            };
            // Same guard as manual swaps: a release only lands on a DLL with
            // the same file name (FSR spans non-interchangeable variants).
            let source_name = Path::new(&source)
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if dll.file_name.to_ascii_lowercase() != source_name {
                continue;
            }
            let plan = swap::SwapPlan {
                db: &db,
                game_id: game.id,
                game_install_dir: Path::new(&game.install_dir),
                family: dll.family,
                dll_path: Path::new(&dll.path),
                source_path: Path::new(&source),
                to_version: &latest.version,
                automatic: true,
            };
            if swap::swap(plan).is_ok() {
                let _ = db.update_dll_version(game.id, &dll.path, &latest.version);
                let _ = db.set_desired(game.id, dll.family, &latest.version);
                let _ = db.start_crashwatch(game.id);
                swapped += 1;
                if !swapped_names.contains(&game.name) {
                    swapped_names.push(game.name.clone());
                }
            }
        }
    }

    if swapped > 0 {
        notify(
            app,
            "Auto-update complete",
            &format!(
                "Upgraded {} DLL(s) across: {}",
                swapped,
                swapped_names.join(", ")
            ),
        );
        format!("auto-updated {swapped} DLL(s)")
    } else {
        String::new()
    }
}

/// Spawn the periodic poll. Interval comes from settings; re-read every cycle
/// so changes apply without a restart.
pub fn spawn(app: AppHandle) {
    let watcher_app = app.clone();
    tauri::async_runtime::spawn(async move {
        // First check shortly after launch, then on the configured cadence.
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        loop {
            let _ = run_cycle(&app).await;
            let hours = {
                let db = app.state::<Db>();
                db.get_settings().map(|s| s.poll_hours).unwrap_or(6).max(1)
            };
            tokio::time::sleep(std::time::Duration::from_secs(u64::from(hours) * 3600)).await;
        }
    });
    tauri::async_runtime::spawn(crash_watcher(watcher_app));
}

// ---- crash detection + automatic rollback -----------------------------------

/// A session shorter than this right after a swap counts as a crash.
const CRASH_SESSION_SECS: u64 = 150;
/// This many crashes in a row trigger the rollback.
const CRASH_LIMIT: u32 = 2;
/// Process poll cadence while any game is under watch.
const WATCH_TICK_SECS: u64 = 15;

/// Watch games that just had a DLL swapped: one healthy session clears the
/// watch; `CRASH_LIMIT` short-lived sessions in a row restore every backed-up
/// DLL and forget the chosen versions so re-apply doesn't redo the damage.
async fn crash_watcher(app: AppHandle) {
    use std::collections::HashMap;
    use std::time::Instant;
    // game_id -> Some(session start) while the game is running.
    let mut sessions: HashMap<i64, Option<Instant>> = HashMap::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(WATCH_TICK_SECS)).await;
        let watches = {
            let db = app.state::<Db>();
            db.get_crashwatch().unwrap_or_default()
        };
        if watches.is_empty() {
            sessions.clear();
            continue;
        }
        let games = {
            let db = app.state::<Db>();
            db.get_games().unwrap_or_default()
        };

        for (game_id, _crashes) in watches {
            let Some(game) = games.iter().find(|g| g.id == game_id) else {
                let db = app.state::<Db>();
                let _ = db.clear_crashwatch(game_id);
                continue;
            };
            let install_dir = game.install_dir.clone();
            let running = tauri::async_runtime::spawn_blocking(move || {
                swap::game_is_running(Path::new(&install_dir))
            })
            .await
            .unwrap_or(false);

            let entry = sessions.entry(game_id).or_insert(None);
            match (*entry, running) {
                (None, true) => *entry = Some(Instant::now()),
                (Some(started), false) => {
                    *entry = None;
                    let db = app.state::<Db>();
                    if started.elapsed().as_secs() >= CRASH_SESSION_SECS {
                        // Survived long enough — the swap is healthy.
                        let _ = db.clear_crashwatch(game_id);
                    } else {
                        let crashes = db.bump_crash(game_id).unwrap_or(0);
                        if crashes >= CRASH_LIMIT {
                            rollback_game(&app, game).await;
                        } else {
                            notify(
                                &app,
                                "Short session detected",
                                &format!(
                                    "{} exited quickly after a DLL swap. One more crash and Uplift restores the original DLLs.",
                                    game.name
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Restore every backed-up DLL of the game and clear its chosen versions.
async fn rollback_game(app: &AppHandle, game: &Game) {
    let db = app.state::<Db>();
    let mut restored = 0u32;
    for dll in game.dlls.iter().filter(|d| d.has_backup) {
        if swap::restore(
            &db,
            game.id,
            Path::new(&game.install_dir),
            dll.family,
            Path::new(&dll.path),
        )
        .is_ok()
        {
            let _ = db.clear_desired(game.id, dll.family);
            if let Ok(v) = crate::dll::read_file_version(Path::new(&dll.path)) {
                let _ = db.update_dll_version(game.id, &dll.path, &v);
            }
            restored += 1;
        }
    }
    let _ = db.clear_crashwatch(game.id);
    if restored > 0 {
        notify(
            app,
            "Automatic rollback",
            &format!(
                "{} crashed twice right after a DLL swap — restored {} original DLL(s). The version rail is all yours to try something else.",
                game.name, restored
            ),
        );
        let _ = app.emit("uplift://refreshed", ());
    }
}
