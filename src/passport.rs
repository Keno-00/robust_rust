use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ptr;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    BCryptGenRandom, CryptProtectData, CryptUnprotectData, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    CRYPT_INTEGER_BLOB,
};

pub const SECONDS_PER_DAY: f64 = 24.0 * 60.0 * 60.0;
pub const ROLLBACK_TOLERANCE_SECONDS: f64 = 5.0;
pub const MAX_TAMPER_BEFORE_REVOKE: u32 = 3;
pub const HEARTBEAT_MIN_INTERVAL_SECONDS: f64 = 1.0;
pub const LEDGER_SCHEMA_VERSION: u32 = 2;
pub const LEDGER_AAD: &[u8] = b"veruxapb-license-ledger-v2";
pub const PASSPORT_SERVER_URL: &str = match option_env!("VERUX_LICENSE_SERVER_URL") {
    Some(value) => value,
    None => "https://verux-license.verux-services.workers.dev",
};
pub const PRODUCT_ID: &str = "verux.apb";
const MAX_SERVER_RESPONSE_BYTES: u64 = 64 * 1024;
const MAX_SESSION_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub email: String,
    pub expiry: String,
    pub status: String,
    pub product_name: String,
    pub bought_at: String,
    pub license_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LedgerPayload {
    pub schema: u32,
    pub sequence: u32,
    pub license_fingerprint: String,
    pub license_tier: String,
    pub bought_at_ts: f64,
    pub used_seconds: f64,
    pub session_count: u32,
    pub tamper_count: u32,
    pub last_runtime_update_unix: f64,
    pub last_verified_unix: f64,
    pub state_digest: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LedgerEnvelope {
    pub schema: u32,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub license_key: String,
    pub tier: String,
    pub bought_at_ts: f64,
    pub used_seconds: f64,
    pub entitlement_seconds: Option<f64>,
    pub remaining_seconds: Option<f64>,
    pub remaining_text: String,
    pub used_text: String,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub message: String,
    pub status: String,
    pub http_status: u16,
    pub user: Option<UserInfo>,
    pub usage: Option<UsageSnapshot>,
    #[serde(default)]
    pub session_token: String,
    #[serde(default)]
    pub heartbeat_interval_seconds: u64,
    #[serde(default)]
    pub session_ttl_seconds: u64,
    #[serde(default)]
    pub session_hash: String,
    #[serde(skip)]
    pub lease_deadline: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub session_token: String,
    pub sequence: u64,
    pub heartbeat_interval_seconds: u64,
    pub last_heartbeat_mono: std::time::Instant,
    heartbeat_in_flight: bool,
    session_hash: String,
    lease_deadline: std::time::Instant,
}

impl Drop for ActiveSession {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.session_token.zeroize();
    }
}

static ACTIVE_SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);

// --- DPAPI Wrappers ---

pub fn dpapi_protect(data: &[u8], description: &str) -> Result<Vec<u8>, String> {
    let mut data_in = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let desc_wide: Vec<u16> = description
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut entropy = CRYPT_INTEGER_BLOB {
        cbData: LEDGER_AAD.len() as u32,
        pbData: LEDGER_AAD.as_ptr() as *mut u8,
    };

    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        let ok = CryptProtectData(
            &mut data_in,
            PCWSTR(desc_wide.as_ptr()),
            Some(&mut entropy),
            None,
            None,
            0,
            &mut data_out,
        );
        if ok.is_ok() {
            let res =
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            use zeroize::Zeroize;
            std::slice::from_raw_parts_mut(data_out.pbData, data_out.cbData as usize).zeroize();
            let _ = LocalFree(HLOCAL(data_out.pbData as *mut std::ffi::c_void));
            Ok(res)
        } else {
            Err(format!(
                "CryptProtectData failed with code {:?}",
                GetLastError()
            ))
        }
    }
}

pub fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut data_in = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let mut entropy = CRYPT_INTEGER_BLOB {
        cbData: LEDGER_AAD.len() as u32,
        pbData: LEDGER_AAD.as_ptr() as *mut u8,
    };

    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        let ok = CryptUnprotectData(
            &mut data_in,
            None,
            Some(&mut entropy),
            None,
            None,
            0,
            &mut data_out,
        );
        if ok.is_ok() {
            let res =
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            use zeroize::Zeroize;
            std::slice::from_raw_parts_mut(data_out.pbData, data_out.cbData as usize).zeroize();
            let _ = LocalFree(HLOCAL(data_out.pbData as *mut std::ffi::c_void));
            Ok(res)
        } else {
            Err(format!(
                "CryptUnprotectData failed with code {:?}",
                GetLastError()
            ))
        }
    }
}

// --- CSPRNG Helper ---

pub fn generate_random_bytes(buf: &mut [u8]) -> Result<(), String> {
    unsafe {
        let status = BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
        if status.is_err() {
            return Err(format!("BCryptGenRandom failed: {:?}", status));
        }
    }
    Ok(())
}

// --- AES-GCM Encryption Helpers ---

pub fn aes_encrypt(
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES GCM key init error: {}", e))?;
    let nonce_ref = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    cipher
        .encrypt(nonce_ref, payload)
        .map_err(|e| format!("AES GCM encrypt error: {}", e))
}

pub fn aes_decrypt(
    key: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES GCM key init error: {}", e))?;
    let nonce_ref = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: ciphertext,
        aad,
    };
    cipher
        .decrypt(nonce_ref, payload)
        .map_err(|e| format!("AES GCM decrypt error: {}", e))
}

// --- Path Resolvers ---

pub fn get_secure_storage_dir() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
        format!("{}\\AppData\\Local", home)
    });
    std::path::PathBuf::from(base)
        .join("VeruxAPB")
        .join("secure")
}

