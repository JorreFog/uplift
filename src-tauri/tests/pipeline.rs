//! End-to-end test of the download → verify → swap → restore pipeline.
//!
//! Serves the real DLL zips from `dll-archive/` (the files published as
//! GitHub release assets) over localhost, so the exact bytes users download
//! are the bytes under test. Skips if the archive folder is absent.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use uplift_lib::db::Db;
use uplift_lib::dll::read_file_version;
use uplift_lib::models::{Family, Release};
use uplift_lib::{downloads, swap};

fn archive_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dll-archive");
    dir.exists().then(|| dir.canonicalize().unwrap())
}

/// Minimal single-threaded HTTP file server; good enough for two GETs.
fn serve(dir: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/')
                .to_string();
            match std::fs::read(dir.join(&path)) {
                Ok(body) => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(&body);
                }
                Err(_) => {
                    let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    port
}

fn release(family: Family, version: &str, url: String, sha256: &str, zip_path: &str) -> Release {
    Release {
        family,
        version: version.into(),
        url,
        sha256: sha256.into(),
        zip_path: Some(zip_path.into()),
        release_date: None,
        notified: false,
        downloaded: false,
    }
}

#[tokio::test]
async fn download_verify_swap_restore() {
    let Some(archive) = archive_dir() else {
        eprintln!("dll-archive/ not present — skipping pipeline test");
        return;
    };
    let port = serve(archive);
    let tmp = std::env::temp_dir().join(format!("uplift-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // Keep the download library inside the temp dir, away from real app data.
    std::env::set_var("UPLIFT_DATA_DIR", &tmp);
    let db = Db::open_at(&tmp.join("test.db")).unwrap();

    // Hashes must match manifest/dll-releases.json — same bytes users get.
    let new = release(
        Family::Dlss,
        "310.7.0.0",
        format!("http://127.0.0.1:{port}/dlss_v310.7.0.0.zip"),
        "1cdfdc957cb7fc9805500ca6793f607ef2d4dbd7c967feac70aca9caf382d0c0",
        "nvngx_dlss.dll",
    );
    let old = release(
        Family::Dlss,
        "310.6.0.0",
        format!("http://127.0.0.1:{port}/dlss_v310.6.0.0.zip"),
        "4e86dad07a052a0672f231f98a11a463da99239ca73e154d8e27951b260b99a8",
        "nvngx_dlss.dll",
    );
    db.merge_releases(&[new.clone(), old.clone()]).unwrap();

    // Download both; hash verification happens inside download_release.
    let new_path = downloads::download_release(&db, Family::Dlss, "310.7.0.0")
        .await
        .expect("download 310.7");
    let old_path = downloads::download_release(&db, Family::Dlss, "310.6.0.0")
        .await
        .expect("download 310.6");
    assert_eq!(read_file_version(Path::new(&new_path)).unwrap(), "310.7.0.0");
    assert_eq!(read_file_version(Path::new(&old_path)).unwrap(), "310.6.0.0");

    // Tampered hash must be refused.
    let bad = release(
        Family::DlssG,
        "310.7.0.0",
        format!("http://127.0.0.1:{port}/dlss_g_v310.7.0.0.zip"),
        "0000000000000000000000000000000000000000000000000000000000000000",
        "nvngx_dlssg.dll",
    );
    db.merge_releases(&[bad]).unwrap();
    let err = downloads::download_release(&db, Family::DlssG, "310.7.0.0")
        .await
        .expect_err("hash mismatch must fail");
    assert!(err.to_string().contains("hash mismatch"), "got: {err}");

    // Fake game: ships 310.6, swap to 310.7, then restore.
    let game_dir = tmp.join("FakeGame");
    std::fs::create_dir_all(&game_dir).unwrap();
    let target = game_dir.join("nvngx_dlss.dll");
    std::fs::copy(&old_path, &target).unwrap();

    swap::swap(swap::SwapPlan {
        db: &db,
        game_id: 1,
        game_install_dir: &game_dir,
        family: Family::Dlss,
        dll_path: &target,
        source_path: Path::new(&new_path),
        to_version: "310.7.0.0",
        automatic: false,
    })
    .expect("swap");
    assert_eq!(read_file_version(&target).unwrap(), "310.7.0.0");
    let backup = game_dir.join("nvngx_dlss.dll.uplift.bak");
    assert!(backup.exists(), "first swap must create a backup");
    assert_eq!(read_file_version(&backup).unwrap(), "310.6.0.0");

    // Second swap must not clobber the sacred original backup.
    swap::swap(swap::SwapPlan {
        db: &db,
        game_id: 1,
        game_install_dir: &game_dir,
        family: Family::Dlss,
        dll_path: &target,
        source_path: Path::new(&old_path),
        to_version: "310.6.0.0",
        automatic: false,
    })
    .expect("swap back");
    assert_eq!(read_file_version(&backup).unwrap(), "310.6.0.0");

    swap::restore(&db, 1, &game_dir, Family::Dlss, &target).expect("restore");
    assert_eq!(read_file_version(&target).unwrap(), "310.6.0.0");

    let swaps = db.get_swaps(10).unwrap();
    assert_eq!(swaps.len(), 3, "two swaps + one restore recorded");

    let _ = std::fs::remove_dir_all(&tmp);
}
