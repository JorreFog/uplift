use serde::{Deserialize, Serialize};

/// The DLL families Uplift manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Dlss,   // Super Resolution        nvngx_dlss.dll
    DlssG,  // Frame Generation        nvngx_dlssg.dll
    DlssD,  // Ray Reconstruction      nvngx_dlssd.dll
    Xess,   // Intel XeSS              libxess.dll
    XessFg, // Intel XeSS Frame Gen    libxess_fg.dll
    Fsr,    // AMD FidelityFX / FSR    amd_fidelityfx_*.dll, ffx_fsr2_*.dll
}

impl Family {
    pub const ALL: [Family; 6] = [
        Family::Dlss,
        Family::DlssG,
        Family::DlssD,
        Family::Xess,
        Family::XessFg,
        Family::Fsr,
    ];

    /// File names that identify this family on disk (lowercase).
    pub fn file_names(&self) -> &'static [&'static str] {
        match self {
            Family::Dlss => &["nvngx_dlss.dll"],
            Family::DlssG => &["nvngx_dlssg.dll"],
            Family::DlssD => &["nvngx_dlssd.dll"],
            Family::Xess => &["libxess.dll"],
            Family::XessFg => &["libxess_fg.dll"],
            Family::Fsr => &[
                "amd_fidelityfx_dx12.dll",
                "amd_fidelityfx_vk.dll",
                "amd_fidelityfx_upscaler_dx12.dll",
                "ffx_fsr2_api_x64.dll",
                "ffx_fsr2_api_dx12_x64.dll",
                "ffx_fsr2_api_vk_x64.dll",
            ],
        }
    }

    pub fn from_file_name(name: &str) -> Option<Family> {
        let lower = name.to_ascii_lowercase();
        Family::ALL
            .into_iter()
            .find(|f| f.file_names().contains(&lower.as_str()))
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Family::Dlss => "dlss",
            Family::DlssG => "dlss_g",
            Family::DlssD => "dlss_d",
            Family::Xess => "xess",
            Family::XessFg => "xess_fg",
            Family::Fsr => "fsr",
        }
    }

    pub fn from_str(s: &str) -> Option<Family> {
        match s {
            "dlss" => Some(Family::Dlss),
            "dlss_g" => Some(Family::DlssG),
            "dlss_d" => Some(Family::DlssD),
            "xess" => Some(Family::Xess),
            "xess_fg" => Some(Family::XessFg),
            "fsr" => Some(Family::Fsr),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Steam,
    Epic,
    Gog,
    Ubisoft,
    BattleNet,
    Xbox,
    Manual,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Steam => "steam",
            Platform::Epic => "epic",
            Platform::Gog => "gog",
            Platform::Ubisoft => "ubisoft",
            Platform::BattleNet => "battle_net",
            Platform::Xbox => "xbox",
            Platform::Manual => "manual",
        }
    }
    pub fn from_str(s: &str) -> Platform {
        match s {
            "steam" => Platform::Steam,
            "epic" => Platform::Epic,
            "gog" => Platform::Gog,
            "ubisoft" => Platform::Ubisoft,
            "battle_net" => Platform::BattleNet,
            "xbox" => Platform::Xbox,
            _ => Platform::Manual,
        }
    }
}

/// A DLL found inside a game's install directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledDll {
    pub family: Family,
    pub path: String,
    pub file_name: String,
    pub version: String,
    /// True if a `.uplift.bak` backup of the original exists next to it.
    pub has_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub platform: Platform,
    pub install_dir: String,
    pub steam_appid: Option<u32>,
    /// Current box-art URL resolved from Steam's store API. Steam replaces
    /// capsule art in place (e.g. Diablo IV each season), so this is
    /// re-resolved on every scan and background cycle.
    pub cover_url: Option<String>,
    pub dlls: Vec<InstalledDll>,
    pub prefs: GamePrefs,
    /// Anti-cheat risk flag resolved from the community blocklist.
    pub anticheat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePrefs {
    pub auto_update: bool,
    /// Re-apply the user's chosen DLL version when a game update reverts it.
    #[serde(default = "default_true")]
    pub reapply: bool,
    /// family -> pinned version ("" means unpinned)
    pub pins: std::collections::HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Default for GamePrefs {
    fn default() -> Self {
        GamePrefs { auto_update: false, reapply: true, pins: Default::default() }
    }
}

/// One downloadable DLL release, from the remote releases manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub family: Family,
    pub version: String,
    pub url: String,
    /// SHA-256 of the final DLL file.
    pub sha256: String,
    /// If the download is a zip, path of the DLL inside it.
    #[serde(default)]
    pub zip_path: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    /// True once the tray has notified the user about this release.
    #[serde(default)]
    pub notified: bool,
    /// True if the DLL is present in the local library.
    #[serde(default)]
    pub downloaded: bool,
}

/// One stored frametime capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchRecord {
    pub id: i64,
    pub at: String,
    pub duration_s: u32,
    pub frames: u32,
    pub avg_fps: f64,
    pub low_1pct: f64,
}

/// Community-curated changelog entry for one release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub family: Family,
    pub version: String,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub known_issues: Vec<String>,
    #[serde(default)]
    pub recommended_preset: Option<String>,
}

/// Anti-cheat blocklist entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiCheatEntry {
    /// Substring matched case-insensitively against the game name.
    #[serde(default)]
    pub name_contains: Option<String>,
    #[serde(default)]
    pub steam_appid: Option<u32>,
    /// e.g. "EAC", "BattlEye", "Vanguard"
    pub system: String,
    /// "block" (refuse to swap) or "warn" (require confirmation, no auto-update)
    #[serde(default = "default_severity")]
    pub severity: String,
}

fn default_severity() -> String {
    "warn".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapRecord {
    pub id: i64,
    pub game_id: i64,
    pub family: Family,
    pub dll_path: String,
    pub from_version: String,
    pub to_version: String,
    pub at: String,
    pub automatic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub poll_hours: u32,
    pub minimize_to_tray: bool,
    pub notify_on_new_release: bool,
    pub launch_at_startup: bool,
    /// "dark" | "light"
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Accent palette: "copper" | "mint" | "azure" | "violet"
    #[serde(default = "default_accent")]
    pub accent: String,
}

fn default_theme() -> String {
    "dark".into()
}

fn default_accent() -> String {
    "copper".into()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            poll_hours: 6,
            minimize_to_tray: true,
            notify_on_new_release: true,
            launch_at_startup: false,
            theme: default_theme(),
            accent: default_accent(),
        }
    }
}

/// Compare dotted version strings numerically ("10.2.0" > "9.9.9").
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split(['.', ',', '-'])
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    let n = va.len().max(vb.len());
    for i in 0..n {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            o => return o,
        }
    }
    std::cmp::Ordering::Equal
}
