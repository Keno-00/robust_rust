"""Capture exactly one F8-paced APB camera-discovery state through EchoDriver."""

import argparse
import re
import sys
from pathlib import Path

from apb_f8_common import (
    CAMERA_CLASS, Session, capture_matrix_blocks, capture_table, client_capture,
    module_base, utc_now, wait_f8, write_json,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("label", help="Short capture state, e.g. action_level or action_up")
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--entries", type=int, default=65536,
                        help="Bounded table slots to inspect; default 65536")
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,47}", args.label)
            or not 32768 <= args.entries <= 131072):
        parser.error("Use a valid PID, a short lowercase label, a new output directory, and 32768..131072 entries")
    session = Session(args.pid)
    try:
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        print(f"Single-state camera discovery: {args.label}. APB PID {args.pid}, "
              f"module 0x{base:X}, scanning {args.entries} table slots.", flush=True)
        print("Set the requested camera view while standing still. F8 starts one capture; "
              "hold that view until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, args.label)
        started = utc_now()
        prefix = args.output_directory / args.label
        objects, table = capture_table(session, base, prefix, args.entries)
        matrix_blocks = capture_matrix_blocks(session, prefix)
        image = client_capture(session, Path(str(prefix) + ".png"))
        report = {
            "format": "apb-camera-single-f8-v1", "label": args.label,
            "started_utc": started, "completed_utc": utc_now(),
            "identity": session.identity, "module_base": f"0x{base:X}",
            "table": table, "matrix_blocks": matrix_blocks, "image": image,
            "known_marker_16920_addresses": [f"0x{address:X}" for address in objects[CAMERA_CLASS]],
            "status": "unclassified_camera_discovery_capture",
        }
        write_json(args.output_directory / "capture.json", report)
        print(f"CAPTURE COMPLETE: {args.label}; {table['readable_headers']} headers, "
              f"{matrix_blocks['records']} matrix blocks, "
              f"{len(objects[CAMERA_CLASS])} objects with the old camera marker.", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if not matrix_blocks["records"]:
            raise RuntimeError("No matrix blocks were readable; inspect the saved table and headers")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Single-state camera capture interrupted; partial output remains for diagnosis.", file=sys.stderr)
        raise SystemExit(1)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Single-state camera capture failed: {error}")
