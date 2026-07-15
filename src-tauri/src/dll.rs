use crate::models::{Family, InstalledDll};
use anyhow::{anyhow, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Read the file version out of a PE binary's VS_FIXEDFILEINFO.
pub fn read_file_version(path: &Path) -> Result<String> {
    let map = pelite::FileMap::open(path)?;
    let file = pelite::PeFile::from_bytes(&map)?;
    let resources = file.resources().map_err(|e| anyhow!("no resources: {e}"))?;
    let version_info = resources
        .version_info()
        .map_err(|e| anyhow!("no version info: {e}"))?;
    let fixed = version_info
        .fixed()
        .ok_or_else(|| anyhow!("no fixed version info"))?;
    let v = fixed.dwFileVersion;
    Ok(format!("{}.{}.{}.{}", v.Major, v.Minor, v.Patch, v.Build))
}

/// Walk a game directory looking for DLLs Uplift knows how to manage.
/// Depth-limited so huge game folders stay fast; skips common junk dirs.
pub fn discover_dlls(install_dir: &Path) -> Vec<InstalledDll> {
    let mut out = vec![];
    let walker = WalkDir::new(install_dir)
        .max_depth(7)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            !(e.file_type().is_dir()
                && (name == "node_modules" || name.starts_with('.') || name == "__pycache__"))
        });

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(family) = Family::from_file_name(&file_name) else {
            continue;
        };
        let path = entry.path();
        let version = read_file_version(path).unwrap_or_else(|_| "unknown".into());
        let backup = path.with_file_name(format!("{file_name}.uplift.bak"));
        out.push(InstalledDll {
            family,
            path: path.to_string_lossy().to_string(),
            file_name,
            version,
            has_backup: backup.exists(),
        });
    }
    out
}
