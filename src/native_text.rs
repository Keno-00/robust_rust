//! Build-specific reversible masking. This is not encryption or authorization.
use zeroize::Zeroizing;

include!(concat!(env!("OUT_DIR"), "/native_text_material.rs"));

const fn mask(index: usize) -> u8 {
    let lane = (index % 64) as u32;
    (TEXT_MASK_A.rotate_left(lane)
        ^ TEXT_MASK_B.rotate_right((lane + TEXT_ROTATION) % 64)
        ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) as u8
}

/// Re-encode the existing compatibility table at compile time without changing
/// filenames, legacy settings, or enrolled artifact identities.
pub(crate) const fn from_legacy<const N: usize>(mut bytes: [u8; N]) -> [u8; N] {
    let mut index = 0;
    while index < N {
        bytes[index] ^= 167 ^ mask(index);
        index += 1;
    }
    bytes
}

#[inline(never)]
pub(crate) fn open(bytes: &[u8]) -> Zeroizing<String> {
    // Keep encoded constants from becoming plaintext through LTO folding.
    let bytes = std::hint::black_box(bytes);
    let mut text = Zeroizing::new(String::with_capacity(bytes.len()));
    for (index, byte) in bytes.iter().enumerate() {
        // These compatibility names are ASCII, as in the original table.
        text.push((*byte ^ std::hint::black_box(mask(index))) as char);
    }
    text
}
