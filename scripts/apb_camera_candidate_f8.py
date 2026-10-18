"""Capture one prepared view of a same-launch camera-object lead via EchoDriver."""

import argparse
import json
import re
import sys
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("marker_report", type=Path)
    parser.add_argument("address", type=lambda value: int(value, 0))
    parser.add_argument("label")
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,47}", args.label)):
        parser.error("Use a valid PID, a short lowercase label, and a new output directory")
    source = json.loads(args.marker_report.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-marker-f8-v1":
        raise RuntimeError("Camera candidate source is not a marker-scan report")
    matching = [item for item in source["leads"] if int(item["object"], 16) == args.address]
    if len(matching) != 1 or not matching[0]["finite"]:
        raise RuntimeError(f"0x{args.address:X} is not one finite candidate in the marker report")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Camera candidate belongs to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since marker capture")
        args.output_directory.mkdir(parents=True)
        print(f"Candidate 0x{args.address:X}; one {args.label} view capture. "
              "Stand still and hold the view until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, args.label)
        before = utc_now()
        camera = camera_at(session, args.address)
        image = client_capture(session, args.output_directory / "candidate.png")
        camera_after = camera_at(session, args.address)
        report = {"format": "apb-camera-candidate-f8-v1", "label": args.label,
                  "started_utc": before, "completed_utc": utc_now(),
                  "identity": session.identity, "module_base": f"0x{base:X}",
                  "marker_report": str(args.marker_report), "camera": camera,
                  "camera_after_image": camera_after, "image": image,
                  "status": "unclassified_camera_candidate"}
        write_json(args.output_directory / "capture.json", report)
        print(f"CAPTURE COMPLETE: {args.label}; vertical component "
              f"{camera['matrix'][11]:.4f}; camera position "
              f"{tuple(round(value, 2) for value in camera['position'])}.", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Candidate capture interrupted; completed files remain for diagnosis.", file=sys.stderr)
        raise SystemExit(1)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Camera candidate capture failed: {error}")
