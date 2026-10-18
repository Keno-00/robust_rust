//! Protocol v2: signature verification precedes trusting any response data.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use zeroize::Zeroizing;

#[derive(Deserialize)]
struct Envelope {
    key_id: String,
    payload: String,
    signature: String,
}

#[derive(Deserialize)]
struct SignedPayload {
    protocol_version: u32,
    product_id: String,
    request_nonce: String,
    route: String,
    http_status: u16,
    issued_at: u64,
    expires_at: u64,
    result: Value,
}

pub(crate) fn verify(bytes: &[u8], nonce: &str, route: &str, status: u16) -> Result<Value, String> {
    let verify = || -> Result<Value, &'static str> {
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|_| "unsigned_or_malformed_response")?;
        let pins: BTreeMap<String, String> = serde_json::from_str(
            include_str!("../license-server/security/public-keys.json")
                .trim_start_matches('\u{feff}'),
        )
        .map_err(|_| "invalid_public_pins")?;
        let encoded_key = pins.get(&envelope.key_id).ok_or("untrusted_signing_key")?;
        let public: [u8; 32] = URL_SAFE_NO_PAD
            .decode(encoded_key)
            .map_err(|_| "invalid_public_pin")?
            .try_into()
            .map_err(|_| "invalid_public_pin")?;
        let signature = URL_SAFE_NO_PAD
            .decode(&envelope.signature)
            .map_err(|_| "invalid_signature_encoding")?;
        let signature =
            Signature::from_slice(&signature).map_err(|_| "invalid_signature_length")?;
        let payload = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(&envelope.payload)
                .map_err(|_| "invalid_payload_encoding")?,
        );
        VerifyingKey::from_bytes(&public)
            .map_err(|_| "invalid_public_pin")?
            .verify_strict(&payload, &signature)
            .map_err(|_| "signature_mismatch")?;
        let payload: SignedPayload =
            serde_json::from_slice(&payload).map_err(|_| "invalid_signed_schema")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "invalid_system_clock")?
            .as_secs();
        if payload.protocol_version != 2
            || payload.product_id != crate::passport::PRODUCT_ID
            || payload.request_nonce != nonce
            || payload.route != format!("/{route}")
            || payload.http_status != status
            || payload.expires_at < payload.issued_at
            || payload.expires_at - payload.issued_at > 120
            || payload.issued_at > now.saturating_add(300)
            || now > payload.expires_at.saturating_add(300)
        {
            return Err("signed_context_mismatch");
        }
        Ok(payload.result)
    };
    verify().map_err(|code| {
        crate::log_err!("Signed response rejected: code={}, route={}, http_status={}", code, route, status);
        "License response could not be authenticated. Check the clock, update the application, or contact support.".into()
    })
}
