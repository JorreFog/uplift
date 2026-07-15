//! Per-game DLSS render-preset overrides via NVIDIA's driver settings store
//! (NVAPI DRS) — the same mechanism NVIDIA Profile Inspector uses.
//!
//! Setting IDs and value enums come from NVIDIA's public NvApiDriverSettings.h;
//! function interface IDs are the well-known nvapi_QueryInterface hashes.
//! Everything is loaded dynamically from nvapi64.dll so the app still runs
//! (with presets disabled) on machines without an NVIDIA driver.

#![cfg(windows)]

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

/// NGX_DLSS_SR_OVERRIDE_RENDER_PRESET_SELECTION_ID
pub const SR_PRESET_ID: u32 = 0x10E4_1DF3;
/// NGX_DLSS_RR_OVERRIDE_RENDER_PRESET_SELECTION_ID
pub const RR_PRESET_ID: u32 = 0x10E4_1DF7;
/// RENDER_PRESET_Latest — "newest preset this DLL supports"
pub const PRESET_LATEST: u32 = 0x00FF_FFFF;

type NvStatus = i32;
type Handle = *mut c_void;
type UnicodeString = [u16; 2048];

const OK: NvStatus = 0;

// NVDRS_SETTING_V1 under #pragma pack(push,4): 12320 bytes.
const SETTING_VER1: u32 = (12320) | (1 << 16);
// NVDRS_PROFILE_V1: 4116 bytes.
const PROFILE_VER1: u32 = (4116) | (1 << 16);
// NVDRS_APPLICATION_V1: 12296 bytes.
const APPLICATION_VER1: u32 = (12296) | (1 << 16);

const SETTING_TYPE_DWORD: u32 = 0;
const SETTING_LOCATION_CURRENT: u32 = 0;

#[repr(C, packed(4))]
struct NvdrsSetting {
    version: u32,
    setting_name: UnicodeString,
    setting_id: u32,
    setting_type: u32,
    setting_location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined: [u8; 4100],
    current: [u8; 4100],
}

impl NvdrsSetting {
    fn zeroed() -> Box<Self> {
        // Heap-allocate: 12KB is unfriendly to the stack in async contexts.
        unsafe { Box::new(std::mem::zeroed()) }
    }
    fn current_u32(&self) -> u32 {
        u32::from_ne_bytes(self.current[0..4].try_into().unwrap())
    }
    fn set_current_u32(&mut self, v: u32) {
        self.current[0..4].copy_from_slice(&v.to_ne_bytes());
    }
}

#[repr(C, packed(4))]
struct NvdrsProfile {
    version: u32,
    profile_name: UnicodeString,
    gpu_support: u32,
    is_predefined: u32,
    num_of_apps: u32,
    num_of_settings: u32,
}

#[repr(C, packed(4))]
struct NvdrsApplication {
    version: u32,
    is_predefined: u32,
    app_name: UnicodeString,
    user_friendly_name: UnicodeString,
    launcher: UnicodeString,
}

impl NvdrsApplication {
    fn zeroed() -> Box<Self> {
        unsafe { Box::new(std::mem::zeroed()) }
    }
}

fn unicode(s: &str) -> UnicodeString {
    let mut out = [0u16; 2048];
    for (i, u) in s.encode_utf16().take(2047).enumerate() {
        out[i] = u;
    }
    out
}

macro_rules! nvapi_fns {
    ($($name:ident : $id:expr => fn($($arg:ty),*) ;)*) => {
        struct NvApi {
            $($name: unsafe extern "C" fn($($arg),*) -> NvStatus,)*
        }
        impl NvApi {
            fn load() -> Result<NvApi> {
                unsafe {
                    let lib = libloading::Library::new("nvapi64.dll")
                        .map_err(|_| anyhow!("nvapi64.dll not found — NVIDIA driver required"))?;
                    let query: libloading::Symbol<
                        unsafe extern "C" fn(u32) -> *mut c_void,
                    > = lib.get(b"nvapi_QueryInterface")?;
                    let api = NvApi {
                        $($name: {
                            let p = query($id);
                            if p.is_null() {
                                bail!(concat!("nvapi: ", stringify!($name), " unavailable"));
                            }
                            std::mem::transmute(p)
                        },)*
                    };
                    // Intentionally leak the library: NVAPI stays loaded for
                    // the process lifetime, matching NvAPI_Initialize semantics.
                    std::mem::forget(lib);
                    Ok(api)
                }
            }
        }
    };
}

