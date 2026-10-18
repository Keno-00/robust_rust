"""One F8-paced EchoDriver pointer-reference scan for a verified same-launch camera lead."""

import argparse
import json
import sys
from pathlib import Path

from apb_f8_common import Session, camera_at, module_base, scan_pointer_refs, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("candidate_capture", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("candidate_address", nargs="?", type=lambda value: int(value, 0))
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    source = json.loads(args.candidate_capture.read_text(encoding="utf-8"))
    if source.get("format") == "apb-camera-candidate-f8-v1":
        address = int(source["camera"]["address"], 16)
        if args.candidate_address is not None and args.candidate_address != address:
            raise RuntimeError("Explicit address differs from the single candidate capture")
    elif source.get("format") == "apb-camera-candidate-set-f8-v1":
        if args.candidate_address is None:
            raise RuntimeError("A candidate-set capture requires an explicit camera address")
        matching = [item for item in source["before_image"]
                    if item["address"] == f"0x{args.candidate_address:X}" and "camera" in item]
        matching_after = [item for item in source["after_image"]
                          if item["address"] == f"0x{args.candidate_address:X}" and "camera" in item]
        if len(matching) != 1 or len(matching_after) != 1:
            raise RuntimeError("Explicit camera address is not valid in both candidate-set reads")
        address = args.candidate_address
    else:
        raise RuntimeError("Source is not a camera-candidate capture")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Camera candidate belongs to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since candidate capture")
        args.output_directory.mkdir(parents=True)
        print(f"Pointer references to camera candidate 0x{address:X}. Keep APB foreground "
              "until CAPTURE COMPLETE; any steady view is fine.", flush=True)
        wait_f8(session, "camera pointer-reference scan")
        started = utc_now()
        before = camera_at(session, address)
        refs = scan_pointer_refs(session, [f"0x{address:X}"], base)
        after = camera_at(session, address)
        report = {"format": "apb-camera-refs-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "candidate_capture": str(args.candidate_capture),
                  "candidate_address": f"0x{address:X}",
                  "camera_before": before, "camera_after": after,
                  "references": refs, "status": "unclassified_pointer_references"}
        write_json(args.output_directory / "references.json", report)
        static = [item for item in refs["hits"] if item["source_module_rva"] is not None]
        print(f"CAPTURE COMPLETE: {len(refs['hits'])} exact references, "
              f"{len(static)} in APB.exe, {refs['pages_failed']} failed pages.", flush=True)
        for item in static[:20]:
            print(f"  APB.exe+{item['source_module_rva']} -> {item['target']}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if not refs["hits"]:
            raise RuntimeError("No exact references in readable writable pages; inspect saved coverage")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Camera reference scan interrupted; completed files remain for diagnosis.", file=sys.stderr)
        raise SystemExit(1)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Camera reference scan failed: {error}")
