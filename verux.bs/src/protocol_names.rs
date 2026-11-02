// Existing protocol/storage names opened transiently for compatibility.
use crate::native_text::{from_legacy, open};

const NUMERICAL_ID: [u8; 19] = from_legacy([
    209, 194, 213, 210, 223, 137, 198, 215, 197, 137, 212, 202, 200, 200, 211, 207, 206, 201, 192,
]);
const MODEL_DIRECTORY: [u8; 6] = from_legacy([202, 203, 138, 198, 206, 202]);
const PASSPORT_FILE: [u8; 11] =
    from_legacy([203, 206, 196, 194, 201, 212, 194, 137, 204, 194, 222]);

pub(crate) fn numerical_id() -> String {
    open(&NUMERICAL_ID).to_string()
}
pub(crate) fn model_directory() -> String {
    open(&MODEL_DIRECTORY).to_string()
}

pub(crate) fn passport_file() -> zeroize::Zeroizing<String> {
    open(&PASSPORT_FILE)
}
