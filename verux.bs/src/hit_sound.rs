//! Embedded WAV playback for verified local red hitmarker events.
#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(sound: *const u8, module: isize, flags: u32) -> i32;
}

// Generated from the supplied WAV directory; every entry remains embedded.
include!(concat!(env!("OUT_DIR"), "/hit_sound_catalog.rs"));

pub fn sound(name: &str) -> Result<&'static [u8], String> {
    SOUNDS.iter().find(|(label, _)| *label == name).map(|(_, bytes)| *bytes)
        .ok_or_else(|| format!("Selected hit sound is unavailable: {name}. Select a supplied WAV in General."))
}

pub fn play(bytes: &'static [u8]) -> Result<(), String> {
    // Static bytes stay alive through asynchronous SND_MEMORY playback.
    if unsafe { PlaySoundW(bytes.as_ptr(), 0, 0x0001 | 0x0002 | 0x0004) } == 0 {
        return Err("Hit sound playback failed. Check the Windows audio output and restart processing.".into());
    }
    Ok(())
}

pub fn stop() {
    unsafe { PlaySoundW(std::ptr::null(), 0, 0); }
}
