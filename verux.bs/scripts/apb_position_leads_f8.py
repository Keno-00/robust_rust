"""F8-controlled jump trace of explicit fields found in controlled movement captures."""

import argparse
import csv
import json
import math
import struct
import time
from pathlib import Path

from apb_f8_common import Session, client_capture, module_base, utc_now, wait_f8, write_json


def read_lead(session, base, lead):
    address = int(lead["address"], 16)
    offset = int(lead["position_offset"], 16)
    body = session.read(address, 0x100)
    actual_vtable = struct.unpack_from("<Q", body)[0]
    actual_marker = struct.unpack_from("<I", body, 0x3C)[0]
    expected_vtable = base + int(lead["vtable_rva"], 16)
    if actual_vtable != expected_vtable or actual_marker != lead["class_marker"]:
        raise RuntimeError(f"Lead {lead['address']} changed type: vtable 0x{actual_vtable:X} "
                           f"(expected 0x{expected_vtable:X}), marker {actual_marker} "
                           f"(expected {lead['class_marker']}); collect fresh movement evidence")
    xyz = struct.unpack_from("<fff", body, offset)
    if not all(math.isfinite(value) and abs(value) < 1_000_000 for value in xyz):
        raise RuntimeError(f"Invalid XYZ at {lead['address']}+{lead['position_offset']}: {xyz}")
    return xyz, body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--inspect-only", action="store_true",
                        help="Read both saved object headers immediately and report each result without a jump trace")
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    source = json.loads(args.manifest.read_text(encoding="utf-8"))
    if source.get("format") != "apb-controlled-position-leads-v1":
        raise RuntimeError("Source must be an explicit controlled-position-lead manifest")
    leads = source["candidates"]
    if not 1 <= len(leads) <= 32:
        raise RuntimeError("Expected 1..32 explicit position fields")
    keys = set()
    for lead in leads:
        offset = int(lead["position_offset"], 16)
        key = (lead["address"], lead["position_offset"])
        if key in keys or not 0x40 <= offset <= 0xF4 or offset % 4:
            raise RuntimeError(f"Invalid or duplicate field: {key}")
        keys.add(key)
        if not lead["headers_stable"] or lead["move_xy"] <= 0 or lead["return_xy"] >= lead["move_xy"]:
            raise RuntimeError(f"Lead {key} lacks stable type and controlled away/return evidence")
    session = Session(args.pid)
    try:
        if session.identity != source["identity"]:
            raise RuntimeError("Movement evidence belongs to another APB launch")
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {
            "format": "apb-position-leads-session-v1", "identity": session.identity,
            "module_base": f"0x{base:X}", "started_utc": utc_now(),
            "source_manifest": str(args.manifest), "candidates": leads,
            "scope": "explicit movement-derived fields; no automatic discovery or pawn classification",
        })
        if args.inspect_only:
            results = []
            for lead in leads:
                record = {"address": lead["address"], "position_offset": lead["position_offset"],
                          "expected_vtable_rva": lead["vtable_rva"],
                          "expected_marker": lead["class_marker"]}
                try:
                    body = session.read(int(lead["address"], 16), 0x100)
                    record.update(body_hex=body.hex(),
                                  actual_vtable=f"0x{struct.unpack_from('<Q', body)[0]:X}",
                                  actual_marker=struct.unpack_from("<I", body, 0x3C)[0])
                    xyz, _ = read_lead(session, base, lead)
                    record.update(position=xyz, status="saved_type_matches")
                except (OSError, RuntimeError) as error:
                    record.update(status="unavailable", error=str(error))
                results.append(record)
                print(json.dumps(record, allow_nan=False), flush=True)
            write_json(args.output_directory / "inspection.json", {
                "format": "apb-position-leads-inspection-v1", "identity": session.identity,
                "module_base": f"0x{base:X}", "source_manifest": str(args.manifest),
                "utc": utc_now(), "candidates": results,
            })
            if any(item["status"] != "saved_type_matches" for item in results):
                raise RuntimeError("Saved object type check failed; inspect inspection.json before tracing")
            return
        print("Targeted laptop jump trace. Keep the desktop still and APB foreground. "
              "F8 starts; jump twice on the laptop, then F8 on desktop ends after landing.", flush=True)
        for lead in leads:
            print(f"  {lead['address']} + {lead['position_offset']} "
                  f"marker {lead['class_marker']}", flush=True)
        wait_f8(session, "laptop ready to jump; F8 starts targeted trace")
        before = []
        for lead in leads:
            xyz, body = read_lead(session, base, lead)
            before.append({"address": lead["address"], "position_offset": lead["position_offset"],
                           "position": xyz, "body_hex": body.hex()})
        image_before = client_capture(session, args.output_directory / "before.png")
        print("TARGETED TRACE STARTED: jump twice on the laptop; F8 after landing ends.", flush=True)
        temporary = args.output_directory / "jump.tsv.partial"
        summaries = {key: {"minimum": None, "maximum": None, "first": None, "last": None} for key in keys}
        rounds = 0
        largest_gap = 0.0
        previous = None
        started = time.monotonic()
        last_progress = started
        with temporary.open("x", newline="", encoding="utf-8") as stream:
            writer = csv.writer(stream, delimiter="\t")
            writer.writerow(("elapsed_s", "address", "position_offset", "x", "y", "z"))
            while True:
                session.require_foreground()
                if session.user32.GetAsyncKeyState(0x77) & 0x8000:
                    while session.user32.GetAsyncKeyState(0x77) & 0x8000:
                        time.sleep(0.05)
                    break
                elapsed = time.monotonic() - started
                if previous is not None:
                    largest_gap = max(largest_gap, elapsed - previous)
                previous = elapsed
                for lead in leads:
                    xyz, _ = read_lead(session, base, lead)
                    key = (lead["address"], lead["position_offset"])
                    writer.writerow((f"{elapsed:.4f}", *key, *xyz))
                    summary = summaries[key]
                    if summary["first"] is None:
                        summary.update(first=xyz, minimum=list(xyz), maximum=list(xyz))
                    summary["last"] = xyz
                    for axis in range(3):
                        summary["minimum"][axis] = min(summary["minimum"][axis], xyz[axis])
                        summary["maximum"][axis] = max(summary["maximum"][axis], xyz[axis])
                rounds += 1
                if time.monotonic() - last_progress >= 2:
                    print(f"  Trace active: {rounds} rounds", flush=True)
                    last_progress = time.monotonic()
                time.sleep(0.1)
        if rounds < 5:
            raise RuntimeError("Fewer than five rounds; partial trace retained")
        temporary.replace(args.output_directory / "jump.tsv")
        image_after = client_capture(session, args.output_directory / "after.png")
        after = []
        for lead in leads:
            xyz, body = read_lead(session, base, lead)
            after.append({"address": lead["address"], "position_offset": lead["position_offset"],
                          "position": xyz, "body_hex": body.hex()})
        results = [{"address": key[0], "position_offset": key[1], **value,
                    "axis_ranges": [value["maximum"][i] - value["minimum"][i] for i in range(3)]}
                   for key, value in sorted(summaries.items())]
        write_json(args.output_directory / "jump.json", {
            "format": "apb-position-leads-jump-v1", "identity": session.identity,
            "source_manifest": str(args.manifest), "sample_rounds": rounds,
            "max_sample_gap_s": largest_gap, "before": before, "after": after,
            "image_before": image_before, "image_after": image_after,
            "candidates": results, "status": "unclassified_position_field_trace",
        })
        for result in results:
            print(f"  {result['address']}+{result['position_offset']}: "
                  f"XYZ ranges {result['axis_ranges']}", flush=True)
        if largest_gap > 0.3:
            raise RuntimeError(f"Sample gap {largest_gap:.3f}s exceeded 0.30s; inspect saved trace")
        if not any(result["axis_ranges"][2] > 0 for result in results):
            raise RuntimeError("Neither explicit field changed Z; inspect trace and jump timing")
        print(f"CAPTURE COMPLETE: {rounds} rounds. Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Targeted position trace interrupted; completed files retained")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Targeted position trace failed: {error}")
