use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;

/// Battle.net games appear as normal uninstall entries whose UninstallString
/// points at "Battle.net.exe --uid=...". Best-effort, like the original tool.
pub fn scan() -> Result<Vec<ScannedGame>> {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        use winreg::enums::*;
        use winreg::RegKey;

        let mut games = vec![];
        for path in [
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path) else {
                continue;
            };
            for id in root.enum_keys().filter_map(|k| k.ok()) {
                let Ok(key) = root.open_subkey(&id) else { continue };
                let uninstall: String =
                    key.get_value("UninstallString").unwrap_or_default();
                if !uninstall.to_ascii_lowercase().contains("battle.net") {
                    continue;
                }
                let name: String = match key.get_value("DisplayName") {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                if name.to_ascii_lowercase().contains("battle.net") {
                    continue; // the launcher itself
                }
                let Ok(loc) = key.get_value::<String, _>("InstallLocation") else {
                    continue;
                };
                let dir = PathBuf::from(loc);
                if dir.exists() {
                    games.push(ScannedGame {
                        name,
                        platform: Platform::BattleNet,
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
