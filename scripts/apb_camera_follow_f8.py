"""One F8-paced EchoDriver check of camera pointer slots across a district change."""

import argparse
import json
import re
import struct
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_references", type=Path)
    parser.add_argument("label")
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,47}", args.label)):
        parser.error("Use a valid PID, short lowercase label, and new output directory")
    source = json.loads(args.source_references.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-refs-f8-v1":
        raise RuntimeError("Source is not a camera-reference capture")
    slots = sorted({int(hit["source"], 16) for hit in source["references"]["hits"]})
    if not slots:
        raise RuntimeError("Source contains no camera pointer slots")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Camera pointer slots belong to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since source reference capture")
        args.output_directory.mkdir(parents=True)
        print(f"Checking {len(slots)} source camera pointer slots in {args.label}. "
              "Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, f"{args.label} camera pointer-slot check")
        started = utc_now()
        seen = []
        cameras = {}
        for slot in slots:
            item = {"slot": f"0x{slot:X}"}
            try:
                (target,) = struct.unpack("<Q", session.read(slot, 8))
                item["target"] = f"0x{target:X}"
                if target and target not in cameras:
                    try:
                        cameras[target] = camera_at(session, target)
                    except (OSError, RuntimeError, ValueError) as error:
                        item["target_error"] = str(error)
            except (OSError, RuntimeError, ValueError) as error:
                item["slot_error"] = str(error)
            seen.append(item)
        image = client_capture(session, args.output_directory / "view.png")
        after = {}
        for address in cameras:
            try:
                after[f"0x{address:X}"] = camera_at(session, address)
            except (OSError, RuntimeError, ValueError) as error:
                after[f"0x{address:X}"] = {"error": str(error)}
        report = {"format": "apb-camera-follow-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_references": str(args.source_references),
                  "label": args.label,
                  "slots": seen, "camera_candidates": [cameras[key] for key in sorted(cameras)],
                  "camera_candidates_after_image": after, "image": image,
                  "status": "unclassified_pointer_slots"}
        write_json(args.output_directory / "follow.json", report)
        print(f"CAPTURE COMPLETE: {len(slots)} slots, {len(cameras)} camera-marker targets.", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera pointer-slot check interrupted")
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Camera pointer-slot check failed: {error}")
