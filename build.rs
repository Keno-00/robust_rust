fn main() {
    generate_pointer_encoding_material();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/veruxapp.ico");
        res.set("ProductName", "Verux APB");
        res.set("FileDescription", "Verux APB");
        res.set("OriginalFilename", "verux_apb.exe");
        res.set("InternalName", "verux_apb");
        res.compile().expect("failed to compile Windows resources");
    }
}

fn generate_pointer_encoding_material() {
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                algorithm: *mut std::ffi::c_void,
                buffer: *mut u8,
                length: u32,
                flags: u32,
            ) -> i32;
        }
        // Independent material for pointer and compatibility-text masking.
        let mut bytes = [0u8; 48];
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), 48, 2) };
        assert!(
            status >= 0,
            "Cannot initialize build-time pointer encoding: Windows RNG failed ({status:#X})"
        );
        let words: [u64; 6] = std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
        });
        assert!(
            words[0] != 0 && words[1] != 0 && words[3] != 0 && words[4] != 0,
            "Windows RNG returned invalid encoding material; rebuild"
        );
        let output =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR missing"));
        std::fs::write(output.join("pointer_encoding_material.rs"), format!(
            "const BUILD_MASK_A: u64 = {}u64;\nconst BUILD_MASK_B: u64 = {}u64;\nconst BUILD_ROTATION: u32 = {}u32;\n",
            words[0], words[1], words[2] % 63 + 1,
        )).expect("Cannot write generated pointer encoding material");
        std::fs::write(output.join("native_text_material.rs"), format!(
            "const TEXT_MASK_A: u64 = {}u64;\nconst TEXT_MASK_B: u64 = {}u64;\nconst TEXT_ROTATION: u32 = {}u32;\n",
            words[3], words[4], words[5] % 63 + 1,
        )).expect("Cannot write generated text encoding material");
        bytes.fill(0);
    }
    #[cfg(not(windows))]
    panic!("Verux native pointer encoding requires the Windows build host RNG");
}
