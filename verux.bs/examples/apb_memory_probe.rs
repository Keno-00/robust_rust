//! Read-only native probe for the Memory coordinate source.
//! Run elevated while APB.exe is open; this never sends Arduino quill.
//! `--canvas-root PATH` explicitly reviews six saved identity captures offline.

use robust_rust::lattice_encoding::EncodedAddress;
use robust_rust::apb_atlas::{AtlasSource, EXPECTED_SHA256};
use robust_rust::apb_orchid::{kind, signature, u64_at, PawnRef, PAWN};
use robust_rust::apb_process::find_running_apb_pid;
use robust_rust::apb_grove::{read_roster, RosterRead};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

fn address(value: &Value) -> Result<u64, String> {
    let text = value
        .as_str()
        .ok_or("Missing hexadecimal address in capture")?;
    u64::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
}

fn review_captures(root: &Path) -> Result<(), String> {
    for index in 1..=6 {
        let directory = root.join(format!("identity-inspect-20260930-{index}"));
        let report: Value = serde_json::from_slice(
            &std::fs::read(directory.join("identity_inspection.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if report["identity"]["image_sha256"].as_str() != Some(EXPECTED_SHA256) {
            return Err(format!(
                "Capture {index} belongs to an unverified executable build"
            ));
        }
        let base = address(&report["module_base"])?;
        let records = report["objects"]
            .as_object()
            .ok_or("Capture has no object records")?;
        let mut bodies = BTreeMap::new();
        for record in records.values() {
            let object = address(&record["address"])?;
            let body = std::fs::read(directory.join(format!("object-{object:016X}.bin")))
                .map_err(|e| e.to_string())?;
            if body.len() < 64 {
                return Err(format!(
                    "Capture {index} object 0x{object:X} has a short header"
                ));
            }
            let expected_type = (
                address(&record["vtable_rva"])?,
                record["marker"].as_u64().ok_or("Missing object marker")? as u32,
            );
            let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let fingerprint = format!("{}:{}", hex(&body[..8]), hex(&body[0x24..0x40]));
            if kind(&body, base) != expected_type
                || record["instance_signature"].as_str() != Some(&fingerprint)
            {
                return Err(format!(
                    "Capture {index} object 0x{object:X} differs from its metadata"
                ));
            }
            bodies.insert(object, body);
        }
        let camera = report["camera"]["camera"]
            .as_u64()
            .ok_or("Missing captured camera")?;
        let camera_body = bodies.get(&camera).ok_or("Camera body is not captured")?;
        if camera_body.len() < 0x2E8 {
            return Err("Captured camera prefix does not include its World field".into());
        }
        let world_address = u64_at(camera_body, 0x2E0);
        let world_body = bodies
            .get(&world_address)
            .ok_or("World body is not captured")?;
        let world = PawnRef {
            address: EncodedAddress::new(world_address)?,
            signature: signature(world_body),
        };
        let mut reads = 0;
        let mut bytes = 0;
        let roster = read_roster(base, world, |at, output| {
            let body = bodies
                .get(&at)
                .ok_or_else(|| format!("Uncaptured node 0x{at:X}"))?;
            let saved = body
                .get(..output.len())
                .ok_or_else(|| format!("Short captured body at 0x{at:X}"))?;
            output.copy_from_slice(saved);
            reads += 1;
            bytes += output.len();
            Ok(())
        })?;
        let RosterRead::Complete(roster) = roster else {
            return Err(format!(
                "Saved capture {index} changed during offline replay"
            ));
        };
        let expected: HashSet<_> = bodies
            .iter()
            .filter(|(_, body)| kind(body, base) == PAWN)
            .map(|(&at, body)| Ok(PawnRef {
                address: EncodedAddress::new(at)?,
                signature: signature(body),
            }))
            .collect::<Result<HashSet<_>, String>>()?;
        let reached: HashSet<_> = roster.actors.iter().copied().collect();
        if reached != expected {
            return Err(format!(
                "Saved capture {index} rooted characters differ from the captured subset"
            ));
        }
        println!(
            "Capture {index}: head 0x{:X}, {} characters, {} vehicles, {} nodes, {reads} reads, {bytes} bytes; captured subset agrees.",
            roster.head, roster.actors.len(), roster.vehicles, roster.nodes,
        );
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if !arguments.is_empty() {
        if arguments.len() == 2 && arguments[0] == "--visibility-report" {
            return visibility_report(Path::new(&arguments[1]));
        }
        if arguments.len() != 2 || arguments[0] != "--capture-root" {
            return Err("Usage: apb_memory_probe [--capture-root PATH | --visibility-report NEW_JSON]".into());
        }
        return review_captures(Path::new(&arguments[1]));
    }
    let pid = find_running_apb_pid()?;
    let mut source = AtlasSource::connect(pid)?;
    let discovery = source.discovery();
    println!("PID {pid} base 0x{:X}: rooted head {:?}, {} characters, {} vehicles, {} nodes; waiting {:?}.",
        discovery.base.get(), discovery.head, discovery.actors.len(), discovery.vehicles,
        discovery.nodes, discovery.waiting_reason);
    println!(
        "actor addresses: {:?}",
        discovery
            .actors
            .iter()
            .map(|a| format!("0x{:X}", a.address.get()))
            .collect::<Vec<_>>()
    );
    if let Some(frame) = source.frame()? {
        println!(
            "Projected points: {}; foreground target: {:?}",
            frame.points.len(),
            frame.target(500, 500, 0, 0)
        );
    } else {
        println!("Current frame withheld; inspect verux_apb.log for the acquisition reason.");
    }
    Ok(())
}

fn visibility_report(path: &Path) -> Result<(), String> {
    use robust_rust::apb_prism;
    use robust_rust::atlas_reader::AtlasReader;
    use std::io::Write;
    let pid = find_running_apb_pid()?;
    // Connect enforces the APB image hash and guarded rooted roster. This mode
    // reads raw recency only; it never selects a target or sends quill.
    let source = AtlasSource::connect(pid)?;
    let discovery = source.discovery();
    let world = discovery.world_instance.ok_or_else(|| {
        format!("Visibility probe has no stable World: {:?}", discovery.waiting_reason)
    })?;
    let reader = AtlasReader::connect()?;
    let mut values = Vec::new();
    for &pawn in &discovery.actors {
        let value = apb_prism::sample(&reader, pid, discovery.base.get(), pawn, world)?;
        values.push((pawn, value));
    }
    let clock = apb_prism::world_clock(&reader, pid, discovery.base.get(), world)?;
    let mut rows = Vec::new();
    for (pawn, value) in values {
        let value = value.at_world_time(clock)?;
        rows.push(serde_json::json!({"pawn": format!("0x{:X}", pawn.address.get()),
            "last_render_time": value.last_render_time, "world_time": value.world_time,
            "age_seconds": value.age_seconds, "recently_rendered": value.recently_rendered}));
    }
    let report = serde_json::json!({"format": "apb-native-render-recency-v1", "pid": pid,
        "expected_image_sha256": EXPECTED_SHA256, "module_base": format!("0x{:X}", discovery.base.get()),
        "world": format!("0x{:X}", world.address.get()), "pawn_timestamp_offset": "0xB4",
        "world_clock_offset": "0x6A0", "threshold_seconds": apb_prism::RECENCY_SECONDS,
        "encoding": "little-endian float64", "pawns": rows,
        "limits": "Read-only native sampler; current render recency, not independent scene labels or instantaneous LOS."});
    let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("Create new visibility report {} failed: {e}", path.display()))?;
    output.write_all(&serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("Native visibility sampler: {} current pawns, World clock {clock:.6}; report {}.", rows.len(), path.display());
    Ok(())
}

fn main() {
    let result = run();
    if let Err(error) = result {
        eprintln!("APB Memory probe failed: {error}");
        std::process::exit(1);
    }
}
