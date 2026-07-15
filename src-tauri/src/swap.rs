use crate::db::Db;
use crate::dll::read_file_version;
use crate::models::Family;
use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use sysinfo::System;

/// True if any running process executes from inside the game directory.
/// Swapping a DLL under a live process invites crashes and file locks.
pub fn game_is_running(install_dir: &Path) -> bool {
    let mut sys = System::new();
    sys.refresh_processes();
    let dir = install_dir.to_string_lossy().to_ascii_lowercase();
    sys.processes().values().any(|p| {
        p.exe()
            .map(|exe| exe.to_string_lossy().to_ascii_lowercase().starts_with(&dir))
            .unwrap_or(false)
    })
}

fn backup_path(dll_path: &Path) -> PathBuf {
    let name = dll_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    dll_path.with_file_name(format!("{name}.uplift.bak"))
}

pub struct SwapPlan<'a> {
    pub db: &'a Db,
    pub game_id: i64,
    pub game_install_dir: &'a Path,
    pub family: Family,
    pub dll_path: &'a Path,
    /// Path of the verified DLL in the local library.
    pub source_path: &'a Path,
    pub to_version: &'a str,
    pub automatic: bool,
}

/// Swap one DLL. Sequence:
///   1. refuse if the game is running
///   2. back up the true original (only on first swap — the backup is sacred)
///   3. copy the library DLL over the target
///   4. verify the copy landed intact
pub fn swap(plan: SwapPlan) -> Result<()> {
    if game_is_running(plan.game_install_dir) {
        bail!("The game appears to be running. Close it before swapping.");
    }
    if !plan.source_path.exists() {
        bail!("Downloaded DLL is missing from the library. Re-download it.");
    }
    if !plan.dll_path.exists() {
        bail!("Target DLL no longer exists — rescan the game.");
    }

    let from_version = read_file_version(plan.dll_path).unwrap_or_else(|_| "unknown".into());

    let backup = backup_path(plan.dll_path);
    if !backup.exists() {
        std::fs::copy(plan.dll_path, &backup)
            .map_err(|e| anyhow!("could not back up original DLL: {e}"))?;
    }

    std::fs::copy(plan.source_path, plan.dll_path)
        .map_err(|e| anyhow!("could not write new DLL (try running elevated): {e}"))?;

    let landed = read_file_version(plan.dll_path).unwrap_or_default();
    if landed != plan.to_version {
        // Best effort: put the previous file back rather than leave a mystery DLL.
        let _ = std::fs::copy(&backup, plan.dll_path);
        bail!(
            "post-swap verification failed (found version '{landed}'). Original restored."
        );
    }

    plan.db.record_swap(
        plan.game_id,
        plan.family,
        &plan.dll_path.to_string_lossy(),
        &from_version,
        plan.to_version,
        plan.automatic,
    )?;
    Ok(())
}

/// Restore the pristine original from its backup. The backup is kept so the
/// user can swap and restore as many times as they like.
pub fn restore(
    db: &Db,
    game_id: i64,
    game_install_dir: &Path,
    family: Family,
    dll_path: &Path,
) -> Result<()> {
    if game_is_running(game_install_dir) {
        bail!("The game appears to be running. Close it before restoring.");
    }
    let backup = backup_path(dll_path);
    if !backup.exists() {
        bail!("No backup exists for this DLL — nothing to restore.");
    }
    let from_version = read_file_version(dll_path).unwrap_or_else(|_| "unknown".into());
    std::fs::copy(&backup, dll_path)
        .map_err(|e| anyhow!("could not restore original DLL (try running elevated): {e}"))?;
    let to_version = read_file_version(dll_path).unwrap_or_else(|_| "unknown".into());
    db.record_swap(
        game_id,
        family,
        &dll_path.to_string_lossy(),
        &from_version,
        &to_version,
        false,
    )?;
    Ok(())
}