// --- Ledger Digest Calculation ---

pub fn ledger_digest(payload: &LedgerPayload) -> Result<String, String> {
    let mut temp = payload.clone();
    temp.state_digest = String::new();

    let serialized =
        serde_json::to_string(&temp).map_err(|e| format!("Failed to serialize ledger: {}", e))?;

    let map: BTreeMap<String, serde_json::Value> = serde_json::from_str(&serialized)
        .map_err(|e| format!("Failed to parse JSON map: {}", e))?;

    let compact = serde_json::to_string(&map)
        .map_err(|e| format!("Failed to serialize sorted ledger: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(compact.as_bytes());
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

// --- Secure Ledger Persistence ---

pub fn read_secure_ledger() -> Result<LedgerPayload, String> {
    let secure_dir = get_secure_storage_dir();
    let key_path = secure_dir.join("ledger.key");
    let state_path = secure_dir.join("ledger.state");

    let key = if key_path.exists() {
        let wrapped =
            std::fs::read(&key_path).map_err(|e| format!("Failed to read ledger key: {}", e))?;
        dpapi_unprotect(&wrapped)?
    } else {
        let mut new_key = [0u8; 32];
        generate_random_bytes(&mut new_key)?;
        let wrapped = dpapi_protect(&new_key, "VeruxAPB License Ledger Key")?;
        std::fs::create_dir_all(&secure_dir)
            .map_err(|e| format!("Failed to create secure storage dir: {}", e))?;
        std::fs::write(&key_path, &wrapped)
            .map_err(|e| format!("Failed to write ledger key: {}", e))?;
        new_key.to_vec()
    };

    if key.len() != 32 {
        return Err(format!("Invalid ledger key size: {}", key.len()));
    }

    if !state_path.exists() {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut payload = LedgerPayload {
            schema: LEDGER_SCHEMA_VERSION,
            sequence: 0,
            license_fingerprint: String::new(),
            license_tier: String::new(),
            bought_at_ts: now,
            used_seconds: 0.0,
            session_count: 0,
            tamper_count: 0,
            last_runtime_update_unix: now,
            last_verified_unix: now,
            state_digest: String::new(),
        };
        payload.state_digest = ledger_digest(&payload)?;
        write_secure_ledger(&payload, &key)?;
        return Ok(payload);
    }

    let wrapped_state =
        std::fs::read(&state_path).map_err(|e| format!("Failed to read ledger state: {}", e))?;

    let unwrapped_bytes = dpapi_unprotect(&wrapped_state)?;

    let envelope: LedgerEnvelope = serde_json::from_slice(&unwrapped_bytes)
        .map_err(|e| format!("Failed to parse ledger envelope: {}", e))?;

    let nonce = base64::engine::general_purpose::STANDARD
        .decode(&envelope.nonce)
        .map_err(|e| format!("Failed to decode nonce: {}", e))?;

    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|e| format!("Failed to decode ciphertext: {}", e))?;

    let plaintext = aes_decrypt(&key, &nonce, &ciphertext, LEDGER_AAD)?;

    let payload: LedgerPayload = serde_json::from_slice(&plaintext)
        .map_err(|e| format!("Failed to parse ledger payload: {}", e))?;

    let recorded_digest = payload.state_digest.clone();
    let expected_digest = ledger_digest(&payload)?;

    if recorded_digest != expected_digest {
        return Err("License ledger digest mismatch".to_string());
    }

    Ok(payload)
}

pub fn write_secure_ledger(payload: &LedgerPayload, key: &[u8]) -> Result<(), String> {
    let secure_dir = get_secure_storage_dir();
    let state_path = secure_dir.join("ledger.state");

    let mut new_payload = payload.clone();
    new_payload.schema = LEDGER_SCHEMA_VERSION;
    new_payload.state_digest = String::new();
    new_payload.state_digest = ledger_digest(&new_payload)?;

    let plaintext = serde_json::to_vec(&new_payload)
        .map_err(|e| format!("Failed to serialize ledger: {}", e))?;

    let mut nonce = [0u8; 12];
    generate_random_bytes(&mut nonce)?;

    let ciphertext = aes_encrypt(key, &nonce, &plaintext, LEDGER_AAD)?;

    let envelope = LedgerEnvelope {
        schema: LEDGER_SCHEMA_VERSION,
        nonce: base64::engine::general_purpose::STANDARD.encode(&nonce),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(&ciphertext),
    };

    let envelope_bytes = serde_json::to_vec(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    let wrapped = dpapi_protect(&envelope_bytes, "VeruxAPB License Ledger State")?;

    let tmp_path = secure_dir.join("ledger.state.tmp");
    std::fs::create_dir_all(&secure_dir)
        .map_err(|e| format!("Failed to create secure storage dir: {}", e))?;
    std::fs::write(&tmp_path, &wrapped)
        .map_err(|e| format!("Failed to write tmp ledger state: {}", e))?;

    std::fs::rename(&tmp_path, &state_path)
        .map_err(|e| format!("Failed to rename ledger state: {}", e))?;

    Ok(())
}

pub fn migrate_legacy_usage_fields(
    fingerprint: &str,
    ledger: &mut LedgerPayload,
) -> Result<(), String> {
    let login_path = crate::config::get_login_path();
    if !login_path.exists() {
        return Ok(());
    }
    let content = match std::fs::read_to_string(&login_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut ini = crate::config::IniFile::parse(&content);
    let section = match ini.sections.get_mut("login") {
        Some(s) => s,
        None => return Ok(()),
    };

    let legacy_fingerprint = section
        .get("license_fingerprint")
        .cloned()
        .unwrap_or_default();
    if !legacy_fingerprint.is_empty() && legacy_fingerprint == fingerprint {
        if let Some(used) = section
            .get("used_seconds")
            .and_then(|s| s.parse::<f64>().ok())
        {
            ledger.used_seconds = used;
        }
        if let Some(sessions) = section
            .get("session_count")
            .and_then(|s| s.parse::<u32>().ok())
        {
            ledger.session_count = sessions;
        }
        if let Some(tamper) = section
            .get("tamper_count")
            .and_then(|s| s.parse::<u32>().ok())
        {
            ledger.tamper_count = tamper;
        }
        if let Some(update) = section
            .get("last_runtime_update_unix")
            .and_then(|s| s.parse::<f64>().ok())
        {
            ledger.last_runtime_update_unix = update;
        }
    }

    let legacy_keys = [
        "license_fingerprint",
        "license_tier",
        "bought_at_ts",
        "used_seconds",
        "session_count",
        "tamper_count",
        "last_runtime_update_unix",
        "last_verified_unix",
    ];
    let mut removed = false;
    for key in &legacy_keys {
        if section.remove(*key).is_some() {
            removed = true;
        }
    }

    if removed {
        let serialized = ini.serialize();
        let _ = std::fs::write(&login_path, serialized);
    }
    Ok(())
}

// --- JSON Parsing & Variant Inference Helpers ---

pub fn collect_strings(val: &serde_json::Value, bucket: &mut Vec<String>) {
    match val {
        serde_json::Value::Object(obj) => {
            for v in obj.values() {
                collect_strings(v, bucket);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_strings(v, bucket);
            }
        }
        serde_json::Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() {
                bucket.push(t.to_string());
            }
        }
        _ => {}
    }
}

fn match_tier_rule(text: &str, days: u32) -> bool {
    let lower = text.to_lowercase();
    let search_str = days.to_string();

    let mut pos = 0;
    while let Some(idx) = lower[pos..].find(&search_str) {
        let absolute_idx = pos + idx;

        let preceded_by_digit = if absolute_idx > 0 {
            lower.as_bytes()[absolute_idx - 1].is_ascii_digit()
        } else {
            false
        };

        if !preceded_by_digit {
            let mut remaining = &lower[absolute_idx + search_str.len()..];
            remaining = remaining.trim_start_matches(|c: char| {
                c.is_whitespace() || c == '.' || c == '_' || c == '-'
            });

            if remaining.starts_with("day") {
                let after_day = if remaining.starts_with("days") {
                    &remaining[4..]
                } else {
                    &remaining[3..]
                };

                let followed_by_digit = if let Some(first_char) = after_day.chars().next() {
                    first_char.is_ascii_digit()
                } else {
                    false
                };

                if !followed_by_digit {
                    return true;
                }
            }
        }
        pos = absolute_idx + 1;
    }
    false
}

fn match_lifetime(text: &str) -> bool {
    let lower = text.to_lowercase();
    let mut pos = 0;
    while let Some(idx) = lower[pos..].find("lifetime") {
        let absolute_idx = pos + idx;
        let start_boundary = if absolute_idx > 0 {
            let prev = lower.as_bytes()[absolute_idx - 1];
            !prev.is_ascii_alphanumeric() && prev != b'_'
        } else {
            true
        };

        let end_boundary = if absolute_idx + 8 < lower.len() {
            let next = lower.as_bytes()[absolute_idx + 8];
            !next.is_ascii_alphanumeric() && next != b'_'
        } else {
            true
        };

        if start_boundary && end_boundary {
            return true;
        }
        pos = absolute_idx + 1;
    }
    false
}

pub fn infer_variant(result_payload: &serde_json::Value, user: &UserInfo) -> Option<String> {
    let mut strings = Vec::new();
    collect_strings(result_payload, &mut strings);

    if !user.product_name.is_empty() {
        strings.push(user.product_name.clone());
    }
    if !user.status.is_empty() {
        strings.push(user.status.clone());
    }
    if !user.expiry.is_empty() {
        strings.push(user.expiry.clone());
    }

    for text in &strings {
        if match_tier_rule(text, 30) {
            return Some("30.days".to_string());
        }
        if match_tier_rule(text, 7) {
            return Some("7.days".to_string());
        }
        if match_tier_rule(text, 1) {
            return Some("1.day".to_string());
        }
        if match_lifetime(text) {
            return Some("lifetime".to_string());
        }
    }
    None
}

pub fn parse_timestamp(text: &str) -> Option<f64> {
    let candidate = text.trim();
    if candidate.is_empty() {
        return None;
    }

    if candidate.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(val) = candidate.parse::<u64>() {
            if val > 10_000_000_000 {
                return Some(val as f64 / 1000.0);
            }
            return Some(val as f64);
        }
    }

    let normalized = candidate
        .replace('/', "-")
        .replace('T', " ")
        .replace('Z', " +00:00");

    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let date_part = parts[0];
    let date_split: Vec<&str> = date_part.split('-').collect();
    if date_split.len() < 3 {
        return None;
    }

    let year = date_split[0].parse::<i32>().ok()?;
    let month = date_split[1].parse::<i32>().ok()?;
    let day = date_split[2].parse::<i32>().ok()?;

    let mut hour = 0;
    let mut min = 0;
    let mut sec = 0;

    if parts.len() > 1 {
        let time_part = parts[1];
        let time_only = time_part.split('+').next()?.split('-').next()?;
        let time_split: Vec<&str> = time_only.split(':').collect();
        if time_split.len() >= 2 {
            hour = time_split[0].parse::<i32>().ok()?;
            min = time_split[1].parse::<i32>().ok()?;
            if time_split.len() >= 3 {
                sec = time_split[2].parse::<i32>().ok()?;
            }
        }
    }

    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 12 } else { month };

    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let y_in_era = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day - 1;
    let doe = y_in_era * 365 + y_in_era / 4 - y_in_era / 100 + doy;
    let days = era * 146097 + doe - 719468;

    let mut seconds =
        (days as i64) * 86400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);

    if parts.len() > 1 && parts[1].contains('+') {
        if let Some(tz_part) = parts[1].split('+').nth(1) {
            let tz_split: Vec<&str> = tz_part.split(':').collect();
            if !tz_split.is_empty() {
                if let Ok(tz_h) = tz_split[0].parse::<i64>() {
                    seconds -= tz_h * 3600;
                    if tz_split.len() > 1 {
                        if let Ok(tz_m) = tz_split[1].parse::<i64>() {
                            seconds -= tz_m * 60;
                        }
                    }
                }
            }
        }
    } else if parts.len() > 1 && parts[1].contains('-') {
        if let Some(tz_part) = parts[1].split('-').nth(1) {
            let tz_split: Vec<&str> = tz_part.split(':').collect();
            if !tz_split.is_empty() {
                if let Ok(tz_h) = tz_split[0].parse::<i64>() {
                    seconds += tz_h * 3600;
                    if tz_split.len() > 1 {
                        if let Ok(tz_m) = tz_split[1].parse::<i64>() {
                            seconds += tz_m * 60;
                        }
                    }
                }
            }
        }
    }

    Some(seconds as f64)
}

