//! Authenticated encrypted display catalog. Display text opens on the UI thread only.
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use std::{
    cell::OnceCell,
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc, OnceLock,
    },
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    nonce: String,
    ciphertext: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    version: u32,
    #[serde(deserialize_with = "secret_string")]
    key: Zeroizing<String>,
    rows: BTreeMap<String, Row>,
}

fn secret_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(deserializer).map(Zeroizing::new)
}

pub(crate) struct Catalog {
    key: Zeroizing<[u8; 32]>,
    rows: BTreeMap<String, ([u8; 12], Vec<u8>)>,
    pub session_hash: String,
    validated: OnceLock<()>,
}
impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Catalog")
    }
}

pub(crate) struct Frame<'a> {
    catalog: &'a Catalog,
    labels: BTreeMap<&'a str, OnceCell<Zeroizing<String>>>,
}
impl Frame<'_> {
    pub fn text(&self, id: &str) -> &str {
        // Immutable rows are authenticated and UTF-8 checked before rendering.
        // Only labels used by this frame remain open; Drop wipes these strings.
        self.labels
            .get(id)
            .expect("Display ID must belong to the validated catalog")
            .get_or_init(|| {
                self.catalog
                    .decode(id)
                    .expect("Immutable validated display row must open")
            })
            .as_str()
    }
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/petal_ids.rs"));

pub(crate) fn receive() -> Result<Arc<Catalog>, String> {
    let (bytes, session_hash) = crate::algorithm_runtime::receive_algorithm(
        "verux.apb.surface.objects",
        128 * 1024,
        &Arc::new(AtomicU32::new(0)),
        &Arc::new(AtomicBool::new(false)),
    )?;
    let mut packet: Packet = serde_json::from_slice(&bytes)
        .map_err(|_| "Interface catalog is invalid. Contact support.")?;
    let decoded = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(packet.key.as_bytes())
            .map_err(|_| "Interface key is invalid.")?,
    );
    if packet.version != 1 || decoded.len() != 32 || packet.rows.len() != REQUIRED_IDS.len() {
        return Err("Interface catalog is incompatible. Update the application.".into());
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&decoded);
    let mut rows = BTreeMap::new();
    for id in REQUIRED_IDS {
        let row = packet
            .rows
            .remove(*id)
            .ok_or("Interface catalog is incomplete. Contact support.")?;
        let nonce: [u8; 12] = URL_SAFE_NO_PAD
            .decode(row.nonce)
            .map_err(|_| "Interface nonce is invalid.")?
            .try_into()
            .map_err(|_| "Interface nonce is invalid.")?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(row.ciphertext)
            .map_err(|_| "Interface row is invalid.")?;
        if !(16..=4096).contains(&ciphertext.len()) {
            return Err("Interface row exceeds its limit.".into());
        }
        rows.insert(id.to_string(), (nonce, ciphertext));
    }
    crate::passport::check_algorithm_session(&session_hash)?;
    Ok(Arc::new(Catalog {
        key,
        rows,
        session_hash,
        validated: OnceLock::new(),
    }))
}

impl Catalog {
    pub fn open_frame(&self) -> Result<Frame<'_>, String> {
        crate::passport::check_algorithm_session(&self.session_hash)?;
        if self.validated.get().is_none() {
            // Validation stays on the UI thread. Each temporary is wiped before
            // the next row opens, instead of retaining the whole catalog in RAM.
            for id in self.rows.keys() {
                drop(self.decode(id)?);
            }
            let _ = self.validated.set(());
        }
        let labels = self
            .rows
            .keys()
            .map(|id| (id.as_str(), OnceCell::new()))
            .collect();
        Ok(Frame {
            catalog: self,
            labels,
        })
    }

    fn decode(&self, id: &str) -> Result<Zeroizing<String>, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key[..]).map_err(|_| "Interface key is invalid.")?;
        let (nonce, encrypted) = self.rows.get(id).ok_or("Interface row is missing.")?;
        let aad = format!("verux.apb|surface|1|{id}");
        let bytes = Zeroizing::new(
            cipher
                .decrypt(
                    Nonce::from_slice(nonce),
                    Payload {
                        msg: encrypted,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| "Interface row authentication failed. Contact support.")?,
        );
        let text = std::str::from_utf8(&bytes).map_err(|_| "Interface text is invalid.")?;
        if text.is_empty() || text.contains('\0') {
            return Err("Interface text contains invalid characters.".into());
        }
        Ok(Zeroizing::new(text.to_owned()))
    }
}
