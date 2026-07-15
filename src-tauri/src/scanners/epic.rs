use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct EpicManifest {
    #[serde(rename = "DisplayName")]
    display_name: Option<String>,
    #[serde(rename = "InstallLocation")]
    install_location: Option<String>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
}

fn manifests_dir() -> PathBuf {
    // Epic writes one JSON ".item" file per installed game here.
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(program_data).join(r"Epic\EpicGamesLauncher\Data\Manifests")
}

pub fn scan() -> Result<Vec<ScannedGame>> {
    let dir = manifests_dir();
    let mut games = vec![];
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(games);
    };
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<EpicManifest>(&text) else {
            continue;
        };
        if manifest.incomplete {
            continue;
        }
        if let (Some(name), Some(loc)) = (manifest.display_name, manifest.install_location) {
            let dir = PathBuf::from(loc);
            if dir.exists() {
                games.push(ScannedGame {
                    name,
                    platform: Platform::Epic,
                    install_dir: dir,
                    steam_appid: None,
                });
            }
        }
    }
    Ok(games)
}
