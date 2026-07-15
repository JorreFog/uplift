use crate::db::{library_dir, Db};
use crate::models::{Family, Release};
use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;

pub fn sha256_file(path: &std::path::Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Download a release into the local library:
///   %APPDATA%/Uplift/library/<family>/<version>/<dll name>
/// The stored file's SHA-256 must match the manifest before it is recorded.
pub async fn download_release(db: &Db, family: Family, version: &str) -> Result<String> {
    let releases = db.get_releases()?;
    let release = releases
        .iter()
        .find(|r| r.family == family && r.version == version)
        .cloned()
        .ok_or_else(|| anyhow!("unknown release {} {}", family.as_str(), version))?;

    if let Some(existing) = db.get_download(family, version)? {
        if std::path::Path::new(&existing).exists() {
            return Ok(existing);
        }
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("uplift/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let bytes = client
        .get(&release.url)
        .send()
        .await
        .context("downloading release")?
        .error_for_status()?
        .bytes()
        .await?;

    let dll_bytes = extract_dll(&release, bytes.to_vec())?;

    // Verify against the manifest hash before anything touches the library.
    let mut hasher = Sha256::new();
    hasher.update(&dll_bytes);
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(&release.sha256) {
        bail!(
            "hash mismatch for {} {} — expected {}, got {}. Refusing to store.",
            family.as_str(),
            version,
            release.sha256,
            actual
        );
    }

    let file_name = release
        .zip_path
        .as_deref()
        .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string())
        .unwrap_or_else(|| default_file_name(family));
    let dir: PathBuf = library_dir().join(family.as_str()).join(version);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(file_name);
    std::fs::write(&dest, &dll_bytes)?;

    let dest_s = dest.to_string_lossy().to_string();
    db.record_download(family, version, &dest_s)?;
    Ok(dest_s)
}

fn extract_dll(release: &Release, bytes: Vec<u8>) -> Result<Vec<u8>> {
    match &release.zip_path {
        None => Ok(bytes),
        Some(inner) => {
            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor).context("opening zip")?;
            let mut file = archive
                .by_name(inner)
                .with_context(|| format!("'{inner}' not found in zip"))?;
            let mut out = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut out)?;
            Ok(out)
        }
    }
}

fn default_file_name(family: Family) -> String {
    family.file_names()[0].to_string()
}