nvapi_fns! {
    initialize:            0x0150E828 => fn();
    drs_create_session:    0x0694D52E => fn(*mut Handle);
    drs_destroy_session:   0xDAD9CFF8 => fn(Handle);
    drs_load_settings:     0x375DBD6B => fn(Handle);
    drs_save_settings:     0xFCBC7E14 => fn(Handle);
    drs_create_profile:    0xCC176068 => fn(Handle, *mut NvdrsProfile, *mut Handle);
    drs_find_profile:      0x7E4A9A0B => fn(Handle, *const u16, *mut Handle);
    drs_create_application:0x4347A9DE => fn(Handle, Handle, *mut NvdrsApplication);
    drs_find_application:  0xEEE566B2 => fn(Handle, *const u16, *mut Handle, *mut NvdrsApplication);
    drs_get_setting:       0x73BF8338 => fn(Handle, Handle, u32, *mut NvdrsSetting);
    drs_set_setting:       0x577DD202 => fn(Handle, Handle, *mut NvdrsSetting);
    drs_delete_profile:    0x17093206 => fn(Handle, Handle);
}

static NVAPI: OnceLock<Option<NvApi>> = OnceLock::new();

fn api() -> Result<&'static NvApi> {
    NVAPI
        .get_or_init(|| {
            let api = NvApi::load().ok()?;
            (unsafe { (api.initialize)() } == OK).then_some(api)
        })
        .as_ref()
        .ok_or_else(|| anyhow!("NVIDIA driver (nvapi64.dll) is not available on this system"))
}

pub fn driver_available() -> bool {
    api().is_ok()
}

fn check(what: &str, status: NvStatus) -> Result<()> {
    if status == OK {
        Ok(())
    } else {
        bail!("nvapi {what} failed (status {status})")
    }
}

/// A DRS session that always loads settings and destroys itself on drop.
struct Session {
    api: &'static NvApi,
    handle: Handle,
}

impl Session {
    fn open() -> Result<Session> {
        let api = api()?;
        let mut handle: Handle = std::ptr::null_mut();
        check("CreateSession", unsafe { (api.drs_create_session)(&mut handle) })?;
        let s = Session { api, handle };
        check("LoadSettings", unsafe { (api.drs_load_settings)(s.handle) })?;
        Ok(s)
    }

    /// Profile for the exe: the driver's predefined game profile when one
    /// exists, otherwise an Uplift-created profile bound to the exe name.
    fn profile_for_exe(&self, exe: &str) -> Result<Handle> {
        let exe_w = unicode(exe);
        let mut profile: Handle = std::ptr::null_mut();
        let mut app = NvdrsApplication::zeroed();
        app.version = APPLICATION_VER1;
        let found = unsafe {
            (self.api.drs_find_application)(self.handle, exe_w.as_ptr(), &mut profile, &mut *app)
        };
        if found == OK && !profile.is_null() {
            return Ok(profile);
        }

        let name = format!("Uplift - {exe}");
        let name_w = unicode(&name);
        let by_name =
            unsafe { (self.api.drs_find_profile)(self.handle, name_w.as_ptr(), &mut profile) };
        if by_name == OK && !profile.is_null() {
            return Ok(profile);
        }

        let mut prof = NvdrsProfile {
            version: PROFILE_VER1,
            profile_name: unicode(&name),
            gpu_support: 0xFFFF_FFFF,
            is_predefined: 0,
            num_of_apps: 0,
            num_of_settings: 0,
        };
        check("CreateProfile", unsafe {
            (self.api.drs_create_profile)(self.handle, &mut prof, &mut profile)
        })?;
        let mut app = NvdrsApplication::zeroed();
        app.version = APPLICATION_VER1;
        app.app_name = unicode(exe);
        app.user_friendly_name = unicode(exe);
        check("CreateApplication", unsafe {
            (self.api.drs_create_application)(self.handle, profile, &mut *app)
        })?;
        Ok(profile)
    }

