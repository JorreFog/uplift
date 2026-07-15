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
}
