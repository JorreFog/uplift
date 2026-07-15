use super::ScannedGame;
use crate::models::Platform;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Minimal parser for Valve's text KeyValues (VDF/ACF) format.
/// Handles quoted keys/values, nested braces, escaped quotes and comments —
/// which is all libraryfolders.vdf and appmanifest files use.
#[derive(Debug, Default)]
pub struct Vdf {
    pub values: HashMap<String, String>,
    pub children: HashMap<String, Vdf>,
}

impl Vdf {
    pub fn parse(text: &str) -> Vdf {
        let mut chars = text.chars().peekable();
        // The file is `"root" { ... }`; parse tokens and descend.
        let mut root = Vdf::default();
        Self::parse_block(&mut chars, &mut root);
        // If the file had a single named root block, unwrap it for convenience.
        if root.values.is_empty() && root.children.len() == 1 {
            return root.children.into_values().next().unwrap();
        }
        root
    }

    fn parse_block(chars: &mut std::iter::Peekable<std::str::Chars>, node: &mut Vdf) {
        loop {
            Self::skip_ws_and_comments(chars);
            match chars.peek() {
                None | Some('}') => {
                    chars.next();
                    return;
                }
                Some('"') => {
                    let key = Self::read_string(chars);
                    Self::skip_ws_and_comments(chars);
                    match chars.peek() {
                        Some('"') => {
                            let value = Self::read_string(chars);
                            node.values.insert(key.to_ascii_lowercase(), value);
                        }
                        Some('{') => {
                            chars.next();
                            let mut child = Vdf::default();
                            Self::parse_block(chars, &mut child);
                            node.children.insert(key.to_ascii_lowercase(), child);
                        }
                        _ => return,
                    }
                }
                Some('{') => {
                    // Anonymous block — descend into current node.
                    chars.next();
                    Self::parse_block(chars, node);
                }
                _ => {
                    chars.next();
                }
            }
        }
    }

    fn skip_ws_and_comments(chars: &mut std::iter::Peekable<std::str::Chars>) {
        loop {
            while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
                chars.next();
            }
            if chars.peek() == Some(&'/') {
                // Line comment: consume to end of line.
                while !matches!(chars.peek(), None | Some('\n')) {
                    chars.next();
                }
            } else {
                return;
            }
        }
    }

    fn read_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut s = String::new();
        chars.next(); // opening quote
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(esc) = chars.next() {
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    }
                }
                '"' => break,
                other => s.push(other),
            }
        }
        s
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }
}

fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        for (hive, path) in [
            (HKEY_CURRENT_USER, r"Software\Valve\Steam"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam"),
        ] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(path) {
                for value in ["SteamPath", "InstallPath"] {
                    if let Ok(p) = key.get_value::<String, _>(value) {
                        let pb = PathBuf::from(p);
                        if pb.exists() {
                            return Some(pb);
                        }
                    }
                }
            }
        }
        let fallback = PathBuf::from(r"C:\Program Files (x86)\Steam");
        fallback.exists().then_some(fallback)
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().map(|h| h.join(".steam/steam")).filter(|p| p.exists())
    }
}

/// Resolve to the on-disk path (real casing, backslashes) so the same library
/// reached via the registry (`c:/program files (x86)/steam`) and via
/// libraryfolders.vdf (`C:\Program Files (x86)\Steam`) compares equal.
/// Returns None if the path does not exist.
fn canonical(p: PathBuf) -> Option<PathBuf> {
    let c = std::fs::canonicalize(&p).ok()?;
    let s = c.to_string_lossy();
    Some(PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s)))
}

pub fn scan() -> Result<Vec<ScannedGame>> {
    let Some(root) = steam_root() else {
        return Ok(vec![]);
    };
    let lf = root.join("steamapps").join("libraryfolders.vdf");
    let mut library_paths = Vec::new();
    if let Some(steamapps) = canonical(root.join("steamapps")) {
        library_paths.push(steamapps);
    }
    if let Ok(text) = std::fs::read_to_string(&lf) {
        let vdf = Vdf::parse(&text);
        for (_, folder) in &vdf.children {
            if let Some(p) = folder.get("path") {
                if let Some(steamapps) = canonical(Path::new(p).join("steamapps")) {
                    if !library_paths.contains(&steamapps) {
                        library_paths.push(steamapps);
                    }
                }
            }
        }
    }

    let mut games = vec![];
    for steamapps in library_paths {
        let Ok(entries) = std::fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let acf = Vdf::parse(&text);
            let (Some(app_name), Some(installdir)) = (acf.get("name"), acf.get("installdir"))
            else {
                continue;
            };
            let appid = acf.get("appid").and_then(|a| a.parse::<u32>().ok());
            // Skip runtimes/redists Steam registers as apps.
            let lower = app_name.to_ascii_lowercase();
            if lower.contains("steamworks common")
                || lower.contains("redistributable")
                || lower.starts_with("proton")
                || lower.starts_with("steam linux runtime")
            {
                continue;
            }
            let dir = steamapps.join("common").join(installdir);
            if dir.exists() {
                games.push(ScannedGame {
                    name: app_name.to_string(),
                    platform: Platform::Steam,
                    install_dir: dir,
                    steam_appid: appid,
                });
            }
        }
    }
    Ok(games)
}
