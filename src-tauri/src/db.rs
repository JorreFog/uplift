use crate::models::*;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

pub fn data_dir() -> PathBuf {
    // UPLIFT_DATA_DIR redirects all app data; tests use it to stay hermetic.
    let dir = std::env::var_os("UPLIFT_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Uplift")
        });
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn library_dir() -> PathBuf {
    let dir = data_dir().join("library");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl Db {
    pub fn open() -> Result<Db> {
        Self::open_at(&data_dir().join("uplift.db"))
    }

    /// Open a database at an explicit path (tests use a temp file).
    pub fn open_at(path: &std::path::Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS games (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                install_dir TEXT NOT NULL UNIQUE,
                steam_appid INTEGER,
                cover_url TEXT
            );
            CREATE TABLE IF NOT EXISTS dlls (
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                family TEXT NOT NULL,
                path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                version TEXT NOT NULL,
                PRIMARY KEY (game_id, path)
            );
            CREATE TABLE IF NOT EXISTS prefs (
                game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                auto_update INTEGER NOT NULL DEFAULT 0,
                reapply INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS desired (
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                family TEXT NOT NULL,
                version TEXT NOT NULL,
                PRIMARY KEY (game_id, family)
            );
            CREATE TABLE IF NOT EXISTS pins (
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                family TEXT NOT NULL,
                version TEXT NOT NULL,
                PRIMARY KEY (game_id, family)
            );
            CREATE TABLE IF NOT EXISTS releases (
                family TEXT NOT NULL,
                version TEXT NOT NULL,
                url TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                zip_path TEXT,
                release_date TEXT,
                notified INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (family, version)
            );
            CREATE TABLE IF NOT EXISTS downloads (
                family TEXT NOT NULL,
                version TEXT NOT NULL,
                local_path TEXT NOT NULL,
                PRIMARY KEY (family, version)
            );
            CREATE TABLE IF NOT EXISTS swaps (
                id INTEGER PRIMARY KEY,
                game_id INTEGER NOT NULL,
                family TEXT NOT NULL,
                dll_path TEXT NOT NULL,
                from_version TEXT NOT NULL,
                to_version TEXT NOT NULL,
                at TEXT NOT NULL,
                automatic INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        // Migrations for databases created before these columns existed; the
        // error when a column is already there is expected and ignored.
        let _ = conn.execute("ALTER TABLE games ADD COLUMN cover_url TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE prefs ADD COLUMN reapply INTEGER NOT NULL DEFAULT 1",
            [],
        );
        Ok(Db(Mutex::new(conn)))
    }

    // ---- games -------------------------------------------------------------

    /// Insert or update a scanned game; returns its id.
    pub fn upsert_game(
        &self,
        name: &str,
        platform: Platform,
        install_dir: &str,
        steam_appid: Option<u32>,
    ) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO games (name, platform, install_dir, steam_appid)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(install_dir) DO UPDATE SET name = ?1, platform = ?2, steam_appid = ?4",
            params![name, platform.as_str(), install_dir, steam_appid],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM games WHERE install_dir = ?1",
            params![install_dir],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn replace_dlls(&self, game_id: i64, dlls: &[InstalledDll]) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM dlls WHERE game_id = ?1", params![game_id])?;
        let mut stmt = conn.prepare(
            "INSERT INTO dlls (game_id, family, path, file_name, version) VALUES (?1,?2,?3,?4,?5)",
        )?;
        for d in dlls {
            stmt.execute(params![
                game_id,
                d.family.as_str(),
                d.path,
                d.file_name,
                d.version
            ])?;
        }
        Ok(())
    }

    /// Update one installed DLL row after an out-of-band version change.
    pub fn update_dll_version(&self, game_id: i64, path: &str, version: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE dlls SET version = ?3 WHERE game_id = ?1 AND path = ?2",
            params![game_id, path, version],
        )?;
        Ok(())
    }

    // ---- desired versions (drive re-apply after game updates) --------------

    /// Remember the version the user chose for a game+family.
    pub fn set_desired(&self, game_id: i64, family: Family, version: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO desired (game_id, family, version) VALUES (?1, ?2, ?3)
             ON CONFLICT(game_id, family) DO UPDATE SET version = ?3",
            params![game_id, family.as_str(), version],
        )?;
        Ok(())
    }

    /// Forget the choice (the user restored the original DLL).
    pub fn clear_desired(&self, game_id: i64, family: Family) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM desired WHERE game_id = ?1 AND family = ?2",
            params![game_id, family.as_str()],
        )?;
        Ok(())
    }

    pub fn get_desired(&self) -> Result<Vec<(i64, Family, String)>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT game_id, family, version FROM desired")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id, fam, ver)| Family::from_str(&fam).map(|f| (id, f, ver)))
            .collect();
        Ok(rows)
    }

    /// Store resolved box-art URLs, keyed by steam appid.
    pub fn set_cover_urls(&self, covers: &[(u32, String)]) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("UPDATE games SET cover_url = ?2 WHERE steam_appid = ?1")?;
        for (appid, url) in covers {
            stmt.execute(params![appid, url])?;
        }
        Ok(())
    }

    /// Remove games from a platform that were not seen in the latest scan.
    pub fn prune_platform(&self, platform: Platform, seen_dirs: &[String]) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, install_dir FROM games WHERE platform = ?1")?;
        let rows: Vec<(i64, String)> = stmt
            .query_map(params![platform.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (id, dir) in rows {
            if !seen_dirs.contains(&dir) {
                conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
            }
        }
        Ok(())
    }

    pub fn get_games(&self) -> Result<Vec<Game>> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, platform, install_dir, steam_appid, cover_url FROM games ORDER BY name COLLATE NOCASE")?;
        let mut games: Vec<Game> = stmt
            .query_map([], |r| {
                Ok(Game {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    platform: Platform::from_str(&r.get::<_, String>(2)?),
                    install_dir: r.get(3)?,
                    steam_appid: r.get(4)?,
                    cover_url: r.get(5)?,
                    dlls: vec![],
                    prefs: GamePrefs::default(),
                    anticheat: None,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut dll_stmt = conn.prepare(
            "SELECT family, path, file_name, version FROM dlls WHERE game_id = ?1",
        )?;
        let mut pref_stmt =
            conn.prepare("SELECT auto_update, reapply FROM prefs WHERE game_id = ?1")?;
        let mut pin_stmt =
            conn.prepare("SELECT family, version FROM pins WHERE game_id = ?1")?;

        for g in games.iter_mut() {
            g.dlls = dll_stmt
                .query_map(params![g.id], |r| {
                    let path: String = r.get(1)?;
                    Ok(InstalledDll {
                        family: Family::from_str(&r.get::<_, String>(0)?)
                            .unwrap_or(Family::Dlss),
                        has_backup: std::path::Path::new(&format!("{path}.uplift.bak"))
                            .exists(),
                        path,
                        file_name: r.get(2)?,
                        version: r.get(3)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            if let Ok((auto, reapply)) = pref_stmt
                .query_row(params![g.id], |r| {
                    Ok((r.get::<_, i64>(0)? != 0, r.get::<_, i64>(1)? != 0))
                })
            {
                g.prefs.auto_update = auto;
                g.prefs.reapply = reapply;
            }
            g.prefs.pins = pin_stmt
                .query_map(params![g.id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
        }
        Ok(games)
    }

    pub fn set_prefs(&self, game_id: i64, prefs: &GamePrefs) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO prefs (game_id, auto_update, reapply) VALUES (?1, ?2, ?3)
             ON CONFLICT(game_id) DO UPDATE SET auto_update = ?2, reapply = ?3",
            params![game_id, prefs.auto_update as i64, prefs.reapply as i64],
        )?;
        conn.execute("DELETE FROM pins WHERE game_id = ?1", params![game_id])?;
        let mut stmt =
            conn.prepare("INSERT INTO pins (game_id, family, version) VALUES (?1,?2,?3)")?;
        for (family, version) in &prefs.pins {
            if !version.is_empty() {
                stmt.execute(params![game_id, family, version])?;
            }
        }
        Ok(())
    }

    // ---- releases / downloads ----------------------------------------------

    /// Merge remote releases; returns the ones not seen before (for notifications).
    pub fn merge_releases(&self, releases: &[Release]) -> Result<Vec<Release>> {
        let conn = self.0.lock().unwrap();
        let mut fresh = vec![];
        for rel in releases {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM releases WHERE family = ?1 AND version = ?2",
                    params![rel.family.as_str(), rel.version],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            conn.execute(
                "INSERT INTO releases (family, version, url, sha256, zip_path, release_date)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(family, version) DO UPDATE SET url=?3, sha256=?4, zip_path=?5, release_date=?6",
                params![
                    rel.family.as_str(),
                    rel.version,
                    rel.url,
                    rel.sha256,
                    rel.zip_path,
                    rel.release_date
                ],
            )?;
            if !exists {
                fresh.push(rel.clone());
            }
        }
        Ok(fresh)
    }

    pub fn mark_notified(&self, family: Family, version: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE releases SET notified = 1 WHERE family = ?1 AND version = ?2",
            params![family.as_str(), version],
        )?;
        Ok(())
    }

    pub fn get_releases(&self) -> Result<Vec<Release>> {
        let conn = self.0.lock().unwrap();
        let downloads: HashMap<(String, String), String> = conn
            .prepare("SELECT family, version, local_path FROM downloads")?
            .query_map([], |r| {
                Ok(((r.get::<_, String>(0)?, r.get::<_, String>(1)?), r.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        let mut stmt = conn.prepare(
            "SELECT family, version, url, sha256, zip_path, release_date, notified FROM releases",
        )?;
        let mut out: Vec<Release> = stmt
            .query_map([], |r| {
                let family_s: String = r.get(0)?;
                let version: String = r.get(1)?;
                Ok(Release {
                    downloaded: downloads.contains_key(&(family_s.clone(), version.clone())),
                    family: Family::from_str(&family_s).unwrap_or(Family::Dlss),
                    version,
                    url: r.get(2)?,
                    sha256: r.get(3)?,
                    zip_path: r.get(4)?,
                    release_date: r.get(5)?,
                    notified: r.get::<_, i64>(6)? != 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        out.sort_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(out)
    }

    pub fn record_download(&self, family: Family, version: &str, local_path: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (family, version, local_path) VALUES (?1,?2,?3)
             ON CONFLICT(family, version) DO UPDATE SET local_path = ?3",
            params![family.as_str(), version, local_path],
        )?;
        Ok(())
    }

    pub fn get_download(&self, family: Family, version: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT local_path FROM downloads WHERE family = ?1 AND version = ?2",
                params![family.as_str(), version],
                |r| r.get(0),
            )
            .ok())
    }

    // ---- swaps ---------------------------------------------------------------

    pub fn record_swap(
        &self,
        game_id: i64,
        family: Family,
        dll_path: &str,
        from_version: &str,
        to_version: &str,
        automatic: bool,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO swaps (game_id, family, dll_path, from_version, to_version, at, automatic)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                game_id,
                family.as_str(),
                dll_path,
                from_version,
                to_version,
                chrono::Utc::now().to_rfc3339(),
                automatic as i64
            ],
        )?;
        Ok(())
    }

    pub fn get_swaps(&self, limit: u32) -> Result<Vec<SwapRecord>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, game_id, family, dll_path, from_version, to_version, at, automatic
             FROM swaps ORDER BY id DESC LIMIT ?1",
        )?;
        let swaps = stmt
            .query_map(params![limit], |r| {
                Ok(SwapRecord {
                    id: r.get(0)?,
                    game_id: r.get(1)?,
                    family: Family::from_str(&r.get::<_, String>(2)?)
                        .unwrap_or(Family::Dlss),
                    dll_path: r.get(3)?,
                    from_version: r.get(4)?,
                    to_version: r.get(5)?,
                    at: r.get(6)?,
                    automatic: r.get::<_, i64>(7)? != 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(swaps)
    }

    // ---- settings --------------------------------------------------------------

    pub fn get_settings(&self) -> Result<Settings> {
        let conn = self.0.lock().unwrap();
        let json: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'app'",
                [],
                |r| r.get(0),
            )
            .ok();
        Ok(json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default())
    }

    pub fn set_settings(&self, s: &Settings) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![serde_json::to_string(s)?],
        )?;
        Ok(())
    }

    /// Arbitrary cached JSON blobs (changelogs, anticheat list).
    pub fn set_blob(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_blob(&self, key: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        Ok(conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .ok())
    }
}