pub fn pick_purchase_timestamp(payload: &serde_json::Value, user: &UserInfo) -> f64 {
    let mut candidates = vec![user.bought_at.clone(), user.expiry.clone()];

    if let Some(date) = payload.get("date").and_then(|v| v.as_str()) {
        candidates.push(date.to_string());
    }
    if let Some(created) = payload.get("created_at").and_then(|v| v.as_str()) {
        candidates.push(created.to_string());
    }
    if let Some(purchased) = payload.get("purchased_at").and_then(|v| v.as_str()) {
        candidates.push(purchased.to_string());
    }

    if let Some(data) = payload.get("data").and_then(|v| v.as_object()) {
        for key in &[
            "date",
            "created_at",
            "purchased_at",
            "purchase_date",
            "ordered_at",
        ] {
            if let Some(val) = data.get(*key).and_then(|v| v.as_str()) {
                candidates.push(val.to_string());
            }
        }
    }

    for candidate in candidates {
        if let Some(ts) = parse_timestamp(&candidate) {
            return ts;
        }
    }

    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub fn extract_user_info(payload: &serde_json::Value, fallback_key: &str) -> UserInfo {
    let data = payload.get("data").and_then(|v| v.as_object());

    let pick = |keys: &[&str]| -> String {
        for &key in keys {
            if let Some(d) = data {
                if let Some(val) = d.get(key) {
                    let s = match val {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    if !s.trim().is_empty() {
                        return s.trim().to_string();
                    }
                }
            }
            if let Some(val) = payload.get(key) {
                let s = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if !s.trim().is_empty() {
                    return s.trim().to_string();
                }
            }
        }
        String::new()
    };

    let name = pick(&[
        "customer_name",
        "buyer_name",
        "name",
        "full_name",
        "customer",
    ]);
    let email = pick(&["customer_email", "buyer_email", "email"]);
    let expiry = pick(&["expires_at", "expiry", "expiration", "license_expires_at"]);
    let status = pick(&["status", "license_status", "state"]);
    let product_name = pick(&["product_name", "product", "plan_name", "variant"]);
    let bought_at = pick(&[
        "purchased_at",
        "purchase_date",
        "created_at",
        "date",
        "ordered_at",
    ]);
    let license_key = pick(&["license_key", "key"]);

    UserInfo {
        name,
        email,
        expiry,
        status,
        product_name,
        bought_at,
        license_key: if license_key.is_empty() {
            fallback_key.to_string()
        } else {
            license_key
        },
    }
}

// --- Usage Calculation Helpers ---

pub fn get_tier_seconds(tier: &str) -> Option<f64> {
    match tier {
        "1.day" => Some(1.0 * SECONDS_PER_DAY),
        "7.days" => Some(7.0 * SECONDS_PER_DAY),
        "30.days" => Some(30.0 * SECONDS_PER_DAY),
        "lifetime" | _ => None,
    }
}

pub fn format_seconds(seconds: Option<f64>) -> String {
    let s = match seconds {
        None => return "Lifetime".to_string(),
        Some(s) => s,
    };
    if s <= 0.0 {
        return "0m".to_string();
    }
    let total = s as u64;
    let days = total / 86400;
    let rem = total % 86400;
    let hours = rem / 3600;
    let rem = rem % 3600;
    let minutes = rem / 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

pub fn build_usage_snapshot(
    license_key: &str,
    tier: &str,
    bought_at_ts: f64,
    ledger: &LedgerPayload,
) -> UsageSnapshot {
    let entitlement_seconds = get_tier_seconds(tier);
    let used_seconds = ledger.used_seconds;
    let remaining_seconds = entitlement_seconds.map(|ent| (ent - used_seconds).max(0.0));
    let valid = remaining_seconds.map(|rem| rem > 0.0).unwrap_or(true);

    UsageSnapshot {
        license_key: license_key.trim().to_string(),
        tier: tier.to_string(),
        bought_at_ts,
        used_seconds,
        entitlement_seconds,
        remaining_seconds,
        remaining_text: format_seconds(remaining_seconds),
        used_text: format_seconds(Some(used_seconds)),
        valid,
    }
}

// --- Policy Enforcer ---

pub struct UsagePolicyResult {
    pub valid: bool,
    pub status: String,
    pub message: String,
    pub usage: Option<UsageSnapshot>,
}

pub fn apply_usage_policy(
    license_key: &str,
    payload: &serde_json::Value,
    user: &UserInfo,
) -> UsagePolicyResult {
    let tier = match infer_variant(payload, user) {
        Some(t) => t,
        None => {
            return UsagePolicyResult {
                valid: false,
                status: "unknown_variation".to_string(),
                message: "Unable to determine product variation for this license.".to_string(),
                usage: None,
            }
        }
    };

    let bought_at_ts = pick_purchase_timestamp(payload, user);
    let fingerprint = license_fingerprint(license_key);

    let mut ledger = match read_secure_ledger() {
        Ok(l) => l,
        Err(_) => {
            return UsagePolicyResult {
                valid: false,
                status: "ledger_error".to_string(),
                message: "Secure license storage could not be verified.".to_string(),
                usage: None,
            }
        }
    };

    let _ = migrate_legacy_usage_fields(&fingerprint, &mut ledger);

    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    if ledger.license_fingerprint != fingerprint {
        ledger.license_fingerprint = fingerprint;
        ledger.license_tier = tier.clone();
        ledger.bought_at_ts = bought_at_ts;
        ledger.used_seconds = 0.0;
        ledger.session_count = 0;
        ledger.tamper_count = 0;
        ledger.last_runtime_update_unix = now;
    }

    ledger.license_tier = tier.clone();
    ledger.last_verified_unix = now;

    let secure_dir = get_secure_storage_dir();
    let key_path = secure_dir.join("ledger.key");
    let key = match std::fs::read(&key_path)
        .map_err(|e| e.to_string())
        .and_then(|wrapped| dpapi_unprotect(&wrapped))
    {
        Ok(k) => k,
        Err(_) => {
            return UsagePolicyResult {
                valid: false,
                status: "ledger_error".to_string(),
                message: "Secure license storage could not be verified.".to_string(),
                usage: None,
            }
        }
    };

    if let Err(_) = write_secure_ledger(&ledger, &key) {
        return UsagePolicyResult {
            valid: false,
            status: "ledger_error".to_string(),
            message: "Secure license storage could not be verified.".to_string(),
            usage: None,
        };
    }

    let snapshot = build_usage_snapshot(license_key, &tier, ledger.bought_at_ts, &ledger);
    if !snapshot.valid {
        return UsagePolicyResult {
            valid: false,
            status: "usage_expired".to_string(),
            message: "License usage time has been fully consumed.".to_string(),
            usage: Some(snapshot),
        };
    }

    UsagePolicyResult {
        valid: true,
        status: "valid".to_string(),
        message: "License verified.".to_string(),
        usage: Some(snapshot),
    }
}

pub fn license_fingerprint(license_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(license_key.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

// --- Online passport server client ---

#[derive(Debug, Deserialize)]
struct ServerUsage {
    tier: String,
    used_seconds: f64,
    entitlement_seconds: Option<f64>,
    remaining_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ServerResponse {
    valid: bool,
    message: String,
    status: String,
    #[serde(default)]
    session_token: String,
    #[serde(default)]
    heartbeat_interval_seconds: u64,
    #[serde(default)]
    session_ttl_seconds: u64,
    #[serde(default)]
    session_hash: String,
    #[serde(skip)]
    lease_deadline: Option<std::time::Instant>,
    user: Option<UserInfo>,
    usage: Option<ServerUsage>,
}

fn failed_response(status: &str, message: String, http_status: u16) -> VerifyResponse {
    crate::log_err!(
        "License request failed: status={}, http_status={}",
        status,
        http_status
    );
    VerifyResponse {
        valid: false,
        message,
        status: status.to_string(),
        http_status,
        user: None,
        usage: None,
        session_token: String::new(),
        heartbeat_interval_seconds: 0,
        session_ttl_seconds: 0,
        session_hash: String::new(),
        lease_deadline: None,
    }
}

fn install_id() -> Result<String, String> {
    let secure_dir = get_secure_storage_dir();
    std::fs::create_dir_all(&secure_dir)
        .map_err(|e| format!("Unable to create secure storage: {}", e))?;
    let path = secure_dir.join("install.id");
    if path.exists() {
        let wrapped =
            std::fs::read(&path).map_err(|e| format!("Unable to read install identity: {}", e))?;
        let raw = dpapi_unprotect(&wrapped)?;
        if raw.len() != 32 {
            return Err("Stored installation identity is invalid. Contact support.".to_string());
        }
        return Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw));
    }
    let mut raw = [0u8; 32];
    generate_random_bytes(&mut raw)?;
    let wrapped = dpapi_protect(&raw, "Verux Installation Identity")?;
    std::fs::write(&path, wrapped)
        .map_err(|e| format!("Unable to save install identity: {}", e))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw))
}

fn signed_request(path: &str, mut body: serde_json::Value) -> Result<(u16, serde_json::Value, String), String> {
    let mut nonce = [0u8; 32];
    generate_random_bytes(&mut nonce)?;
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce);
    body["protocol_version"] = serde_json::json!(2);
    body["request_nonce"] = serde_json::json!(nonce);
    let origin = PASSPORT_SERVER_URL.trim();
    if origin.is_empty() {
        return Err("This build has no license server configured.".to_string());
    }
    if !origin.starts_with("https://") {
        return Err("License server must use HTTPS.".to_string());
    }
    let url = format!(
        "{}/{}",
        origin.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(std::time::Duration::from_secs(15))
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(12))
        .timeout_write(std::time::Duration::from_secs(8))
        .build();
    let serialized = serde_json::to_string(&body)
        .map_err(|error| format!("Unable to encode license request: {}", error))?;
    let response = agent
        .post(&url)
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .set("User-Agent", concat!("Verux/", env!("CARGO_PKG_VERSION")))
        .send_string(&serialized);
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(error)) => {
            crate::log_err!("License transport failed: route={}, kind={:?}", path, error.kind());
            return Err("Unable to reach the license service. Check your connection and retry. If it persists, contact support and check verux_apb.log.".into());
        }
    };
    let status = response.status();
    let content_type = response.header("Content-Type").unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
    {
        crate::log_err!(
            "License response rejected: invalid_content_type, http_status={}",
            status
        );
        return Err(
            "License server returned an invalid response. Please retry or contact support.".into(),
        );
    }
    use std::io::Read;
    let limit = if path == "v2/algorithms/prepare" { 3 * 1024 * 1024 } else { MAX_SERVER_RESPONSE_BYTES };
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    response
        .into_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Unable to read license response. Please try again.".to_string())?;
    if bytes.len() as u64 > limit {
        crate::log_err!(
            "License response rejected: body_too_large, http_status={}",
            status
        );
        return Err("License server returned an oversized response. Contact support.".into());
    }
    let parsed = crate::security_protocol::verify(&bytes, &nonce, path, status)?;
    Ok((status, parsed, nonce))
}

