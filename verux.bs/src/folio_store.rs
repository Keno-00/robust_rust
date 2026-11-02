//! Product-scoped encrypted profiles; legacy import is explicit and logged.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::Path,
};
use zeroize::Zeroizing;

pub(crate) type Sections = HashMap<String, HashMap<String, String>>;
const HEADER: &str = "[vault]\nversion = 1\npayload = ";
const MAX_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    product: String,
    version: u32,
    sections: Sections,
}

pub(crate) fn read(path: &Path) -> io::Result<Sections> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    if meta.len() > MAX_SIZE {
        return Err(io::Error::other("Profile file exceeds its size limit."));
    }
    let raw = Zeroizing::new(fs::read_to_string(path)?);
    if raw.starts_with(HEADER) {
        let protected = URL_SAFE_NO_PAD
            .decode(raw[HEADER.len()..].trim())
            .map_err(|_| {
                io::Error::other("Encrypted profile payload is invalid. Restore a verified backup.")
            })?;
        let bytes = Zeroizing::new(crate::passport::dpapi_unprotect(&protected).map_err(
            |error| {
                io::Error::other(format!(
                    "Cannot unlock profiles for this Windows account: {error}"
                ))
            },
        )?);
        let mut envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::other("Encrypted profile data is invalid. Restore a verified backup.")
        })?;
        if envelope.product != crate::passport::PRODUCT_ID || envelope.version != 1 {
            return Err(io::Error::other(
                "Encrypted profile belongs to a different product or version.",
            ));
        }
        let mut changed = false;
        for map in envelope.sections.values_mut() {
            changed |= crate::folio_keys::import(map)?;
        }
        if changed {
            write(path, &envelope.sections)?;
            crate::log_info!(
                "Migrated encrypted profile aliases for {}: {}",
                crate::passport::PRODUCT_ID,
                path.display()
            );
        }
        return Ok(envelope.sections);
    }
    // A vault marker with an unknown/corrupt envelope must never be imported as plaintext.
    if raw.contains("[vault]") || raw.contains("payload =") {
        return Err(io::Error::other(
            "Profile vault header is invalid. Restore a verified backup.",
        ));
    }
    let mut sections = import_ini(&raw)?;
    for map in sections.values_mut() {
        crate::folio_keys::import(map)?;
    }
    write(path, &sections)?;
    crate::log_info!(
        "Imported existing profiles into the {} encrypted vault: {}",
        crate::passport::PRODUCT_ID,
        path.display()
    );
    Ok(sections)
}

fn import_ini(raw: &str) -> io::Result<Sections> {
    let mut sections = Sections::new();
    let mut current = "default".to_owned();
    for (number, line) in raw.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let invalid = || {
            io::Error::other(format!("Legacy profile line {} is invalid or duplicated. Correct the INI before importing.", number + 1))
        };
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].trim().to_owned();
            if current.is_empty() || current.contains(['[', ']']) {
                return Err(invalid());
            }
            sections.entry(current.clone()).or_default();
        } else {
            let (key, value) = line.split_once('=').ok_or_else(invalid)?;
            let key = key.trim();
            if key.is_empty() {
                return Err(invalid());
            }
            let map = sections.entry(current.clone()).or_default();
            if map
                .insert(key.to_owned(), value.trim().to_owned())
                .is_some()
            {
                return Err(invalid());
            }
        }
    }
    Ok(sections)
}

pub(crate) fn write(path: &Path, sections: &Sections) -> io::Result<()> {
    let envelope = Envelope {
        product: crate::passport::PRODUCT_ID.into(),
        version: 1,
        sections: sections.clone(),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&envelope).map_err(io::Error::other)?);
    let wrapped = crate::passport::dpapi_protect(&bytes, "Verux APB Profile Vault")
        .map_err(io::Error::other)?;
    let content = format!("{HEADER}{}\n", URL_SAFE_NO_PAD.encode(wrapped));
    if content.len() as u64 > MAX_SIZE {
        return Err(io::Error::other("Profile vault exceeds its size limit."));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut suffix = [0u8; 16];
    crate::passport::generate_random_bytes(&mut suffix).map_err(io::Error::other)?;
    let temporary = path.with_extension(format!("{}.pending", URL_SAFE_NO_PAD.encode(suffix)));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            MoveFileExW(
                windows::core::PCWSTR(from.as_ptr()),
                windows::core::PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(io::Error::other)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
