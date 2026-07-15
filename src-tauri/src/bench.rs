//! Before/after proof: timed frametime captures via Intel's open-source
//! PresentMon (https://github.com/GameTechDev/PresentMon).
//!
//! The console binary is downloaded once from the official GitHub release,
//! pinned by SHA-256. ETW capture needs elevation, so each capture launches
//! PresentMon through a UAC prompt and waits for the timed run to finish,
//! then parses the CSV into a compact summary that gets attached to the
//! game's history — turning "preset K feels smoother" into numbers.

#![cfg(windows)]

use crate::db::data_dir;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const PRESENTMON_URL: &str =
    "https://github.com/GameTechDev/PresentMon/releases/download/v2.5.1/PresentMon-2.5.1-x64.exe";
const PRESENTMON_SHA256: &str =
    "9bec3083069f58f911e6a512f4806db51a27bd096103087bc1d05ef54c80a191";

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub duration_s: u32,
    pub frames: u32,
    pub avg_fps: f64,
    /// 1% low: fps at the 99th-percentile frametime.
    pub low_1pct: f64,
}

fn presentmon_path() -> PathBuf {
    data_dir().join("tools").join("PresentMon-2.5.1-x64.exe")
}

/// Download PresentMon once; verify the pinned hash every launch.
pub async fn ensure_presentmon() -> Result<PathBuf> {
    let path = presentmon_path();
    if !path.exists() {
        let client = reqwest::Client::builder()
            .user_agent(concat!("uplift/", env!("CARGO_PKG_VERSION")))
            .build()?;
        let bytes = client
            .get(PRESENTMON_URL)
            .send()
            .await
            .context("downloading PresentMon")?
            .error_for_status()?
            .bytes()
            .await?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &bytes)?;
    }
    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(&path)?);
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(PRESENTMON_SHA256) {
        let _ = std::fs::remove_file(&path);
        bail!("PresentMon binary failed hash verification — deleted; try again");
    }
    Ok(path)
}

/// Run a timed capture of `exe_name`. Blocks for ~`seconds` (call on a
/// blocking thread). Shows one UAC prompt — ETW needs elevation.
pub fn capture(presentmon: &std::path::Path, exe_name: &str, seconds: u32) -> Result<BenchResult> {
    let csv = data_dir().join(format!("bench-{}.csv", std::process::id()));
    let _ = std::fs::remove_file(&csv);

    let args = format!(
        "--process_name {exe} --output_file '{csv}' --timed {seconds} --terminate_after_timed --stop_existing_session --no_console_stats",
        exe = exe_name,
        csv = csv.display(),
    );
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process '{}' -ArgumentList \"{args}\" -Verb RunAs -Wait -WindowStyle Hidden",
                presentmon.display()
            ),
        ])
        .status()
        .context("launching PresentMon")?;
    if !status.success() {
        bail!("capture cancelled (elevation declined?)");
    }

    let text = std::fs::read_to_string(&csv)
        .map_err(|_| anyhow!("PresentMon produced no data — is the game running?"))?;
    let _ = std::fs::remove_file(&csv);
    parse_csv(&text, seconds)
}

/// Extract frametimes from PresentMon CSV (column is `FrameTime` in 2.x,
/// `MsBetweenPresents` in 1.x) and summarize.
fn parse_csv(text: &str, seconds: u32) -> Result<BenchResult> {
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| anyhow!("empty capture"))?;
    let cols: Vec<&str> = header.split(',').collect();
    let idx = cols
        .iter()
        .position(|c| c.eq_ignore_ascii_case("FrameTime") || c.eq_ignore_ascii_case("MsBetweenPresents"))
        .ok_or_else(|| anyhow!("no frametime column in capture"))?;

    let mut frametimes: Vec<f64> = lines
        .filter_map(|l| l.split(',').nth(idx))
        .filter_map(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .collect();
    if frametimes.len() < 30 {
        bail!(
            "only {} frames captured — make sure the game is running and rendering",
            frametimes.len()
        );
    }

    let frames = frametimes.len() as u32;
    let mean = frametimes.iter().sum::<f64>() / frametimes.len() as f64;
    frametimes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = frametimes[((frametimes.len() as f64 * 0.99) as usize).min(frametimes.len() - 1)];

    Ok(BenchResult {
        duration_s: seconds,
        frames,
        avg_fps: 1000.0 / mean,
        low_1pct: 1000.0 / p99,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_csv;

    #[test]
    fn parses_v2_csv() {
        let mut csv = String::from("Application,ProcessID,SwapChainAddress,PresentRuntime,SyncInterval,PresentFlags,AllowsTearing,PresentMode,FrameTime,CPUBusy\n");
        for i in 0..100 {
            // 90 frames at ~10ms, 10 slow frames at 30ms.
            let ft = if i % 10 == 9 { 30.0 } else { 10.0 };
            csv.push_str(&format!("game.exe,123,0x1,DXGI,1,0,0,Composed,{ft},5.0\n"));
        }
        let r = parse_csv(&csv, 10).unwrap();
        assert_eq!(r.frames, 100);
        assert!((r.avg_fps - 1000.0 / 12.0).abs() < 1.0, "avg {}", r.avg_fps);
        assert!((r.low_1pct - 1000.0 / 30.0).abs() < 1.0, "low {}", r.low_1pct);
    }
}