fn server_request(path: &str, body: serde_json::Value) -> Result<(u16, ServerResponse), String> {
    let request_started = std::time::Instant::now();
    let (status, value, _) = signed_request(path, body)?;
    let mut parsed = serde_json::from_value::<ServerResponse>(value).map_err(|_| {
        crate::log_err!(
            "License response rejected: invalid_json_schema, http_status={}",
            status
        );
        format!(
            "License server returned an invalid response (HTTP {}).",
            status
        )
    })?;
    if let Err(reason) = validate_server_response(path, status, &parsed) {
        crate::log_err!(
            "License response rejected: {}, http_status={}",
            reason,
            status
        );
        return Err(
            "License server returned an inconsistent response. Please retry or contact support."
                .into(),
        );
    }
    parsed.lease_deadline = request_started.checked_add(std::time::Duration::from_secs(parsed.session_ttl_seconds));
    Ok((status, parsed))
}

pub(crate) fn algorithm_request(body: serde_json::Value) -> Result<(serde_json::Value, String), String> {
    let (status, value, nonce) = signed_request("v2/algorithms/prepare", body)?;
    if status != 200 || value.get("valid") != Some(&serde_json::Value::Bool(true)) {
        crate::log_err!("Algorithm preparation refused: http_status={}", status);
        return Err("Preparation is unavailable. Log in again or contact support.".into());
    }
    Ok((value, nonce))
}

