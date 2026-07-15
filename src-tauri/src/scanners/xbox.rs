use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;
use std::path::PathBuf;

/// The Xbox app drops a UTF-16LE ".GamingRoot" file at the root of every drive
/// it installs games to; it contains the relative library folder (usually
/// "XboxGames"). Each subfolder with a "Content" directory is a game, and the
/// swappable DLLs live inside Content.
pub fn scan() -> Result<Vec<ScannedGame>> {
    let mut games = vec![];
    for drive in b'A'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", drive as char));
        let marker = root.join(".GamingRoot");
        let Ok(bytes) = std::fs::read(&marker) else {
            continue;
        };
        let Some(folder) = parse_gaming_root(&bytes) else {
            continue;
        };
        let library = root.join(folder);
        let Ok(entries) = std::fs::read_dir(&library) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let content = entry.path().join("Content");
            if content.exists() {
                games.push(ScannedGame {
                    name: entry.file_name().to_string_lossy().to_string(),
                    platform: Platform::Xbox,
                    install_dir: content,
                    steam_appid: None,
                });
            }
        }
    }
    Ok(games)
}

/// Format: "RGBX" magic (4 bytes) + flags (4 bytes) + UTF-16LE path, NUL-terminated.
fn parse_gaming_root(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 10 || &bytes[0..4] != b"RGBX" {
        return None;
    }
    let utf16: Vec<u16> = bytes[8..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    let s = String::from_utf16(&utf16).ok()?;
    (!s.is_empty()).then_some(s)
}
