use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;

/// Ubisoft Connect registers installs under
/// HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id> (InstallDir),
/// with display names under ...\Uninstall\Uplay Install <id>.
pub fn scan() -> Result<Vec<ScannedGame>> {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        use winreg::enums::*;
        use winreg::RegKey;

        let mut games = vec![];
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let Ok(installs) =
            hklm.open_subkey(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs")
        else {
            return Ok(games);
        };
        let uninstall = hklm
            .open_subkey(r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall")
            .ok();
        for id in installs.enum_keys().filter_map(|k| k.ok()) {
            let Ok(key) = installs.open_subkey(&id) else { continue };
            let Ok(dir_s) = key.get_value::<String, _>("InstallDir") else {
                continue;
            };
            let dir = PathBuf::from(dir_s.replace('/', "\\"));
            if !dir.exists() {
                continue;
            }
            // Prefer the friendly name from the uninstall entry; fall back to folder name.
            let name = uninstall
                .as_ref()
                .and_then(|u| u.open_subkey(format!("Uplay Install {id}")).ok())
                .and_then(|k| k.get_value::<String, _>("DisplayName").ok())
                .or_else(|| {
                    dir.file_name().map(|f| f.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| format!("Ubisoft game {id}"));
            games.push(ScannedGame {
                name,
                platform: Platform::Ubisoft,
                install_dir: dir,
                steam_appid: None,
            });
        }
        Ok(games)
    }
    #[cfg(not(windows))]
    Ok(vec![])
}