pub(crate) fn algorithm_session() -> Result<(String, String), String> {
    let guard = ACTIVE_SESSION.lock().map_err(|_| "License state is unavailable.")?;
    let session = guard.as_ref().ok_or("Please log in again.")?;
    if std::time::Instant::now() >= session.lease_deadline { return Err("Session expired. Please log in again.".into()); }
    Ok((session.session_token.clone(), session.session_hash.clone()))
}

pub(crate) fn session_is_live() -> bool {
    ACTIVE_SESSION.lock().ok().is_some_and(|guard| guard.as_ref().is_some_and(|session| std::time::Instant::now() < session.lease_deadline))
}

pub(crate) fn check_algorithm_session(hash: &str) -> Result<(), String> {
    let (_, current) = algorithm_session()?;
    if current != hash { return Err("Session changed during preparation. Please restart.".into()); }
    Ok(())
}

fn valid_session_token(token: &str) -> bool {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .is_ok_and(|bytes| {
            bytes.len() == 32
                && base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes) == token
        })
}

fn validate_server_response(
    path: &str,
    http_status: u16,
    response: &ServerResponse,
) -> Result<(), &'static str> {
    if response.status.is_empty()
        || response.status.len() > 64
        || !response
            .status
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        || response.message.is_empty()
        || response.message.len() > 2048
    {
        return Err("invalid_status_fields");
    }
    if response.valid {
        if http_status != 200 {
            return Err("success_on_error_http_status");
        }
        let expected = if path == "v1/deactivate" {
            "deactivated"
        } else {
            "valid"
        };
        if response.status != expected {
            return Err("contradictory_success_status");
        }
        if path != "v1/deactivate" && response.usage.is_none() {
            return Err("missing_usage");
        }
        if path != "v1/deactivate" && (!(1..=3600).contains(&response.session_ttl_seconds)
            || response.session_hash.len() != 64 || !response.session_hash.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("invalid_session_lease");
        }
        if path == "v1/activate"
            && (response.user.is_none()
                || !valid_session_token(&response.session_token)
                || !(15..=300).contains(&response.heartbeat_interval_seconds))
        {
            return Err("invalid_activation_session");
        }
    } else if response.status == "valid" || response.status == "deactivated" {
        return Err("contradictory_failure_status");
    }
    if let Some(usage) = &response.usage {
        let expected_entitlement = match usage.tier.as_str() {
            "1.day" => Some(SECONDS_PER_DAY),
            "7.days" => Some(7.0 * SECONDS_PER_DAY),
            "30.days" => Some(30.0 * SECONDS_PER_DAY),
            "lifetime" => None,
            _ => return Err("unknown_usage_tier"),
        };
        if !usage.used_seconds.is_finite()
            || usage.used_seconds < 0.0
            || usage.entitlement_seconds != expected_entitlement
        {
            return Err("invalid_usage_values");
        }
        match (usage.entitlement_seconds, usage.remaining_seconds) {
            (None, None) => (),
            (Some(entitlement), Some(remaining))
                if remaining.is_finite()
                    && remaining >= 0.0
                    && (remaining - (entitlement - usage.used_seconds).max(0.0)).abs()
                        <= 0.000001 =>
            {
                ()
            }
            _ => return Err("inconsistent_remaining_usage"),
        }
        if response.valid
            && usage
                .remaining_seconds
                .is_some_and(|remaining| remaining <= 0.0)
        {
            return Err("success_on_expired_usage");
        }
    }
    Ok(())
}

