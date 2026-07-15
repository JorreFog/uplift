use crate::db::Db;
use crate::models::*;
use crate::{background, dll, downloads, indicator, presets, remote, scanners, swap};
use std::path::Path;
use tauri::{AppHandle, State};

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e}")
}

/// Full library payload the frontend renders from.
#[derive(serde::Serialize)]
pub struct LibraryPayload {
    pub games: Vec<Game>,
    pub releases: Vec<Release>,
    pub changelogs: Vec<ChangelogEntry>,
    pub swaps: Vec<SwapRecord>,
}

fn build_payload(db: &Db) -> CmdResult<LibraryPayload> {
    let anticheat = remote::cached_anticheat(db);
    let mut games = db.get_games().map_err(err)?;
    for g in games.iter_mut() {
        g.anticheat = remote::anticheat_for(&anticheat, &g.name, g.steam_appid);
    }
    Ok(LibraryPayload {
        games,
        releases: db.get_releases().map_err(err)?,
        changelogs: remote::cached_changelogs(db),
        swaps: db.get_swaps(50).map_err(err)?,
    })
}

#[tauri::command]
pub fn get_library(db: State<Db>) -> CmdResult<LibraryPayload> {
    build_payload(&db)
}

/// Rescan every launcher and re-discover DLLs. Runs on a blocking thread —
/// walking game folders is filesystem-heavy.
#[tauri::command]
pub async fn scan_games(db: State<'_, Db>) -> CmdResult<LibraryPayload> {
    let scanned = tauri::async_runtime::spawn_blocking(scanners::scan_all)
        .await
        .map_err(err)?;

    let mut seen: std::collections::HashMap<Platform, Vec<String>> = Default::default();
    for game in &scanned {
        let dir = game.install_dir.to_string_lossy().to_string();
        let dlls = dll::discover_dlls(&game.install_dir);
        // Only track games that actually contain a swappable DLL — same call
        // DLSS Swapper makes, and it keeps the library signal-only.
        if dlls.is_empty() {
            continue;
        }
        let id = db
            .upsert_game(&game.name, game.platform, &dir, game.steam_appid)
            .map_err(err)?;
        db.replace_dlls(id, &dlls).map_err(err)?;
        seen.entry(game.platform).or_default().push(dir);
    }
    // Prune uninstalled games per platform (manual entries are never pruned).
    for platform in [
        Platform::Steam,
        Platform::Epic,
        Platform::Gog,
        Platform::Ubisoft,
        Platform::BattleNet,
        Platform::Xbox,
    ] {
        let dirs = seen.remove(&platform).unwrap_or_default();
        db.prune_platform(platform, &dirs).map_err(err)?;
    }
    // Box art is cosmetic: resolve current URLs if online, keep scanning if not.
    let _ = remote::refresh_covers(&db).await;
    // A rescan is when reverted DLLs become visible — put the chosen builds back.
    let _ = background::reapply_pass(&db).await;
    build_payload(&db)
}

#[tauri::command]
pub async fn add_manual_game(db: State<'_, Db>, name: String, dir: String) -> CmdResult<LibraryPayload> {
    let path = std::path::PathBuf::from(&dir);
    if !path.exists() {
        return Err("That folder does not exist.".into());
    }
    let dlls = tauri::async_runtime::spawn_blocking(move || dll::discover_dlls(&path))
        .await
        .map_err(err)?;
    if dlls.is_empty() {
        return Err("No swappable DLLs (DLSS / FSR / XeSS) were found in that folder.".into());
    }
    let id = db
        .upsert_game(&name, Platform::Manual, &dir, None)
        .map_err(err)?;
    db.replace_dlls(id, &dlls).map_err(err)?;
    build_payload(&db)
}

#[tauri::command]
pub async fn refresh_remote(app: AppHandle) -> CmdResult<String> {
    Ok(background::run_cycle(&app).await)
}

#[tauri::command]
pub async fn download_dll(db: State<'_, Db>, family: String, version: String) -> CmdResult<LibraryPayload> {
    let family = Family::from_str(&family).ok_or("unknown family")?;
    downloads::download_release(&db, family, &version)
        .await
        .map_err(err)?;
    build_payload(&db)
}

