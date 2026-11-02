"""One F8-paced EchoDriver reverse-reference scan for a cross-district camera-pointer page."""

import argparse
import json
import struct
from pathlib import Path

from apb_f8_common import Session, camera_at, module_base, scan_pointer_refs, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("action_context", type=Path)
    parser.add_argument("social_context", type=Path)
    parser.add_argument("page_base", type=lambda value: int(value, 0))
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or args.page_base < 0x10000 or args.page_base % 4096):
        parser.error("Use a valid PID, aligned page base, and new output directory")
    action = json.loads(args.action_context.read_text(encoding="utf-8"))
    social = json.loads(args.social_context.read_text(encoding="utf-8"))
    if (action.get("format") != "apb-camera-owner-context-f8-v1"
            or social.get("format") != "apb-camera-owner-context-f8-v1"):
        raise RuntimeError("Both sources must be camera owner-context captures")
    if action["identity"] != social["identity"] or action["module_base"] != social["module_base"]:
        raise RuntimeError("Action and social contexts do not share one APB process identity")
    camera_slots = []
    for label, report in (("action", action), ("social", social)):
        matching = [item for item in report["slots"]
                    if item.get("points_to_target") or item.get("points_to_camera")]
        matching = [item for item in matching
                    if int(item["slot"], 16) & ~0xFFF == args.page_base]
        if len(matching) != 1:
            raise RuntimeError(f"Expected one live {label} camera slot in page "
                               f"0x{args.page_base:X}, found {len(matching)}")
        camera_slots.append({"state": label, "slot": matching[0]["slot"],
                             "camera": report["camera_before"]["address"]})
    if camera_slots[0]["slot"] == camera_slots[1]["slot"]:
        raise RuntimeError("Action and social camera slots did not change")
    social_camera = int(camera_slots[1]["camera"], 16)
    session = Session(args.pid)
    try:
        if action["identity"] != session.identity:
            raise RuntimeError("Saved contexts belong to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != action["module_base"]:
            raise RuntimeError("APB module base changed since context captures")
        args.output_directory.mkdir(parents=True)
        print(f"Tracing shared page 0x{args.page_base:X}; camera slots "
              f"{camera_slots[0]['slot']} -> {camera_slots[1]['slot']}. "
              "Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, "cross-district camera-container references")
        started = utc_now()
        page_before = session.read(args.page_base, 4096)
        social_offset = int(camera_slots[1]["slot"], 16) - args.page_base
        (pointer,) = struct.unpack_from("<Q", page_before, social_offset)
        if pointer != social_camera:
            raise RuntimeError(f"Social camera slot changed to 0x{pointer:X}")
        camera_before = camera_at(session, social_camera)
        targets = [args.page_base, args.page_base + 0x20, args.page_base + 0x80]
        refs = scan_pointer_refs(session, [f"0x{target:X}" for target in targets], base)
        page_after = session.read(args.page_base, 4096)
        (pointer_after,) = struct.unpack_from("<Q", page_after, social_offset)
        if pointer_after != social_camera:
            raise RuntimeError(f"Social camera slot changed during scan to 0x{pointer_after:X}")
        camera_after = camera_at(session, social_camera)
        report = {"format": "apb-camera-container-refs-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "action_context": str(args.action_context),
                  "social_context": str(args.social_context), "page_base": f"0x{args.page_base:X}",
                  "camera_slots": camera_slots, "targets": [f"0x{x:X}" for x in targets],
                  "page_before_hex": page_before.hex(), "page_after_hex": page_after.hex(),
                  "camera_before": camera_before, "camera_after": camera_after,
                  "references": refs, "status": "unclassified_container_references"}
        write_json(args.output_directory / "container_refs.json", report)
        static = [item for item in refs["hits"] if item["source_module_rva"] is not None]
        print(f"CAPTURE COMPLETE: {len(refs['hits'])} references to three page anchors, "
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
        raise SystemExit("Camera-container reference scan interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera-container reference scan failed: {error}")
