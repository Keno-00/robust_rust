//! Authenticated, encrypted EchoDriver v3 process-read transport.
use aes_gcm::{
    aead::{AeadInPlace, KeyInit},
    Aes256Gcm, Nonce, Tag,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use zeroize::Zeroizing;

const IOCTL_READ: u32 = 0x226002;
const IOCTL_SESSION: u32 = 0x226004;
const VERSION: u32 = 3;
const SESSION_LIMIT: u64 = 0xFFFFFF;
const HEADER_BYTES: usize = 48;
const FRAME_OVERHEAD: usize = HEADER_BYTES + 16;

// Registry API is kept local to this transport; no key is embedded in a binary.
#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        key: isize,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut std::ffi::c_void,
        size: *mut u32,
    ) -> i32;
}
fn load_key() -> Result<Zeroizing<[u8; 32]>, String> {
    let mut secret = Zeroizing::new([0u8; 32]);
    let mut size = 32;
    // RRF_RT_REG_BINARY | RRF_SUBKEY_WOW6464KEY; HKLM is a sign-extended predefined handle.
    let status = unsafe {
        RegGetValueW(
            0x80000002u32 as i32 as isize,
            w!("SYSTEM\\CurrentControlSet\\Services\\EchoDriver\\Parameters").as_ptr(),
            w!("TransportKey").as_ptr(),
            0x10008,
            std::ptr::null_mut(),
            secret.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 || size != 32 {
        return Err(format!("EchoDriver transport key unavailable (Win32 {status}, length {size}). Run drayebr/scripts/initialize-transport-key.ps1 as Administrator and reload the v3 driver."));
    }
    Ok(secret)
}
fn session_key(master: &[u8; 32], id: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(master).expect("32-byte HMAC key");
    mac.update(b"EchoDriver-v3-session\0");
    mac.update(id);
    let mut result = Zeroizing::new([0u8; 32]);
    result.copy_from_slice(&mac.finalize().into_bytes());
    result
}
struct Session {
    device: HANDLE,
    id: [u8; 16],
    sequence: u64,
    cipher: Aes256Gcm,
}
pub struct AtlasReader {
    session: Mutex<Session>,
}
impl Session {
    fn connect() -> Result<Self, String> {
        let key = load_key()?;
        let device = unsafe {
            CreateFileW(
                w!("\\\\.\\EchoDevice"),
                GENERIC_READ.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|error| {
            format!(
                "Open EchoDevice failed: {error}. Check driver service and administrator access."
            )
        })?;
        let mut hello = [0u8; 32];
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                device,
                IOCTL_SESSION,
                None,
                0,
                Some(hello.as_mut_ptr().cast()),
                32,
                Some(&mut returned),
                None,
            )
        };
        if result.is_err()
            || returned != 32
            || hello[..4] != VERSION.to_le_bytes()
            || hello[4..8] != [0; 4]
            || u64::from_le_bytes(hello[24..32].try_into().unwrap()) != 0
        {
            unsafe {
                let _ = CloseHandle(device);
            }
            return Err(format!("EchoDriver encrypted v3 session failed: {result:?}, returned {returned} bytes. Load the matching v3 driver; v2 is unsupported."));
        }
        let derived = session_key(&key, &hello[8..24]);
        let cipher = Aes256Gcm::new_from_slice(derived.as_ref()).expect("32-byte AES key");
        Ok(Self {
            device,
            id: hello[8..24].try_into().unwrap(),
            sequence: 0,
            cipher,
        })
    }
}
impl AtlasReader {
    pub fn connect() -> Result<Self, String> {
        crate::lattice_encoding::initialize()?;
        Ok(Self { session: Mutex::new(Session::connect()?) })
    }
    pub fn read_exact(
        &self,
        process_id: u32,
        address: u64,
        output: &mut [u8],
    ) -> Result<(), String> {
        // Never leave stale data available on a failed read, including validation/auth failures.
        output.fill(0);
        if process_id == 0
            || address < 0x10000
            || output.is_empty()
            || output.len() > 4096
            || address.checked_add(output.len() as u64 - 1).is_none()
        {
            return Err(
                format!("Invalid memory read: pid={process_id}, address=0x{address:X}, length={}; nonzero PID, user address, and 1-4096 bytes required.", output.len()),
            );
        }
        let mut session = self
            .session
            .lock()
            .map_err(|_| "EchoDriver session lock poisoned; reconnect".to_string())?;
        // Renew on a new file handle: the driver's existing-session query does not reset nonces.
        if session.sequence >= SESSION_LIMIT - 1024 {
            let renewed = Session::connect().map_err(|error| {
                format!("EchoDriver scheduled session renewal failed; processing stopped: {error}")
            })?;
            if renewed.id == session.id {
                return Err("EchoDriver session renewal reused its identity; processing stopped.".into());
            }
            crate::log_info!("EchoDriver encrypted v3 session renewed after {} reads; fresh session identity and key, sequence reset by driver on new handle.", session.sequence);
            *session = renewed;
        }
        session.sequence = session
            .sequence
            .checked_add(1)
            .ok_or("EchoDriver sequence exhausted; reconnect")?;
        let mut header = [0u8; HEADER_BYTES];
        header[..4].copy_from_slice(&VERSION.to_le_bytes());
        header[4..8].copy_from_slice(&1u32.to_le_bytes());
        header[8..24].copy_from_slice(&session.id);
        header[24..32].copy_from_slice(&session.sequence.to_le_bytes());
        if session.sequence > SESSION_LIMIT {
            return Err("EchoDriver session limit reached; reconnect".into());
        }
        header[32..36].copy_from_slice(&1u32.to_le_bytes());
        header[36..44].copy_from_slice(&session.sequence.to_le_bytes());
        header[44..48].copy_from_slice(&16u32.to_le_bytes());
        let mut payload = Zeroizing::new([0u8; 16]);
        payload[..4].copy_from_slice(&process_id.to_le_bytes());
        payload[4..8].copy_from_slice(&(output.len() as u32).to_le_bytes());
        payload[8..16].copy_from_slice(&address.to_le_bytes());
        let tag = session
            .cipher
            .encrypt_in_place_detached(
                Nonce::from_slice(&header[32..44]),
                &header,
                payload.as_mut(),
            )
            .map_err(|_| "EchoDriver request encryption failed".to_string())?;
        let mut request = [0u8; 80];
        request[..48].copy_from_slice(&header);
        request[48..64].copy_from_slice(&tag);
        request[64..].copy_from_slice(payload.as_ref());
        let mut response = Zeroizing::new(vec![0u8; FRAME_OVERHEAD + output.len()]);
        let mut returned = 0;
        unsafe { DeviceIoControl(session.device, IOCTL_READ, Some(request.as_ptr().cast()), 80,
            Some(response.as_mut_ptr().cast()), response.len() as u32, Some(&mut returned), None) }
            .map_err(|error| format!("EchoDriver encrypted read failed for PID {process_id} at 0x{address:X}: {error}. Check matching v3 driver, transport key and requested range."))?;
        if returned as usize != response.len() {
            return Err(format!(
                "EchoDriver v3 response length mismatch: {returned}/{}",
                response.len()
            ));
        }
        let response_header: [u8; 48] = response[..48].try_into().unwrap();
        if response_header[..4] != VERSION.to_le_bytes()
            || response_header[4..8] != 2u32.to_le_bytes()
            || response_header[8..24] != session.id
            || response_header[24..32] != session.sequence.to_le_bytes()
            || response_header[32..36] != 2u32.to_le_bytes()
            || response_header[36..44] != session.sequence.to_le_bytes()
            || response_header[44..48] != (output.len() as u32).to_le_bytes()
        {
            return Err("EchoDriver v3 response session/sequence/header mismatch".into());
        }
        let tag: [u8; 16] = response[48..64].try_into().unwrap();
        session
            .cipher
            .decrypt_in_place_detached(
                Nonce::from_slice(&response_header[32..44]),
                &response_header,
                &mut response[64..],
                Tag::from_slice(&tag),
            )
            .map_err(|_| {
                "EchoDriver response authentication failed; check transport key and reload driver"
                    .to_string()
            })?;
        output.copy_from_slice(&response[64..]);
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.device);
        }
    }
}
