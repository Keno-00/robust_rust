"""One F8-paced EchoDriver check of a module-rooted camera pointer array."""

import argparse
import json
import struct
from pathlib import Path

from apb_camera_owner_context_f8 import region_at
from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


ENTRY_COUNT = 128


def camera_entries(session, table):
    raw = session.read(table, ENTRY_COUNT * 8)
    entries = []
    for index, (pointer,) in enumerate(struct.iter_unpack("<Q", raw)):
        if pointer < 0x10000 or pointer % 8 or pointer >= 0x800000000000:
            continue
        try:
            camera = camera_at(session, pointer)
            entries.append({"index": index, "pointer": f"0x{pointer:X}", "camera": camera})
        except (OSError, RuntimeError, ValueError):
            pass
    return raw, entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_references", type=Path)
    parser.add_argument("label")
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    source = json.loads(args.source_references.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-container-refs-f8-v1":
        raise RuntimeError("Source is not a camera-container reference report")
    page = int(source["page_base"], 16)
    anchor = f"0x{page + 0x80:X}"
    static = [hit for hit in source["references"]["hits"]
              if hit["target"] == anchor and hit["source_module_rva"] is not None]
    if len(static) != 1:
        raise RuntimeError(f"Expected one APB.exe pointer to array start {anchor}, found {len(static)}")
    rva = int(static[0]["source_module_rva"], 16)
    session = Session(args.pid)
    try:
        if source["identity"]["image_sha256"] != session.identity["image_sha256"]:
            raise RuntimeError("Module pointer hypothesis belongs to another APB build")
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        print(f"Reading APB.exe+0x{rva:X} and its first {ENTRY_COUNT} pointer entries "
              f"in {args.label}. Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, f"{args.label} module camera-table check")
        started = utc_now()
        module_address = base + rva
        module_window_before = session.read(module_address - 0x40, 0x100)
        (table,) = struct.unpack("<Q", session.read(module_address, 8))
        if table < 0x10000 or table % 8 or table >= 0x800000000000:
            raise RuntimeError(f"APB.exe+0x{rva:X} contains invalid table pointer 0x{table:X}")
        raw_before, entries_before = camera_entries(session, table)
        image = client_capture(session, args.output_directory / "view.png")
        (table_after,) = struct.unpack("<Q", session.read(module_address, 8))
        if table_after != table:
            raise RuntimeError(f"Module table pointer changed during capture: "
                               f"0x{table:X} -> 0x{table_after:X}")
        raw_after, entries_after = camera_entries(session, table)
        module_window_after = session.read(module_address - 0x40, 0x100)
        report = {"format": "apb-camera-table-root-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_references": str(args.source_references),
                  "source_same_launch": source["identity"] == session.identity,
                  "module_pointer_rva": f"0x{rva:X}", "module_pointer_address": f"0x{module_address:X}",
                  "module_window_before_hex": module_window_before.hex(),
                  "module_window_after_hex": module_window_after.hex(),
                  "table_address": f"0x{table:X}", "table_region": region_at(session, table),
                  "entry_count": ENTRY_COUNT, "table_before_hex": raw_before.hex(),
                  "table_after_hex": raw_after.hex(), "camera_entries_before": entries_before,
                  "camera_entries_after": entries_after, "image": image,
                  "status": "unclassified_module_camera_table"}
        write_json(args.output_directory / "table_root.json", report)
        print(f"CAPTURE COMPLETE: APB.exe+0x{rva:X} -> 0x{table:X}; "
              f"{len(entries_before)} camera-marker entries in first {ENTRY_COUNT} slots.", flush=True)
        for item in entries_before:
            print(f"  [{item['index']}] {item['pointer']} "
                  f"vertical {item['camera']['matrix'][11]:+.4f}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if not entries_before:
            raise RuntimeError("Module table had no camera-marker entries; inspect saved table_root.json")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Module camera-table check interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Module camera-table check failed: {error}")
