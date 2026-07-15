pub mod battlenet;
pub mod epic;
pub mod gog;
pub mod steam;
pub mod ubisoft;
pub mod xbox;

use crate::models::Platform;
use std::path::PathBuf;

/// A game found on disk by a launcher scanner, before DLL discovery.
#[derive(Debug, Clone)]
pub struct ScannedGame {
    pub name: String,
    pub platform: Platform,
    pub install_dir: PathBuf,
    pub steam_appid: Option<u32>,
}

/// Run every scanner, tolerating individual failures.
pub fn scan_all() -> Vec<ScannedGame> {
    let mut out = vec![];
    for result in [
        steam::scan(),
        epic::scan(),
        gog::scan(),
        ubisoft::scan(),
        battlenet::scan(),
        xbox::scan(),
    ] {
        match result {
            Ok(mut games) => out.append(&mut games),
            Err(e) => eprintln!("scanner error (continuing): {e:#}"),
        }
    }
    // Dedupe by install dir (a game can be registered by more than one source).
    out.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    out.dedup_by(|a, b| a.install_dir == b.install_dir);
    out
}
