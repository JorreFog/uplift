use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;

/// GOG's offline installers and Galaxy both register games under
/// HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<productId>.
pub fn scan() -> Result<Vec<ScannedGame>> {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        use winreg::enums::*;
        use winreg::RegKey;

        let mut games = vec![];
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\Games")
        else {
            return Ok(games);
        };
        for id in root.enum_keys().filter_map(|k| k.ok()) {
            let Ok(key) = root.open_subkey(&id) else { continue };
            let name: Option<String> = key.get_value("gameName").ok();
            let path: Option<String> = key
                .get_value::<String, _>("path")
                .or_else(|_| key.get_value("exePath").map(|p: String| {
                    PathBuf::from(p)
                        .parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default()
                }))
                .ok();
            if let (Some(name), Some(path)) = (name, path) {
                let dir = PathBuf::from(path);
                if dir.exists() {
                    games.push(ScannedGame {
                        name,
                        platform: Platform::Gog,
                        install_dir: dir,
                        steam_appid: None,
                    });
                }
            }
        }
        Ok(games)
    }
    #[cfg(not(windows))]
    Ok(vec![])
}