/// Manual swap from the UI. `confirmed_anticheat` must be true to proceed on a
/// game with any anti-cheat flag; "block"-severity games are always refused.
#[tauri::command]
pub async fn swap_dll(
    db: State<'_, Db>,
    game_id: i64,
    family: String,
    version: String,
    confirmed_anticheat: bool,
) -> CmdResult<LibraryPayload> {
    let family = Family::from_str(&family).ok_or("unknown family")?;
    let games = db.get_games().map_err(err)?;
    let game = games.iter().find(|g| g.id == game_id).ok_or("game not found")?;

    let anticheat = remote::cached_anticheat(&db);
    if let Some(flag) = remote::anticheat_for(&anticheat, &game.name, game.steam_appid) {
        if flag.starts_with("block:") {
            return Err(format!(
                "This game uses {} and swapping DLLs risks a ban. Uplift will not swap it.",
                flag.trim_start_matches("block:")
            ));
        }
        if !confirmed_anticheat {
            return Err(format!("anticheat_confirm:{flag}"));
        }
    }

    if !game.dlls.iter().any(|d| d.family == family) {
        return Err("this game has no DLL of that family".into());
    }
    let source = downloads::download_release(&db, family, &version)
        .await
        .map_err(err)?;
    // A release may only land on DLLs with the same file name — families like
    // FSR span DX12/Vulkan/FSR2 variants that are not interchangeable.
    let source_name = Path::new(&source)
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let targets: Vec<_> = game
        .dlls
        .iter()
        .filter(|d| d.family == family && d.file_name.to_ascii_lowercase() == source_name)
        .collect();
    if targets.is_empty() {
        let has = game
            .dlls
            .iter()
            .filter(|d| d.family == family)
            .map(|d| d.file_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "This build ships {source_name}, but the game uses {has}. Pick a matching release."
        ));
    }
    for installed in &targets {
        swap::swap(swap::SwapPlan {
            db: &db,
            game_id,
            game_install_dir: Path::new(&game.install_dir),
            family,
            dll_path: Path::new(&installed.path),
            source_path: Path::new(&source),
            to_version: &version,
            automatic: false,
        })
        .map_err(err)?;
    }

    // Remember the choice so game updates that revert it get re-applied.
    db.set_desired(game_id, family, &version).map_err(err)?;

    // Reflect the new version without a full rescan.
    let mut dlls = game.dlls.clone();
    for d in dlls.iter_mut() {
        if d.family == family && d.file_name.to_ascii_lowercase() == source_name {
            d.version = version.clone();
            d.has_backup = true;
        }
    }
    db.replace_dlls(game_id, &dlls).map_err(err)?;
    build_payload(&db)
}

#[tauri::command]
pub fn restore_dll(db: State<Db>, game_id: i64, family: String) -> CmdResult<LibraryPayload> {
    let family = Family::from_str(&family).ok_or("unknown family")?;
    let games = db.get_games().map_err(err)?;
    let game = games.iter().find(|g| g.id == game_id).ok_or("game not found")?;
    // Restore every DLL of the family that has a backup (multi-DLL families).
    let targets: Vec<_> = game
        .dlls
        .iter()
        .filter(|d| d.family == family && d.has_backup)
        .collect();
    if targets.is_empty() {
        return Err("No backup exists for this DLL — nothing to restore.".into());
    }
    for installed in &targets {
        swap::restore(
            &db,
            game_id,
            Path::new(&game.install_dir),
            family,
            Path::new(&installed.path),
        )
        .map_err(err)?;
    }
    // The user went back to the original — stop defending a chosen version.
    db.clear_desired(game_id, family).map_err(err)?;
    let mut dlls = game.dlls.clone();
    for d in dlls.iter_mut() {
        if d.family == family && d.has_backup {
            d.version = dll::read_file_version(Path::new(&d.path)).unwrap_or_else(|_| "unknown".into());
        }
    }
    db.replace_dlls(game_id, &dlls).map_err(err)?;
    build_payload(&db)
}

/// Current driver preset overrides for a game (NVAPI DRS). Runs blocking —
/// profile lookup walks the install dir for the exe.
#[tauri::command]
pub async fn get_game_presets(db: State<'_, Db>, game_id: i64) -> CmdResult<presets::GamePresets> {
    let games = db.get_games().map_err(err)?;
    let game = games.iter().find(|g| g.id == game_id).ok_or("game not found")?;
    let dir = std::path::PathBuf::from(&game.install_dir);
    tauri::async_runtime::spawn_blocking(move || presets::get_presets(&dir))
        .await
        .map_err(err)
}

/// Apply (or clear with value 0) a DLSS SR/RR preset override for a game.
#[tauri::command]
pub async fn set_game_preset(
    db: State<'_, Db>,
    game_id: i64,
    family: String,
    value: u32,
) -> CmdResult<presets::GamePresets> {
    let setting_id = match family.as_str() {
        "dlss" => presets::SR_PRESET_ID,
        "dlss_d" => presets::RR_PRESET_ID,
        _ => return Err("presets exist only for DLSS and Ray Reconstruction".into()),
    };
    let games = db.get_games().map_err(err)?;
    let game = games.iter().find(|g| g.id == game_id).ok_or("game not found")?;
    let dir = std::path::PathBuf::from(&game.install_dir);
    tauri::async_runtime::spawn_blocking(move || {
        presets::set_preset(&dir, setting_id, value)?;
        Ok::<_, anyhow::Error>(presets::get_presets(&dir))
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
pub fn get_dlss_indicator() -> bool {
    indicator::indicator_enabled()
}

/// Toggle the global NGX on-screen indicator. Triggers a UAC prompt.
#[tauri::command]
pub async fn set_dlss_indicator(enabled: bool) -> CmdResult<bool> {
    tauri::async_runtime::spawn_blocking(move || indicator::set_indicator(enabled))
        .await
        .map_err(err)?
        .map_err(err)?;
    Ok(indicator::indicator_enabled())
}

#[tauri::command]
pub fn set_game_prefs(db: State<Db>, game_id: i64, prefs: GamePrefs) -> CmdResult<()> {
    db.set_prefs(game_id, &prefs).map_err(err)
}

#[tauri::command]
pub fn get_settings(db: State<Db>) -> CmdResult<Settings> {
    db.get_settings().map_err(err)
}

#[tauri::command]
pub fn set_settings(db: State<Db>, settings: Settings) -> CmdResult<()> {
    db.set_settings(&settings).map_err(err)
}
