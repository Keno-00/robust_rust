"""One F8-paced EchoDriver snapshot around exact camera-chain pointer slots."""

import argparse
import ctypes
import hashlib
import json
import struct
import sys
from pathlib import Path

from apb_cursor_scan import MemoryBasicInformation
from apb_camera_chain_refs_f8 import validate_link
from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


def region_at(session, address):
    info = MemoryBasicInformation()
    size = session.kernel32.VirtualQueryEx(session.process, ctypes.c_void_p(address),
                                           ctypes.byref(info), ctypes.sizeof(info))
    if size != ctypes.sizeof(info):
        raise RuntimeError(f"VirtualQueryEx failed for pointer slot 0x{address:X}")
    return {"base": f"0x{info.base:X}", "allocation_base": f"0x{info.allocation_base:X}",
            "size": info.region_size, "state": info.state,
            "protection": info.protect, "kind": info.kind}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_references", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    source = json.loads(args.source_references.read_text(encoding="utf-8"))
    if source.get("format") == "apb-camera-refs-f8-v1":
        camera_address = int(source["camera_before"]["address"], 16)
        target_address = camera_address
        links = []
    elif source.get("format") == "apb-camera-chain-refs-f8-v1":
        camera_address = int(source["camera_address"], 16)
        target_address = int(source["object_before"]["object"], 16)
        links = source["links"]
    else:
        raise RuntimeError("Source is not a camera or camera-chain reference report")
    slots = sorted({int(hit["source"], 16) for hit in source["references"]["hits"]})
    if not slots:
        raise RuntimeError("Source report contains no pointer slots")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Reference report belongs to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since reference capture")
        args.output_directory.mkdir(parents=True)
        print(f"Owner context for {len(slots)} pointers to 0x{target_address:X}. "
              "Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, "camera-chain pointer-owner context")
        started = utc_now()
        for link in links:
            validate_link(session, base, link)
        before = camera_at(session, camera_address)
        slot_records = []
        page_addresses = set()
        active_count = 0
        for slot in slots:
            record = {"slot": f"0x{slot:X}", "region": region_at(session, slot)}
            try:
                (target,) = struct.unpack("<Q", session.read(slot, 8))
                record["target"] = f"0x{target:X}"
                record["points_to_target"] = target == target_address
                if target_address == camera_address:
                    record["points_to_camera"] = target == camera_address
                active_count += target == target_address
            except (OSError, RuntimeError) as error:
                record["read_error"] = str(error)
            slot_records.append(record)
            page = slot & ~0xFFF
            for delta in (-0x2000, -0x1000, 0, 0x1000):
                if page + delta >= 0x10000:
                    page_addresses.add(page + delta)
        pages = []
        for address in sorted(page_addresses):
            item = {"address": f"0x{address:X}"}
            try:
                data = session.read(address, 4096)
                name = f"page-{address:016X}.bin"
                (args.output_directory / name).write_bytes(data)
                item.update({"file": name, "sha256": hashlib.sha256(data).hexdigest()})
            except (OSError, RuntimeError) as error:
                item["read_error"] = str(error)
            pages.append(item)
        image = client_capture(session, args.output_directory / "view.png")
        for link in links:
            validate_link(session, base, link)
        after = camera_at(session, camera_address)
        report = {"format": "apb-camera-owner-context-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_references": str(args.source_references),
                  "target_address": f"0x{target_address:X}", "validated_links": links,
                  "camera_before": before, "camera_after": after, "slots": slot_records,
                  "pages": pages, "image": image, "status": "unclassified_owner_context"}
        write_json(args.output_directory / "context.json", report)
        print(f"CAPTURE COMPLETE: {active_count}/{len(slots)} slots still point to target, "
              f"{sum('file' in page for page in pages)}/{len(pages)} context pages read.", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if not active_count:
            raise RuntimeError("No source slots still point to this chain target; inspect context report")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera pointer-owner context interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera pointer-owner context failed: {error}")