fn map_server_response(status_code: u16, response: ServerResponse) -> VerifyResponse {
    let usage = response.usage.map(|item| UsageSnapshot {
        license_key: String::new(),
        tier: item.tier,
        bought_at_ts: 0.0,
        used_seconds: item.used_seconds,
        entitlement_seconds: item.entitlement_seconds,
        remaining_seconds: item.remaining_seconds,
        remaining_text: format_seconds(item.remaining_seconds),
        used_text: format_seconds(Some(item.used_seconds)),
        valid: item
            .remaining_seconds
            .map(|seconds| seconds > 0.0)
            .unwrap_or(true),
    });
    VerifyResponse {
        valid: response.valid,
        message: response.message,
        status: response.status,
        http_status: status_code,
        user: response.user,
        usage,
        session_token: response.session_token,
        heartbeat_interval_seconds: response.heartbeat_interval_seconds,
        session_ttl_seconds: response.session_ttl_seconds,
        session_hash: response.session_hash,
        lease_deadline: response.lease_deadline,
    }
}

pub fn verify_passport_key(license_key: &str) -> VerifyResponse {
    let key = license_key.trim();
    if key.is_empty() || key.len() > 512 {
        return failed_response(
            "invalid_input",
            "Please enter a valid license key.".to_string(),
            0,
        );
    }
    let install = match install_id() {
        Ok(value) => value,
        Err(error) => return failed_response("device_error", error, 0),
    };
    match server_request(
        "v1/activate",
        serde_json::json!({
            "product_id": PRODUCT_ID,
            "license_key": key,
            "install_id": install,
            "client_version": env!("CARGO_PKG_VERSION"),
        }),
    ) {
        Ok((status, response)) => map_server_response(status, response),
        Err(error) => failed_response("network_error", error, 0),
    }
}

