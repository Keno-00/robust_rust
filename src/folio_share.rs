//! Plain-file sharing of ML models and profiles between users. Imported files are
//! untrusted: size-limited, strictly parsed, validated, and never overwrite.
use crate::config::ProfileConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::UI::{
        Controls::Dialogs::{
            CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER,
            OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
            OPENFILENAMEW,
        },
        WindowsAndMessaging::GetForegroundWindow,
    },
};

const MAX_MODEL_FILE: u64 = 32 * 1024 * 1024;
const MAX_PROFILE_FILE: u64 = 1024 * 1024;
const MAX_SETTINGS: usize = 400;
/// Settings that describe this PC or its hardware rather than a play style.
const MACHINE_KEYS: [&str; 10] = [
    "com_port",
    "monires",
    "grab_resolution",
    "grab_shift",
    "unicorn_shift",
    "fps",
    "mouse_polling_rate",
    "apbpath",
    "MLUnicornModel",
    "MLUnicornEnabled",
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedProfile {
    verux_profile: u32,
    product: String,
    settings: HashMap<String, String>,
}

pub enum Shared {
    Model(Vec<u8>),
    Profile(ProfileConfig),
}

/// Opens the Windows file dialog; `None` means the user cancelled.
pub fn choose_file(save: bool, name: &str, description: &str, pattern: &str, extension: &str) -> Result<Option<PathBuf>, String> {
    let mut file = [0u16; 1024];
    for (slot, unit) in file.iter_mut().zip(name.encode_utf16().take(200)) {
        *slot = unit;
    }
    let filter: Vec<u16> = format!("{description}\0{pattern}\0All files\0*.*\0\0").encode_utf16().collect();
    let default_extension: Vec<u16> = extension.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: unsafe { GetForegroundWindow() },
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrDefExt: PCWSTR(default_extension.as_ptr()),
        Flags: if save {
            OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR
        } else {
            OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR
        },
        ..Default::default()
    };
    let accepted = unsafe {
        if save { GetSaveFileNameW(&mut dialog) } else { GetOpenFileNameW(&mut dialog) }
    }
    .as_bool();
    if !accepted {
        let code = unsafe { CommDlgExtendedError() }.0;
        return if code == 0 { Ok(None) } else { Err(format!("The file dialog failed (code {code}).")) };
    }
    let length = file.iter().position(|unit| *unit == 0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(String::from_utf16_lossy(&file[..length]))))
}

/// A file name that is safe to suggest: letters, digits, space, dash and underscore.
pub fn suggested_name(name: &str, extension: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' })
        .take(60)
        .collect();
    format!("{}.{extension}", if clean.trim().is_empty() { "verux" } else { clean.trim() })
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err("Choose a file, not a folder.".into());
    }
    if meta.len() > limit {
        return Err(format!("File is larger than {} MiB.", limit / 1024 / 1024));
    }
    fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

/// Reads a shared file and decides what it is. A profile is returned already
/// merged onto `base` (this PC's settings) and validated; a model is returned
/// as bytes for the ML store to validate.
pub fn read_shared(path: &Path, base: &ProfileConfig) -> Result<Shared, String> {
    let bytes = read_limited(path, MAX_MODEL_FILE)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "This is not a Verux model or config file.".to_string())?;
    if value.get("verux_profile").is_some() {
        if bytes.len() as u64 > MAX_PROFILE_FILE {
            return Err("Config file is larger than 1 MiB.".into());
        }
        return import_profile(value, base).map(Shared::Profile);
    }
    if value.get("personality").is_some() {
        return Ok(Shared::Model(bytes));
    }
    Err("This is not a Verux model or config file.".into())
}

pub fn export_profile(config: &ProfileConfig, path: &Path) -> Result<(), String> {
    let mut settings = config.to_map();
    for key in MACHINE_KEYS {
        settings.remove(key);
    }
    let shared = SharedProfile {
        verux_profile: 1,
        product: crate::passport::PRODUCT_ID.into(),
        settings,
    };
    let bytes = serde_json::to_vec_pretty(&shared).map_err(|e| format!("Cannot encode config: {e}"))?;
    fs::write(path, bytes).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

fn import_profile(value: serde_json::Value, base: &ProfileConfig) -> Result<ProfileConfig, String> {
    let shared: SharedProfile = serde_json::from_value(value)
        .map_err(|_| "Config file is damaged or not a Verux config.".to_string())?;
    if shared.verux_profile != 1 {
        return Err("This config was made by an incompatible version.".into());
    }
    if shared.product != crate::passport::PRODUCT_ID {
        return Err("This config belongs to a different Verux edition.".into());
    }
    if shared.settings.len() > MAX_SETTINGS
        || shared.settings.iter().any(|(k, v)| k.len() > 64 || v.len() > 256 * 1024)
    {
        return Err("Config file has too many or too large settings.".into());
    }
    let mut config = ProfileConfig::from_map(&shared.settings);
    // This PC's hardware and paths always win over the file's.
    config.com_port = base.com_port.clone();
    config.monitor_resolution = base.monitor_resolution;
    config.grab_resolution = base.grab_resolution;
    config.grab_shift = base.grab_shift;
    config.unicorn_shift = base.unicorn_shift;
    config.fps = base.fps;
    config.mouse_polling_rate = base.mouse_polling_rate;
    config.apb_path = base.apb_path.clone();
    config.ml_unicorn_model = base.ml_unicorn_model.clone();
    config.ml_unicorn_enabled = base.ml_unicorn_enabled;
    let sound = &config.hit_sound_name;
    if sound.len() > 64
        || !sound.to_ascii_lowercase().ends_with(".wav")
        || sound.chars().any(|c| matches!(c, '/' | '\\' | ':'))
        || sound.contains("..")
    {
        return Err("The config names an invalid hit sound.".into());
    }
    config.validate_eligibility()?;
    Ok(config)
}
