//! Read-only native probe for the Memory coordinate source.
//! Run elevated while APB.exe is open; this never sends Arduino input.

use robust_rust::apb_memory::ApbMemorySource;
use robust_rust::apb_process::find_running_apb_pid;

fn main() {
    let result = (|| -> Result<(), String> {
        let pid = find_running_apb_pid()?;
        let mut source = ApbMemorySource::connect(pid)?;
        let discovery = source.discovery();
        println!(
            "PID {} base 0x{:X} camera 0x{:X} local 0x{:X} link 0x{:X} actors {} headers {}/{} stale owners {}",
            pid,
            discovery.base,
            discovery.camera,
            discovery.local_position,
            discovery.local_link,
            discovery.actors.len(),
            discovery.readable_headers,
            discovery.failed_headers,
            discovery.skipped_owners,
        );
        println!(
            "actor addresses: {:?}",
            discovery
                .actors
                .iter()
                .map(|a| format!("0x{a:X}"))
                .collect::<Vec<_>>()
        );
        println!("foreground target: {:?}", source.sample(500, 500, 0, 0)?);
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("APB Memory probe failed: {error}");
        std::process::exit(1);
    }
}