// --- Active Session Management ---

pub fn begin_usage_session(
    _license_key: &str,
    verification_result: &VerifyResponse,
) -> Result<(), String> {
    if !verification_result.valid
        || verification_result.http_status != 200
        || verification_result.status != "valid"
        || !valid_session_token(&verification_result.session_token)
        || !(15..=300).contains(&verification_result.heartbeat_interval_seconds)
        || verification_result.session_ttl_seconds == 0
        || verification_result.session_ttl_seconds > 3600
    {
        return Err("Cannot begin an invalid online session".to_string());
    }
    let deadline = verification_result.lease_deadline.filter(|deadline| *deadline > std::time::Instant::now())
        .ok_or("Session expired while logging in. Please retry Login.")?;
    let mut session_guard = ACTIVE_SESSION.lock().map_err(|_| "License state is unavailable. Restart.")?;
    *session_guard = Some(ActiveSession {
        session_token: verification_result.session_token.clone(),
        sequence: 0,
        heartbeat_interval_seconds: verification_result.heartbeat_interval_seconds,
        last_heartbeat_mono: std::time::Instant::now(),
        heartbeat_in_flight: false,
        session_hash: verification_result.session_hash.clone(),
        lease_deadline: deadline,
    });
    Ok(())
}

pub fn end_usage_session() {
    let session = ACTIVE_SESSION.lock().unwrap().take();
    if let Some(session) = session {
        let result = server_request(
            "v1/deactivate",
            serde_json::json!({
                "product_id": PRODUCT_ID,
                "session_token": session.session_token,
            }),
        );
        if !matches!(result, Ok((200, ServerResponse { valid: true, .. }))) {
            crate::log_err!("License deactivation failed. The server session will expire; reconnect before starting again.");
        }
    }
}

pub fn heartbeat_usage_session(force: bool) -> Option<VerifyResponse> {
    let mut session_guard = ACTIVE_SESSION.lock().unwrap();
    let session = session_guard.as_mut()?;
    if std::time::Instant::now() >= session.lease_deadline {
        *session_guard = None;
        return Some(failed_response("session_expired", "Session expired. Please log in again.".into(), 0));
    }
    if session.heartbeat_in_flight {
        return None;
    }
    let now = std::time::Instant::now();
    if !force
        && now.duration_since(session.last_heartbeat_mono).as_secs()
            < session.heartbeat_interval_seconds
    {
        return None;
    }
    session.last_heartbeat_mono = now;
    if session.sequence >= MAX_SESSION_SEQUENCE {
        *session_guard = None;
        return Some(failed_response(
            "session_expired",
            "Please log in again to renew the session.".into(),
            0,
        ));
    }
    session.sequence += 1;
    session.heartbeat_in_flight = true;
    let token = session.session_token.clone();
    let sequence = session.sequence;
    let session_hash = session.session_hash.clone();
    let request_started = std::time::Instant::now();
    // Network/DNS waits must not hold the session mutex or block a UI caller
    // trying to inspect or replace state. Only one request owns this session.
    drop(session_guard);
    let result = server_request(
        "v1/heartbeat",
        serde_json::json!({
            "product_id": PRODUCT_ID,
            "session_token": token,
            "sequence": sequence,
        }),
    );
    let mut session_guard = ACTIVE_SESSION.lock().unwrap();
    let current = session_guard.as_mut()?;
    if current.session_token != token {
        return None; // A stopped/replaced session cannot change the new session.
    }
    current.heartbeat_in_flight = false;
    match result {
        Ok((status, response)) => {
            if response.valid && response.session_hash != session_hash {
                *session_guard = None;
                return Some(failed_response("invalid_session", "Session verification failed. Log in again.".into(), status));
            }
            if response.valid {
                current.lease_deadline = request_started + std::time::Duration::from_secs(response.session_ttl_seconds);
            }
            let mapped = map_server_response(status, response);
            if !mapped.valid {
                *session_guard = None;
            }
            Some(mapped)
        }
        Err(error) => {
            *session_guard = None;
            Some(failed_response("network_error", error, 0))
        }
    }
}

