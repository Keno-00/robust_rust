use std::collections::BTreeMap;
use std::ptr;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use hmac::{Hmac, Mac};
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};
use base64::Engine as _;

type HmacSha256 = Hmac<Sha256>;

pub const PAYHIP_VERIFY_URL: &str = "https://payhip.com/api/v2/license/verify";
pub const OBFUSCATION_VERSION: u8 = 1;
pub const SECONDS_PER_DAY: f64 = 24.0 * 60.0 * 60.0;
pub const ROLLBACK_TOLERANCE_SECONDS: f64 = 5.0;
pub const MAX_TAMPER_BEFORE_REVOKE: u32 = 3;
pub const HEARTBEAT_MIN_INTERVAL_SECONDS: f64 = 1.0;
pub const LEDGER_SCHEMA_VERSION: u32 = 2;
pub const LEDGER_AAD: &[u8] = b"veruxapb-license-ledger-v2";

const PRODUCT_SECRET_SALT_B64: &str = "QRRFpLsRirD1RB+NMG/nTQ==";
const PRODUCT_SECRET_BLOB_B64: &str = "AQpUqccbEUlFTeVqqjkVgIR4W6/UIF24Xn1zXN2WhMz4fsAJcL15dofhjIDnH37ZXYQxQ0doyIWUdJHy5p45po2l0X38IQc=";

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
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub license_key: String,
    pub tier: String,
    pub bought_at_ts: f64,
    pub last_wall: f64,
    pub last_mono: std::time::Instant,
    pub last_heartbeat_mono: std::time::Instant,
}

static ACTIVE_SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);

// --- DPAPI Wrappers ---

pub fn dpapi_protect(data: &[u8], description: &str) -> Result<Vec<u8>, String> {
    let mut data_in = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    
    let desc_wide: Vec<u16> = description.encode_utf16().chain(std::iter::once(0)).collect();
    
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
            let res = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            let _ = LocalFree(HLOCAL(data_out.pbData as *mut std::ffi::c_void));
            Ok(res)
        } else {
            Err(format!("CryptProtectData failed with code {:?}", GetLastError()))
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
            let res = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            let _ = LocalFree(HLOCAL(data_out.pbData as *mut std::ffi::c_void));
            Ok(res)
        } else {
            Err(format!("CryptUnprotectData failed with code {:?}", GetLastError()))
        }
    }
}

// --- CSPRNG Helper ---

pub fn generate_random_bytes(buf: &mut [u8]) -> Result<(), String> {
    unsafe {
        let status = BCryptGenRandom(
            None,
            buf,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        );
        if status.is_err() {
            return Err(format!("BCryptGenRandom failed: {:?}", status));
        }
    }
    Ok(())
}

// --- AES-GCM Encryption Helpers ---

pub fn aes_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("AES GCM key init error: {}", e))?;
    let nonce_ref = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: plaintext,
        aad,
    };
    cipher.encrypt(nonce_ref, payload)
        .map_err(|e| format!("AES GCM encrypt error: {}", e))
}

pub fn aes_decrypt(key: &[u8], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("AES GCM key init error: {}", e))?;
    let nonce_ref = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: ciphertext,
        aad,
    };
    cipher.decrypt(nonce_ref, payload)
        .map_err(|e| format!("AES GCM decrypt error: {}", e))
}

// --- Path Resolvers ---

pub fn get_secure_storage_dir() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
            format!("{}\\AppData\\Local", home)
        });
    std::path::PathBuf::from(base).join("VeruxAPB").join("secure")
}

// --- Ledger Digest Calculation ---