    fn get_dword(&self, profile: Handle, setting_id: u32) -> Option<u32> {
        let mut s = NvdrsSetting::zeroed();
        s.version = SETTING_VER1;
        let status =
            unsafe { (self.api.drs_get_setting)(self.handle, profile, setting_id, &mut *s) };
        (status == OK).then(|| s.current_u32())
    }

    fn set_dword(&self, profile: Handle, setting_id: u32, value: u32) -> Result<()> {
        let mut s = NvdrsSetting::zeroed();
        s.version = SETTING_VER1;
        s.setting_id = setting_id;
        s.setting_type = SETTING_TYPE_DWORD;
        s.setting_location = SETTING_LOCATION_CURRENT;
        s.set_current_u32(value);
        check("SetSetting", unsafe {
            (self.api.drs_set_setting)(self.handle, profile, &mut *s)
        })?;
        check("SaveSettings", unsafe { (self.api.drs_save_settings)(self.handle) })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.api.drs_destroy_session)(self.handle) };
    }
}

/// Best guess at the game's main executable: the largest .exe in the install
/// dir that isn't an obvious helper (uninstaller, redist, crash handler).
pub fn main_exe(install_dir: &Path) -> Option<String> {
    const SKIP: [&str; 8] = [
        "unins", "redist", "vcredist", "vc_redist", "dxsetup", "crash", "setup", "installer",
    ];
    walkdir::WalkDir::new(install_dir)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        })
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            !SKIP.iter().any(|s| name.contains(s))
        })
        .max_by_key(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .map(|e| e.file_name().to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct GamePresets {
    /// False when no NVIDIA driver is present — UI hides the feature.
    pub available: bool,
    /// Exe the driver profile is bound to.
    pub exe: Option<String>,
    /// Current SR preset override (0 = driver default). None = never set.
    pub sr: Option<u32>,
    /// Current RR preset override.
    pub rr: Option<u32>,
}

pub fn get_presets(install_dir: &Path) -> GamePresets {
    let unavailable = GamePresets { available: false, exe: None, sr: None, rr: None };
    if !driver_available() {
        return unavailable;
    }
    let Some(exe) = main_exe(install_dir) else {
        return GamePresets { available: true, exe: None, sr: None, rr: None };
    };
    let (sr, rr) = match Session::open() {
        Ok(session) => match session.profile_for_exe(&exe) {
            Ok(profile) => (
                session.get_dword(profile, SR_PRESET_ID),
                session.get_dword(profile, RR_PRESET_ID),
            ),
            Err(_) => (None, None),
        },
        Err(_) => (None, None),
    };
    GamePresets { available: true, exe: Some(exe), sr, rr }
}

/// Set (or clear, with value 0) a preset override for the game.
pub fn set_preset(install_dir: &Path, setting_id: u32, value: u32) -> Result<()> {
    let exe = main_exe(install_dir)
        .ok_or_else(|| anyhow!("could not find the game's executable in its install folder"))?;
    let session = Session::open()?;
    let profile = session.profile_for_exe(&exe)?;
    session.set_dword(profile, setting_id, value)
}

/// Remove an Uplift-created profile again (used by the self-test).
pub fn remove_profile_for_exe(exe: &str) -> Result<()> {
    let session = Session::open()?;
    let name = format!("Uplift - {exe}");
    let name_w = unicode(&name);
    let mut profile: Handle = std::ptr::null_mut();
    let found =
        unsafe { (session.api.drs_find_profile)(session.handle, name_w.as_ptr(), &mut profile) };
    if found == OK && !profile.is_null() {
        check("DeleteProfile", unsafe {
            (session.api.drs_delete_profile)(session.handle, profile)
        })?;
        check("SaveSettings", unsafe { (session.api.drs_save_settings)(session.handle) })?;
    }
    Ok(())
}