// --- UI formatting helper ---

pub fn format_user_info_for_ui(result: &VerifyResponse) -> String {
    let mut lines = Vec::new();
    if result.valid {
        lines.push("License verified".to_string());
    } else {
        let msg = if result.message.is_empty() {
            "License verification failed".to_string()
        } else {
            result.message.clone()
        };
        lines.push(msg);
    }

    if let Some(user) = &result.user {
        if !user.name.is_empty() {
            lines.push(format!("Name: {}", user.name));
        }
        if !user.email.is_empty() {
            lines.push(format!("Email: {}", user.email));
        }
        if !user.status.is_empty() {
            lines.push(format!("Status: {}", user.status));
        }
        if !user.product_name.is_empty() {
            lines.push(format!("Product: {}", user.product_name));
        }
    }

    if let Some(usage) = &result.usage {
        lines.push(format!(
            "Variation: {}",
            if usage.tier.is_empty() {
                "Unknown"
            } else {
                &usage.tier
            }
        ));
        lines.push(format!(
            "Used: {}",
            if usage.used_text.is_empty() {
                "0m"
            } else {
                &usage.used_text
            }
        ));
        lines.push(format!(
            "Remaining: {}",
            if usage.remaining_text.is_empty() {
                "Unknown"
            } else {
                &usage.remaining_text
            }
        ));
    }

    if let Some(user) = &result.user {
        if !user.license_key.is_empty() {
            lines.push(format!("License Key: {}", user.license_key));
        }
    }

    lines.join("\n")
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_roundtrip() {
        let original = b"Hello, Windows DPAPI secret data!";
        let protected = dpapi_protect(original, "Test Description");
        assert!(
            protected.is_ok(),
            "DPAPI protect failed: {:?}",
            protected.err()
        );
        let wrapped = protected.unwrap();

        let unprotected = dpapi_unprotect(&wrapped);
        assert!(
            unprotected.is_ok(),
            "DPAPI unprotect failed: {:?}",
            unprotected.err()
        );
        let unwrapped = unprotected.unwrap();

        assert_eq!(original, unwrapped.as_slice());
    }

    #[test]
    fn test_secure_ledger_roundtrip() {
        let ledger_dir = get_secure_storage_dir();
        let _ = std::fs::create_dir_all(&ledger_dir);

        let key = match read_secure_ledger() {
            Ok(_) => {
                let key_path = ledger_dir.join("ledger.key");
                let wrapped = std::fs::read(&key_path).unwrap();
                dpapi_unprotect(&wrapped).unwrap()
            }
            Err(_) => {
                let key_path = ledger_dir.join("ledger.key");
                let wrapped = std::fs::read(&key_path).unwrap();
                dpapi_unprotect(&wrapped).unwrap()
            }
        };

        let payload = LedgerPayload {
            schema: 2,
            sequence: 42,
            license_fingerprint: "test_fingerprint_sha256".to_string(),
            license_tier: "30.days".to_string(),
            bought_at_ts: 123456789.0,
            used_seconds: 120.0,
            session_count: 5,
            tamper_count: 0,
            last_runtime_update_unix: 123456900.0,
            last_verified_unix: 123456800.0,
            state_digest: String::new(),
        };

        let write_res = write_secure_ledger(&payload, &key);
        assert!(
            write_res.is_ok(),
            "Writing ledger failed: {:?}",
            write_res.err()
        );

        let read_res = read_secure_ledger();
        assert!(
            read_res.is_ok(),
            "Reading ledger failed: {:?}",
            read_res.err()
        );

        let read_payload = read_res.unwrap();
        assert_eq!(read_payload.sequence, 42);
        assert_eq!(read_payload.license_fingerprint, "test_fingerprint_sha256");
        assert_eq!(read_payload.license_tier, "30.days");
        assert_eq!(read_payload.used_seconds, 120.0);
    }

    #[test]
    fn test_timestamp_parser() {
        let ts1 = parse_timestamp("2026-06-22 13:53:35");
        assert!(ts1.is_some());

        let ts2 = parse_timestamp("2026/06/22 13:53:35");
        assert!(ts2.is_some());
        assert_eq!(ts1, ts2);

        let ts3 = parse_timestamp("2026-06-22T13:53:35+08:00");
        assert!(ts3.is_some());

        let ts4 = parse_timestamp("1782117215");
        assert_eq!(ts4, Some(1782117215.0));
    }
}

pub(crate) fn algorithm_lease_remaining(hash: &str) -> Result<std::time::Duration,String> {
    let guard=ACTIVE_SESSION.lock().map_err(|_|"License state is unavailable.")?;
    let session=guard.as_ref().ok_or("Please log in again.")?;
    if session.session_hash!=hash {return Err("Session changed. Reopen Dataset.".into());}
    session.lease_deadline.checked_duration_since(std::time::Instant::now()).ok_or("Session expired. Log in again.".into())
}
