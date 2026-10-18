"""One F8-paced EchoDriver reverse-reference scan for a verified camera owner."""

import argparse
import json
import struct
import sys
from pathlib import Path

from apb_f8_common import Session, camera_at, module_base, scan_pointer_refs, utc_now, wait_f8, write_json


# Observed in the post-restart owner-context capture for APB SHA-256
# 3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640.
OWNER_VTABLE_RVA = 0x2B95CF0
OWNER_CLASS = 16998
CAMERA_POINTER_OFFSET = 0x8


def owner_at(session, address, module):
    body = session.read(address, 0x80)
    vtable = struct.unpack_from("<Q", body, 0)[0]
    camera = struct.unpack_from("<Q", body, CAMERA_POINTER_OFFSET)[0]
    kind = struct.unpack_from("<I", body, 0x3C)[0]
    if vtable != module + OWNER_VTABLE_RVA or kind != OWNER_CLASS:
        raise RuntimeError(f"Owner 0x{address:X} signature changed: "
                           f"vtable 0x{vtable:X}, class {kind}")
    return {"address": f"0x{address:X}", "vtable": f"0x{vtable:X}",
            "class": kind, "camera_pointer": f"0x{camera:X}", "body_hex": body.hex()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("owner_context", type=Path)
    parser.add_argument("owner_address", type=lambda value: int(value, 0))
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or args.owner_address < 0x10000 or args.owner_address % 8):
        parser.error("Use a valid PID, aligned owner address, and new output directory")
    context = json.loads(args.owner_context.read_text(encoding="utf-8"))
    if context.get("format") != "apb-camera-owner-context-f8-v1":
        raise RuntimeError("Source is not a camera owner-context report")
    camera_address = int(context["camera_before"]["address"], 16)
    expected_slot = f"0x{args.owner_address + CAMERA_POINTER_OFFSET:X}"
    matching = [item for item in context["slots"] if item["slot"] == expected_slot
                and item.get("points_to_camera")]
    if len(matching) != 1:
        raise RuntimeError(f"Owner +0x8 is not one verified camera pointer slot: {expected_slot}")
    session = Session(args.pid)
    try:
        if context["identity"] != session.identity:
            raise RuntimeError("Owner context belongs to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != context["module_base"]:
            raise RuntimeError("APB module base changed since owner-context capture")
        args.output_directory.mkdir(parents=True)
        print(f"Tracing owner 0x{args.owner_address:X} with camera at +0x8. "
              "Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, "camera owner reverse-reference scan")
        started = utc_now()
        before = owner_at(session, args.owner_address, base)
        if int(before["camera_pointer"], 16) != camera_address:
            raise RuntimeError("Owner no longer points to the verified camera")
        camera_before = camera_at(session, camera_address)
        refs = scan_pointer_refs(session, [f"0x{args.owner_address:X}"], base)
        after = owner_at(session, args.owner_address, base)
        camera_after = camera_at(session, camera_address)
        if int(after["camera_pointer"], 16) != camera_address:
            raise RuntimeError("Owner changed camera pointer during reverse-reference scan")
        report = {"format": "apb-camera-owner-refs-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "owner_context": str(args.owner_context),
                  "owner_before": before, "owner_after": after,
                  "camera_before": camera_before, "camera_after": camera_after,
                  "references": refs, "status": "unclassified_owner_references"}
        write_json(args.output_directory / "owner_refs.json", report)
        static = [item for item in refs["hits"] if item["source_module_rva"] is not None]
        print(f"CAPTURE COMPLETE: {len(refs['hits'])} owner references, "
              f"{len(static)} in APB.exe, {refs['pages_failed']} failed pages.", flush=True)
        for item in static[:20]:
            print(f"  APB.exe+{item['source_module_rva']} -> {item['target']}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera owner reverse-reference scan interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera owner reverse-reference scan failed: {error}")
