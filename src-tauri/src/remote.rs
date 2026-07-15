use crate::db::Db;
use crate::models::*;
use anyhow::{Context, Result};
use serde::Deserialize;

/// Where the community-maintained data lives: the `manifest/` folder of the
/// app repo itself, fetched raw. Fork the repo and change these to run your
/// own manifest.
pub const RELEASES_URL: &str =
    "https://raw.githubusercontent.com/JorreFog/uplift/main/manifest/dll-releases.json";
pub const CHANGELOGS_URL: &str =
    "https://raw.githubusercontent.com/JorreFog/uplift/main/manifest/changelogs.json";
pub const ANTICHEAT_URL: &str =
    "https://raw.githubusercontent.com/JorreFog/uplift/main/manifest/anticheat.json";

#[derive(Deserialize)]
struct ReleasesFile {
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct ChangelogsFile {
    changelogs: Vec<ChangelogEntry>,
}

#[derive(Deserialize)]
struct AntiCheatFile {
    entries: Vec<AntiCheatEntry>,
}

pub struct RefreshOutcome {
    pub new_releases: Vec<Release>,
}

/// Fetch all three remote documents, persist them, and report which releases
/// are new since the last refresh (those drive notifications).
pub async fn refresh(db: &Db) -> Result<RefreshOutcome> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("uplift/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let releases: ReleasesFile = client
        .get(RELEASES_URL)
        .send()
        .await
        .context("fetching releases manifest")?
        .error_for_status()?
        .json()
        .await
        .context("parsing releases manifest")?;
    let new_releases = db.merge_releases(&releases.releases)?;

    // Changelogs and the anti-cheat list are cached as raw JSON; parse failures
    // on either are non-fatal so a bad PR to one file can't break refreshes.
    if let Ok(resp) = client.get(CHANGELOGS_URL).send().await {
        if let Ok(text) = resp.error_for_status().map(|r| r.text()) {
            if let Ok(text) = text.await {
                if serde_json::from_str::<ChangelogsFile>(&text).is_ok() {
                    db.set_blob("changelogs", &text)?;
                }
            }
        }
    }
    if let Ok(resp) = client.get(ANTICHEAT_URL).send().await {
        if let Ok(text) = resp.error_for_status().map(|r| r.text()) {
            if let Ok(text) = text.await {
                if serde_json::from_str::<AntiCheatFile>(&text).is_ok() {
                    db.set_blob("anticheat", &text)?;
                }
            }
        }
    }

    Ok(RefreshOutcome { new_releases })
}

// ---- box art ----------------------------------------------------------------
//
// Steam serves store assets from content-hashed paths now
// (`store_item_assets/steam/apps/{appid}/{hash}/library_capsule.jpg`) and
// replaces art in place — Diablo IV gets a new capsule every season. The old
// flat `steam/apps/{appid}/library_600x900.jpg` path is a frozen snapshot that
// never updates, so current URLs must be resolved through the store API.

const GET_ITEMS_URL: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/";
const ASSET_BASE: &str = "https://shared.steamstatic.com/store_item_assets/";

#[derive(Deserialize)]
struct GetItemsResponse {
    response: GetItemsBody,
}

#[derive(Deserialize)]
struct GetItemsBody {
    #[serde(default)]
    store_items: Vec<StoreItem>,
}

#[derive(Deserialize)]
struct StoreItem {
    appid: Option<u32>,
    assets: Option<StoreAssets>,
}

#[derive(Deserialize, Default)]
struct StoreAssets {
    asset_url_format: Option<String>,
    library_capsule_2x: Option<String>,
    library_capsule: Option<String>,
    hero_capsule_2x: Option<String>,
    hero_capsule: Option<String>,
    main_capsule_2x: Option<String>,
    main_capsule: Option<String>,
}

impl StoreAssets {
    /// Best available box art, portrait formats first.
    fn best_cover(&self) -> Option<String> {
        let format = self.asset_url_format.as_ref()?;
        let file = self
            .library_capsule_2x
            .as_ref()
            .or(self.library_capsule.as_ref())
            .or(self.hero_capsule_2x.as_ref())
            .or(self.hero_capsule.as_ref())
            .or(self.main_capsule_2x.as_ref())
            .or(self.main_capsule.as_ref())?;
        Some(format!("{ASSET_BASE}{}", format.replace("${FILENAME}", file)))
    }
}

/// Resolve current box-art URLs for every Steam game in one batched store API
/// call and persist them. Failures are the caller's to ignore — art is
/// cosmetic and must never break a scan.
pub async fn refresh_covers(db: &Db) -> Result<()> {
    let appids: Vec<u32> = db
        .get_games()?
        .iter()
        .filter_map(|g| g.steam_appid)
        .collect();
    if appids.is_empty() {
        return Ok(());
    }

    let ids: Vec<serde_json::Value> = appids
        .iter()
        .map(|a| serde_json::json!({ "appid": a }))
        .collect();
    let input = serde_json::json!({
        "ids": ids,
        "context": { "language": "english", "country_code": "US" },
        "data_request": { "include_assets": true },
    });

    let client = reqwest::Client::builder()
        .user_agent(concat!("uplift/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let resp: GetItemsResponse = client
        .get(GET_ITEMS_URL)
        .query(&[("input_json", input.to_string())])
        .send()
        .await
        .context("fetching store items")?
        .error_for_status()?
        .json()
        .await
        .context("parsing store items")?;

    let covers: Vec<(u32, String)> = resp
        .response
        .store_items
        .iter()
        .filter_map(|item| {
            let url = item.assets.as_ref()?.best_cover()?;
            Some((item.appid?, url))
        })
        .collect();
    db.set_cover_urls(&covers)?;
    Ok(())
}

pub fn cached_changelogs(db: &Db) -> Vec<ChangelogEntry> {
    db.get_blob("changelogs")
        .ok()
        .flatten()
        .and_then(|t| serde_json::from_str::<ChangelogsFile>(&t).ok())
        .map(|f| f.changelogs)
        .unwrap_or_default()
}

pub fn cached_anticheat(db: &Db) -> Vec<AntiCheatEntry> {
    db.get_blob("anticheat")
        .ok()
        .flatten()
        .and_then(|t| serde_json::from_str::<AntiCheatFile>(&t).ok())
        .map(|f| f.entries)
        .unwrap_or_default()
}

/// Resolve the anti-cheat flag for a game, if any.
pub fn anticheat_for(entries: &[AntiCheatEntry], name: &str, steam_appid: Option<u32>) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    for e in entries {
        let appid_hit = matches!((e.steam_appid, steam_appid), (Some(a), Some(b)) if a == b);
        let name_hit = e
            .name_contains
            .as_ref()
            .map(|n| lower.contains(&n.to_ascii_lowercase()))
            .unwrap_or(false);
        if appid_hit || name_hit {
            return Some(format!("{}:{}", e.severity, e.system));
        }
    }
    None
}
