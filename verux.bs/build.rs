fn main() {
    enforce_canonical_workspace();
    generate_pointer_encoding_material();
    generate_hit_sound_catalog();
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

/// Release builds come only from the canonical checkout and land in its own
/// target folder; see AGENTS.md "Workspace and sandbox rule".
fn enforce_canonical_workspace() {
    println!("cargo:rerun-if-env-changed=VERUX_SOURCE_ROOT");
    if std::env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }
    let root = std::env::var("VERUX_SOURCE_ROOT")
        .unwrap_or_else(|_| r"C:\Users\Admin\Documents\source_repos".to_string());
    let real = |path: &std::path::Path| {
        std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|_| panic!("Workspace rule: cannot resolve {}", path.display()))
    };
    let expected = std::path::Path::new(&root).join("robust_rust");
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory missing"),
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR missing"));
    assert!(
        expected.exists() && real(&manifest) == real(&expected),
        "Workspace rule: release builds must run from {} (this checkout: {}). Move the work into the main repo instead of building a copy or sandbox.",
        expected.display(),
        manifest.display()
    );
    assert!(
        manifest.join(".git").is_dir(),
        "Workspace rule: {} is a git worktree or has no repository; release from the main checkout.",
        manifest.display()
    );
    let target = real(&manifest.join("target"));
    assert!(
        real(&out).starts_with(&(target + "\\")),
        "Workspace rule: release output must stay under {}\\target (OUT_DIR is {}). Do not use a --target-dir outside the repo.",
        manifest.display(),
        out.display()
    );
}

fn generate_hit_sound_catalog() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    if std::env::var_os("CARGO_FEATURE_MEMORY").is_none() { return; }
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory missing"));
    let directory = root.join("assets/wav");
    let mut names: Vec<String> = std::fs::read_dir(&directory)
        .expect("Cannot read supplied hit sounds")
        .map(|entry| entry.expect("Cannot inspect supplied hit sound").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("wav")))
        .map(|path| {
            let bytes = std::fs::read(&path).expect("Cannot read supplied WAV");
            assert!(bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE", "Invalid supplied WAV: {}", path.display());
            path.file_name().expect("WAV filename missing").to_str().expect("WAV filename must be UTF-8").to_owned()
        }).collect();
    names.sort_by_cached_key(|name| name.to_lowercase());
    assert!(!names.is_empty(), "No supplied hit sounds. Restore assets/wav before building.");
    assert!(names.iter().any(|name| name == "minecraft_hit.wav"), "Default hit sound minecraft_hit.wav is missing. Restore it or explicitly update the default.");
    let mut source = String::from("pub const SOUNDS: &[(&str, &[u8])] = &[\n");
    for name in names {
        source.push_str(&format!("({name:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/wav/\", {name:?}))),\n"));
    }
    source.push_str("];\n");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory missing"));
    std::fs::write(output.join("hit_sound_catalog.rs"), source).expect("Cannot write embedded hit sound catalog");
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
