//! Reversible XOR/rotation encoding for native metadata and cached target addresses.
//! This is extraction friction, not encryption or an authorization boundary.
use std::fmt;
use std::sync::OnceLock;
use zeroize::Zeroizing;

include!(concat!(env!("OUT_DIR"), "/pointer_encoding_material.rs"));

/// Build-specific encoded offset. Decode only at the arithmetic/read boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EncodedOffset(u64);

impl EncodedOffset {
    pub const fn new(value: u64) -> Self {
        Self((value ^ BUILD_MASK_A).rotate_left(BUILD_ROTATION) ^ BUILD_MASK_B)
    }

    #[inline(never)]
    pub fn get(self) -> u64 {
        // Best-effort prevention of constant folding; not a cryptographic guarantee.
        (std::hint::black_box(self.0) ^ std::hint::black_box(BUILD_MASK_B))
            .rotate_right(std::hint::black_box(BUILD_ROTATION))
            ^ std::hint::black_box(BUILD_MASK_A)
    }
}

/// Mask copied instance fingerprints too: their raw bytes contain target pointers.
pub(crate) fn encode_fingerprint<const N: usize>(mut value: [u8; N]) -> [u8; N] {
    let mask = std::hint::black_box(BUILD_MASK_A).to_le_bytes();
    let second = std::hint::black_box(BUILD_MASK_B).to_le_bytes();
    for (index, byte) in value.iter_mut().enumerate() {
        *byte ^= mask[index % 8] ^ second[(index / 8) % 8];
    }
    value
}

impl fmt::Debug for EncodedOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedOffset")
    }
}

/// Keeps a type's module RVA encoded; class marker is a diagnostic type ID.
#[derive(Debug, Clone, Copy)]
pub struct ObjectType {
    rva: EncodedOffset,
    marker: u32,
}

impl ObjectType {
    pub const fn new(rva: u64, marker: u32) -> Self {
        Self {
            rva: EncodedOffset::new(rva),
            marker,
        }
    }
    pub fn matches(self, kind: (u64, u32)) -> bool {
        kind.0 == self.rva.get() && kind.1 == self.marker
    }
}

impl PartialEq<ObjectType> for (u64, u32) {
    fn eq(&self, expected: &ObjectType) -> bool {
        expected.matches(*self)
    }
}

struct Key {
    masks: Zeroizing<[u64; 2]>,
    rotation: u32,
}
static KEY: OnceLock<Result<Key, String>> = OnceLock::new();

/// Fail explicitly on RNG failure; there is no predictable or unencoded substitute.
pub fn initialize() -> Result<(), String> {
    runtime_key().map(|_| ())
}

fn runtime_key() -> Result<&'static Key, String> {
    KEY.get_or_init(|| {
        let mut bytes = Zeroizing::new([0u8; 24]);
        crate::passport::generate_random_bytes(&mut bytes[..]).map_err(|error| {
            crate::log_err!("Pointer encoding initialization failed: {error}");
            "Cannot initialize pointer encoding. Restart Verux and inspect verux_apb.log."
                .to_string()
        })?;
        let words: [u64; 3] = std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
        });
        if words[0] == 0 || words[1] == 0 {
            crate::log_err!("Pointer encoding initialization failed: invalid RNG material");
            return Err(
                "Pointer encoding initialization failed. Restart Verux and inspect verux_apb.log."
                    .into(),
            );
        }
        Ok(Key {
            masks: Zeroizing::new([words[0], words[1]]),
            rotation: (words[2] % 63 + 1) as u32,
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Data-only encoding; never used to mangle Rust references, handles or function pointers.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncodedAddress(u64);

impl EncodedAddress {
    pub fn new(value: u64) -> Result<Self, String> {
        let key = runtime_key()?;
        Ok(Self(
            (value ^ key.masks[0]).rotate_left(key.rotation) ^ key.masks[1],
        ))
    }
    #[inline(never)]
    pub fn get(self) -> u64 {
        // Private representation can only be constructed after successful initialization.
        let key = KEY
            .get()
            .and_then(|result| result.as_ref().ok())
            .expect("EncodedAddress construction invariant violated");
        (std::hint::black_box(self.0) ^ key.masks[1]).rotate_right(key.rotation) ^ key.masks[0]
    }
}

impl fmt::Debug for EncodedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedAddress")
    }
}
impl fmt::UpperHex for EncodedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Existing explicitly requested address diagnostics retain their meaning.
        fmt::UpperHex::fmt(&self.get(), f)
    }
}
