//! Round-trip test of the NVAPI DRS preset bindings against the real driver.
//! Skips silently on machines without an NVIDIA driver. Uses a throwaway
//! profile bound to a dummy exe name and deletes it afterwards.

#![cfg(windows)]

use uplift_lib::presets;

#[test]
fn preset_roundtrip() {
    if !presets::driver_available() {
        eprintln!("no NVIDIA driver — skipping preset test");
        return;
    }

    let tmp = std::env::temp_dir().join("uplift-preset-test");
    std::fs::create_dir_all(&tmp).unwrap();
    let exe = "uplift_selftest.exe";
    std::fs::write(tmp.join(exe), b"MZ").unwrap();

    // Set SR preset K (11), read it back, then clear it.
    presets::set_preset(&tmp, presets::SR_PRESET_ID, 11).expect("set preset K");
    let p = presets::get_presets(&tmp);
    assert!(p.available);
    assert_eq!(p.exe.as_deref(), Some(exe));
    assert_eq!(p.sr, Some(11), "expected SR override K after set");

    presets::set_preset(&tmp, presets::SR_PRESET_ID, 0).expect("clear preset");
    let p = presets::get_presets(&tmp);
    assert_eq!(p.sr, Some(0), "expected cleared override");

    presets::remove_profile_for_exe(exe).expect("cleanup profile");
    let _ = std::fs::remove_dir_all(&tmp);
}
