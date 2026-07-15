//! Global DLSS on-screen indicator toggle.
//!
//! NVIDIA's NGX runtime draws a debug overlay in every DLSS game when
//! `HKLM\SOFTWARE\NVIDIA Corporation\Global\NGXCore\ShowDlssIndicator` is
//! 0x400. It shows the loaded DLL version, render preset and mode — the
//! ground truth for "is my swap/preset actually active?".
//!
//! Reading is unprivileged; writing HKLM needs elevation, so the write goes
//! through reg.exe with the UAC "runas" verb (one prompt per toggle).

#![cfg(windows)]

use anyhow::{bail, Context, Result};

const KEY: &str = r"SOFTWARE\NVIDIA Corporation\Global\NGXCore";
const VALUE: &str = "ShowDlssIndicator";
const ON: u32 = 0x400;

pub fn indicator_enabled() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(KEY)
        .and_then(|k| k.get_value::<u32, _>(VALUE))
        .map(|v| v == ON)
        .unwrap_or(false)
}

/// Toggle the indicator. Spawns an elevated reg.exe (UAC prompt).
pub fn set_indicator(enabled: bool) -> Result<()> {
    let args = if enabled {
        format!(r#"add "HKLM\{KEY}" /v {VALUE} /t REG_DWORD /d {ON} /f"#)
    } else {
        format!(r#"delete "HKLM\{KEY}" /v {VALUE} /f"#)
    };
    // -Wait so we can re-read the value afterwards; UAC decline surfaces as
    // a non-zero exit / failed re-read rather than a hang.
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!("Start-Process reg.exe -ArgumentList '{args}' -Verb RunAs -Wait -WindowStyle Hidden"),
        ])
        .status()
        .context("launching elevated reg.exe")?;
    if !status.success() {
        bail!("elevation was declined — the indicator setting was not changed");
    }
    if indicator_enabled() != enabled {
        bail!("the registry value did not change (elevation declined?)");
    }
    Ok(())
}