pub fn ledger_digest(payload: &LedgerPayload) -> Result<String, String> {
    let mut temp = payload.clone();
    temp.state_digest = String::new();
    
    let serialized = serde_json::to_string(&temp)
        .map_err(|e| format!("Failed to serialize ledger: {}", e))?;
    
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
        let wrapped = std::fs::read(&key_path)
            .map_err(|e| format!("Failed to read ledger key: {}", e))?;
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
    
    let wrapped_state = std::fs::read(&state_path)
        .map_err(|e| format!("Failed to read ledger state: {}", e))?;
    
    let unwrapped_bytes = dpapi_unprotect(&wrapped_state)?;
    
    let envelope: LedgerEnvelope = serde_json::from_slice(&unwrapped_bytes)
        .map_err(|e| format!("Failed to parse ledger envelope: {}", e))?;
        
    let nonce = base64::engine::general_purpose::STANDARD.decode(&envelope.nonce)
        .map_err(|e| format!("Failed to decode nonce: {}", e))?;
        
    let ciphertext = base64::engine::general_purpose::STANDARD.decode(&envelope.ciphertext)
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

pub fn migrate_legacy_usage_fields(fingerprint: &str, ledger: &mut LedgerPayload) -> Result<(), String> {
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
    
    let legacy_fingerprint = section.get("license_fingerprint").cloned().unwrap_or_default();
    if !legacy_fingerprint.is_empty() && legacy_fingerprint == fingerprint {
        if let Some(used) = section.get("used_seconds").and_then(|s| s.parse::<f64>().ok()) {
            ledger.used_seconds = used;
        }
        if let Some(sessions) = section.get("session_count").and_then(|s| s.parse::<u32>().ok()) {
            ledger.session_count = sessions;
        }
        if let Some(tamper) = section.get("tamper_count").and_then(|s| s.parse::<u32>().ok()) {
            ledger.tamper_count = tamper;
        }
        if let Some(update) = section.get("last_runtime_update_unix").and_then(|s| s.parse::<f64>().ok()) {
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

// --- Product Secret Decryption ---

pub fn reveal_product_secret() -> Result<String, String> {
    let salt = base64::engine::general_purpose::STANDARD.decode(PRODUCT_SECRET_SALT_B64)
        .map_err(|e| format!("Base64 salt decode failed: {}", e))?;
        
    let blob = base64::engine::general_purpose::STANDARD.decode(PRODUCT_SECRET_BLOB_B64)
        .map_err(|e| format!("Base64 blob decode failed: {}", e))?;
        
    if blob.len() < 1 + 16 {
        return Err("Invalid obfuscated secret payload length".to_string());
    }
    
    let version = blob[0];
    if version != OBFUSCATION_VERSION {
        return Err("Unsupported obfuscation version".to_string());
    }
    
    let masked = &blob[1..blob.len()-16];
    let tag = &blob[blob.len()-16..];
    
    let mut stream_key = vec![0u8; masked.len()];
    let mut mac_key = vec![0u8; 32];
    
    pbkdf2::pbkdf2_hmac::<Sha256>(
        &salt,
        b"payhip-obf-stream-v1",
        120_000,
        &mut stream_key,
    );
    
    pbkdf2::pbkdf2_hmac::<Sha256>(
        &salt,
        b"payhip-obf-mac-v1",
        120_000,
        &mut mac_key,
    );
    
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(&mac_key)
        .map_err(|e| format!("HMAC init failed: {}", e))?;
    mac.update(&[version]);
    mac.update(masked);
    let expected_tag = mac.finalize().into_bytes();
    let expected_tag_16 = &expected_tag[..16];
    
    if tag != expected_tag_16 {
        return Err("Secret integrity check failed".to_string());
    }
    
    let mut secret = vec![0u8; masked.len()];
    for idx in 0..masked.len() {
        secret[idx] = masked[idx] ^ stream_key[idx];
    }
    
    String::from_utf8(secret).map_err(|e| format!("Invalid UTF-8 secret: {}", e))
}

// --- Payhip HTTP Request ---

pub fn http_verify_request(license_key: &str, product_secret: &str) -> Result<(u16, String), String> {
    let url = format!("https://payhip.com/api/v2/license/verify?license_key={}", license_key);
    
    let resp = ureq::get(&url)
        .set("Accept", "application/json, text/plain, */*")
        .set("Accept-Language", "en-US,en;q=0.9")
        .set("Cache-Control", "no-cache")
        .set("Pragma", "no-cache")
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .set("Referer", "https://payhip.com/")
        .set("Origin", "https://payhip.com")
        .set("product-secret-key", product_secret)
        .call();
        
    match resp {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string()
                .map_err(|e| format!("Failed to read response body: {}", e))?;
            Ok((status, body))
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string()
                .map_err(|e| format!("Failed to read error response body: {}", e))?;
            Ok((code, body))
        }
        Err(e) => {
            Err(format!("Network error: {}", e))
        }
    }
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
            remaining = remaining.trim_start_matches(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-');
            
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
    
    let normalized = candidate.replace('/', "-").replace('T', " ").replace('Z', " +00:00");
    
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
    
    let mut seconds = (days as i64) * 86400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);
    
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
    let mut candidates = vec![
        user.bought_at.clone(),
        user.expiry.clone(),
    ];
    
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
        for key in &["date", "created_at", "purchased_at", "purchase_date", "ordered_at"] {
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
    
    let name = pick(&["customer_name", "buyer_name", "name", "full_name", "customer"]);
    let email = pick(&["customer_email", "buyer_email", "email"]);
    let expiry = pick(&["expires_at", "expiry", "expiration", "license_expires_at"]);
    let status = pick(&["status", "license_status", "state"]);
    let product_name = pick(&["product_name", "product", "plan_name", "variant"]);
    let bought_at = pick(&["purchased_at", "purchase_date", "created_at", "date", "ordered_at"]);
    let license_key = pick(&["license_key", "key"]);
    
    UserInfo {
        name,
        email,
        expiry,
        status,
        product_name,
        bought_at,
        license_key: if license_key.is_empty() { fallback_key.to_string() } else { license_key },
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

pub fn build_usage_snapshot(license_key: &str, tier: &str, bought_at_ts: f64, ledger: &LedgerPayload) -> UsageSnapshot {
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

pub fn apply_usage_policy(license_key: &str, payload: &serde_json::Value, user: &UserInfo) -> UsagePolicyResult {
    let tier = match infer_variant(payload, user) {
        Some(t) => t,
        None => return UsagePolicyResult {
            valid: false,
            status: "unknown_variation".to_string(),
            message: "Unable to determine product variation for this license.".to_string(),
            usage: None,
        },
    };
    
    let bought_at_ts = pick_purchase_timestamp(payload, user);
    let fingerprint = license_fingerprint(license_key);
    
    let mut ledger = match read_secure_ledger() {
        Ok(l) => l,
        Err(_) => return UsagePolicyResult {
            valid: false,
            status: "ledger_error".to_string(),
            message: "Secure license storage could not be verified.".to_string(),
            usage: None,
        },
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
    let key = match std::fs::read(&key_path).map_err(|e| e.to_string()).and_then(|wrapped| dpapi_unprotect(&wrapped)) {
        Ok(k) => k,
        Err(_) => return UsagePolicyResult {
            valid: false,
            status: "ledger_error".to_string(),
            message: "Secure license storage could not be verified.".to_string(),
            usage: None,
        },
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

// --- Main Verification Endpoint ---

pub fn verify_license_key(license_key: &str) -> VerifyResponse {
    let key = license_key.trim();
    if key.is_empty() {
        return VerifyResponse {
            valid: false,
            message: "Please enter a license key.".to_string(),
            status: "invalid_input".to_string(),
            http_status: 0,
            user: None,
            usage: None,
        };
    }
    
    let secret_val = match reveal_product_secret() {
        Ok(s) => s,
        Err(e) => {
            return VerifyResponse {
                valid: false,
                message: format!("Decryption failed: {}", e),
                status: "decryption_error".to_string(),
                http_status: 0,
                user: None,
                usage: None,
            };
        }
    };
    
    let (status_code, body_text) = match http_verify_request(key, &secret_val) {
        Ok(res) => res,
        Err(e) => {
            return VerifyResponse {
                valid: false,
                message: format!("Network error while verifying license: {}", e),
                status: "network_error".to_string(),
                http_status: 0,
                user: None,
                usage: None,
            };
        }
    };
    
    let lower_body = body_text.to_lowercase();
    let cloudflare_signals = lower_body.contains("cloudflare")
        || lower_body.contains("cf-ray")
        || lower_body.contains("attention required")
        || lower_body.contains("just a moment")
        || lower_body.contains("captcha");
        
    if cloudflare_signals && (status_code == 403 || status_code == 429 || status_code == 503) {
        return VerifyResponse {
            valid: false,
            message: "Verification blocked by Cloudflare. Please try again in a moment.".to_string(),
            status: "cloudflare_blocked".to_string(),
            http_status: status_code,
            user: None,
            usage: None,
        };
    }
    
    let payload: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(p) => p,
        Err(_) => serde_json::Value::Null,
    };
    
    let source = payload.get("data").unwrap_or(&payload);
    
    if status_code >= 400 {
        let message = payload.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        return VerifyResponse {
            valid: false,
            message: if message.is_empty() { format!("Payhip verification failed with HTTP {}.", status_code) } else { message },
            status: "http_error".to_string(),
            http_status: status_code,
            user: None,
            usage: None,
        };
    }
    
    let user = extract_user_info(&payload, key);
    
    let payload_valid = payload.get("valid")
        .or_else(|| payload.get("success"))
        .or_else(|| source.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
        
    let normalized_status = source.get("status")
        .or_else(|| payload.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
        
    let enabled_flag = source.get("enabled").and_then(|v| v.as_bool());
    
    if enabled_flag == Some(false) || normalized_status == "disabled" || normalized_status == "inactive" || normalized_status == "revoked" {
        let message = payload.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        return VerifyResponse {
            valid: false,
            message: if message.is_empty() { "This license key is disabled.".to_string() } else { message },
            status: "disabled_key".to_string(),
            http_status: status_code,
            user: Some(user),
            usage: None,
        };
    }
    
    let returned_key = source.get("license_key")
        .or_else(|| payload.get("license_key"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
        
    if !returned_key.is_empty() && returned_key != key {
        return VerifyResponse {
            valid: false,
            message: "License key mismatch returned by verification service.".to_string(),
            status: "key_mismatch".to_string(),
            http_status: status_code,
            user: Some(user),
            usage: None,
        };
    }
    
    if !payload_valid {
        let message = payload.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        return VerifyResponse {
            valid: false,
            message: if message.is_empty() { "Invalid license key.".to_string() } else { message },
            status: "invalid_key".to_string(),
            http_status: status_code,
            user: Some(user),
            usage: None,
        };
    }
    
    let usage_policy = apply_usage_policy(key, &payload, &user);
    VerifyResponse {
        valid: usage_policy.valid,
        message: if usage_policy.message.is_empty() { "License verified.".to_string() } else { usage_policy.message },
        status: usage_policy.status,
        http_status: status_code,
        user: Some(user),
        usage: usage_policy.usage,
    }
}

// --- Active Session Management ---

pub fn begin_usage_session(license_key: &str, verification_result: &VerifyResponse) -> Result<(), String> {
    if !verification_result.valid {
        return Err("Cannot begin session for invalid verification result".to_string());
    }
    
    let usage = match &verification_result.usage {
        Some(u) => u,
        None => return Err("Missing usage state for this license".to_string()),
    };
    
    let mut ledger = match read_secure_ledger() {
        Ok(l) => l,
        Err(e) => return Err(format!("Secure license storage could not be verified: {}", e)),
    };
    
    let now_wall = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
        
    ledger.session_count += 1;
    ledger.last_runtime_update_unix = now_wall;
    
    let secure_dir = get_secure_storage_dir();
    let key_path = secure_dir.join("ledger.key");
    let key = std::fs::read(&key_path).map_err(|e| e.to_string()).and_then(|wrapped| dpapi_unprotect(&wrapped))?;
    write_secure_ledger(&ledger, &key)?;
    
    let mut session_guard = ACTIVE_SESSION.lock().unwrap();
    *session_guard = Some(ActiveSession {
        license_key: license_key.trim().to_string(),
        tier: usage.tier.clone(),
        bought_at_ts: usage.bought_at_ts,
        last_wall: now_wall,
        last_mono: std::time::Instant::now(),
        last_heartbeat_mono: std::time::Instant::now(),
    });
    
    Ok(())
}

pub fn end_usage_session() {
    let mut session_guard = ACTIVE_SESSION.lock().unwrap();
    if session_guard.is_some() {
        let _ = heartbeat_usage_session_internal(&mut *session_guard, true);
    }
    *session_guard = None;
}

pub fn heartbeat_usage_session(force: bool) -> Option<VerifyResponse> {
    let mut session_guard = ACTIVE_SESSION.lock().unwrap();
    if session_guard.is_none() {
        return None;
    }
    match heartbeat_usage_session_internal(&mut *session_guard, force) {
        Ok(res) => res,
        Err(err_msg) => {
            *session_guard = None;
            Some(VerifyResponse {
                valid: false,
                message: err_msg,
                status: "ledger_error".to_string(),
                http_status: 0,
                user: None,
                usage: None,
            })
        }
    }
}

fn heartbeat_usage_session_internal(session_opt: &mut Option<ActiveSession>, force: bool) -> Result<Option<VerifyResponse>, String> {
    let session = match session_opt {
        Some(s) => s,
        None => return Ok(None),
    };
    
    let now_mono = std::time::Instant::now();
    let now_wall = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
        
    if !force {
        let elapsed_heartbeat = now_mono.duration_since(session.last_heartbeat_mono).as_secs_f64();
        if elapsed_heartbeat < HEARTBEAT_MIN_INTERVAL_SECONDS {
            return Ok(None);
        }
        session.last_heartbeat_mono = now_mono;
    }
    
    let delta_usage = now_mono.duration_since(session.last_mono).as_secs_f64().max(0.0);
    session.last_mono = now_mono;
    
    if !force && delta_usage <= 0.0 {
        return Ok(None);
    }
    
    let mut ledger = read_secure_ledger()?;
        
    if (now_wall + ROLLBACK_TOLERANCE_SECONDS) < session.last_wall {
        ledger.tamper_count += 1;
        
        let secure_dir = get_secure_storage_dir();
        let key_path = secure_dir.join("ledger.key");
        let key = std::fs::read(&key_path).map_err(|e| e.to_string()).and_then(|wrapped| dpapi_unprotect(&wrapped))?;
        write_secure_ledger(&ledger, &key)?;
        
        if ledger.tamper_count >= MAX_TAMPER_BEFORE_REVOKE {
            return Err("License revoked due to repeated system-clock tampering.".to_string());
        }
    }
    
    session.last_wall = now_wall;
    
    ledger.used_seconds += delta_usage;
    ledger.last_runtime_update_unix = now_wall;
    
    let secure_dir = get_secure_storage_dir();
    let key_path = secure_dir.join("ledger.key");
    let key = std::fs::read(&key_path).map_err(|e| e.to_string()).and_then(|wrapped| dpapi_unprotect(&wrapped))?;
    
    let tier = if !ledger.license_tier.is_empty() {
        ledger.license_tier.clone()
    } else {
        session.tier.clone()
    };
    
    let entitlement = get_tier_seconds(&tier);
    let snapshot = build_usage_snapshot(&session.license_key, &tier, ledger.bought_at_ts, &ledger);
    
    write_secure_ledger(&ledger, &key)?;
    
    if let Some(ent) = entitlement {
        if ledger.used_seconds >= ent {
            return Ok(Some(VerifyResponse {
                valid: false,
                message: "License usage time has been fully consumed.".to_string(),
                status: "usage_expired".to_string(),
                http_status: 200,
                user: None,
                usage: Some(snapshot),
            }));
        }
    }
    
    Ok(Some(VerifyResponse {
        valid: true,
        message: "License active.".to_string(),
        status: "valid".to_string(),
        http_status: 200,
        user: None,
        usage: Some(snapshot),
    }))
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
        lines.push(format!("Variation: {}", if usage.tier.is_empty() { "Unknown" } else { &usage.tier }));
        lines.push(format!("Used: {}", if usage.used_text.is_empty() { "0m" } else { &usage.used_text }));
        lines.push(format!("Remaining: {}", if usage.remaining_text.is_empty() { "Unknown" } else { &usage.remaining_text }));
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
    fn test_reveal_product_secret() {
        let secret = reveal_product_secret();
        assert!(secret.is_ok(), "Decryption failed: {:?}", secret.err());
        let s = secret.unwrap();
        assert!(!s.is_empty(), "Decrypted secret is empty");
    }

    #[test]
    fn test_dpapi_roundtrip() {
        let original = b"Hello, Windows DPAPI secret data!";
        let protected = dpapi_protect(original, "Test Description");
        assert!(protected.is_ok(), "DPAPI protect failed: {:?}", protected.err());
        let wrapped = protected.unwrap();
        
        let unprotected = dpapi_unprotect(&wrapped);
        assert!(unprotected.is_ok(), "DPAPI unprotect failed: {:?}", unprotected.err());
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
        assert!(write_res.is_ok(), "Writing ledger failed: {:?}", write_res.err());
        
        let read_res = read_secure_ledger();
        assert!(read_res.is_ok(), "Reading ledger failed: {:?}", read_res.err());
        
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

