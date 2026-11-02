"""One F8-paced EchoDriver capture of every camera-shape lead in a prepared view."""

import argparse
import json
import re
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("marker_report", type=Path)
    parser.add_argument("label")
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,47}", args.label)):
        parser.error("Use a valid PID, short lowercase label, and new output directory")
    source = json.loads(args.marker_report.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-marker-f8-v1":
        raise RuntimeError("Source is not a camera marker-scan report")
    addresses = sorted({int(value, 16) for value in source.get("shape_matches", [])})
    if not 1 <= len(addresses) <= 32:
        raise RuntimeError(f"Expected 1-32 saved camera-shape leads, found {len(addresses)}")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Marker report belongs to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since marker scan")
        args.output_directory.mkdir(parents=True)
        print(f"Comparing {len(addresses)} camera-shape leads in one {args.label} view. "
              "Hold the view until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, args.label)
        started = utc_now()
        before = []
        for address in addresses:
            try:
                before.append({"address": f"0x{address:X}", "camera": camera_at(session, address)})
            except (OSError, RuntimeError, ValueError) as error:
                before.append({"address": f"0x{address:X}", "error": str(error)})
        image = client_capture(session, args.output_directory / "view.png")
        after = []
        for address in addresses:
            try:
                after.append({"address": f"0x{address:X}", "camera": camera_at(session, address)})
            except (OSError, RuntimeError, ValueError) as error:
                after.append({"address": f"0x{address:X}", "error": str(error)})
        report = {"format": "apb-camera-candidate-set-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "marker_report": str(args.marker_report),
                  "label": args.label, "before_image": before, "after_image": after,
                  "image": image, "status": "unclassified_camera_candidates"}
        write_json(args.output_directory / "candidate_set.json", report)
        print(f"CAPTURE COMPLETE: {args.label}; {len(addresses)} leads read. "
              "Vertical matrix values:", flush=True)
        for item in before:
            if "camera" in item:
                print(f"  {item['address']}: {item['camera']['matrix'][11]:+.4f}", flush=True)
            else:
                print(f"  {item['address']}: {item['error']}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera candidate-set capture interrupted")
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Camera candidate-set capture failed: {error}")
