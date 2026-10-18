"""Record the unverified +0xD1C structure as a unit; never dereference it."""

import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

from apb_visibility_candidates import load
from apb_f8_common import utc_now, write_json


STRUCTURE_OFFSET = 0xD1C
STRUCTURE_SIZE = 16
PRIOR_SESSION = "20261003-101517-553-in-view"


def decode_structure(pawn_bytes):
    if len(pawn_bytes) < STRUCTURE_OFFSET + STRUCTURE_SIZE:
        raise RuntimeError("Selected pawn prefix does not contain +0xD1C..+0xD2B; inspect scope before recording")
    raw = bytes(pawn_bytes[STRUCTURE_OFFSET:STRUCTURE_OFFSET+STRUCTURE_SIZE])
    pointer, count, capacity = struct.unpack("<Qii", raw)
    pointer_plausible = 0x10000 <= pointer < 0x800000000000 and pointer % 8 == 0
    all_zero = pointer == count == capacity == 0
    return {"offset": hex(STRUCTURE_OFFSET), "raw_hex": raw.hex(), "pointer": hex(pointer),
            "count": count, "capacity": capacity, "all_zero": all_zero,
            "nonzero_pointer": pointer != 0,
            "layout_plausible": all_zero or (pointer_plausible and 0 <= count <= capacity <= 1_000_000),
            "interpretation": "unverified_pointer_count_capacity"}


def summarize(records, expected_clear):
    if not records:
        raise RuntimeError("No structure samples to summarize")
    return {"samples": len(records), "all_zero_samples": sum(r["all_zero"] for r in records),
            "implausible_layout_samples": sum(not r["layout_plausible"] for r in records),
            "nonzero_pointer_fraction": sum(r["nonzero_pointer"] for r in records)/len(records),
            "label_agreement_fraction": sum(r["layout_plausible"] and r["nonzero_pointer"] == expected_clear
                                             for r in records)/len(records),
            "counts": sorted({r["count"] for r in records}),
            "capacities": sorted({r["capacity"] for r in records}),
            "distinct_pointer_count": len({r["pointer"] for r in records})}


def review_structure(directory, output=None):
    scope, captures, evidence, unfinished, discarded = load(directory)
    if scope.get("protocol_name") != "observer-los":
        raise RuntimeError("Structure observer review requires the explicit observer-los protocol")
    reference = scope["fixed_target_reference"]
    tolerance = scope["movement_tolerances"]["target_position"]
    phases = []
    for capture in captures:
        folder = directory/capture["name"]
        raw = (folder/"raw.bin").read_bytes()
        rows = [json.loads(line) for line in (folder/"samples.jsonl").read_text(encoding="utf-8").splitlines()]
        records, exceptions = [], []
        max_target_delta = 0.0
        for row in rows:
            entry = next(r for r in row["regions"] if r["name"] == "pawn")
            body = raw[entry["raw_offset"]:entry["raw_offset"]+entry["size"]]
            value = decode_structure(body)
            if row.get("structure") != value:
                raise RuntimeError(f"Structure log/raw bytes differ in {capture['name']}, round {row['round']}")
            positions = [struct.unpack_from("<fff", body, 0x88), row["position"], row["ownership_position"]]
            if not all(math.isfinite(v) for xyz in positions for v in xyz):
                raise RuntimeError("Nonfinite fixed-target evidence")
            displacement = max(math.dist(xyz, reference) for xyz in positions)
            max_target_delta = max(max_target_delta, displacement)
            if displacement > tolerance:
                raise RuntimeError(f"Fixed-target guard violated in saved {capture['name']}")
            records.append(value)
            if not value["layout_plausible"] or value["nonzero_pointer"] != (capture["phase"]["observation"] == "clear"):
                exceptions.append({"round": row["round"], "utc": row["utc"], "session_s": row["session_s"],
                                   "pawn_read_start_s": entry["start_s"], "pawn_read_end_s": entry["end_s"],
                                   "camera": row["camera"], "structure": value})
        expected = capture["phase"]["observation"] == "clear"
        trimmed = [decode_structure(row) for row in capture["matrices"]["pawn"]]
        phases.append({"capture": capture["name"], "phase": capture["phase"],
                       "raw": summarize(records, expected), "trimmed": summarize(trimmed, expected),
                       "maximum_target_displacement": max_target_delta, "exceptions": exceptions})
    result = {"format": "apb-visibility-structure-observer-v1", "created_utc": utc_now(),
              "source_session": str(directory.resolve()), "identity": scope["identity"],
              "scope_sha256": hashlib.sha256((directory/"session-scope.json").read_bytes()).hexdigest(),
              "evidence": evidence, "unfinished_captures": unfinished, "discarded_captures": discarded,
              "prior_lead_session": PRIOR_SESSION, "structure_offset": hex(STRUCTURE_OFFSET),
              "fixed_target_reference": reference, "target_tolerance": tolerance,
              "status": "operator_label_correlation_only", "phases": phases,
              "method": "Predeclared nonzero-pointer hypothesis; no threshold fitted. Pointer/count/capacity recorded together. "
                        "Raw and trimmed label agreement both reported. Invalid layout counts as disagreement. "
                        "No pointer dereference or engine visibility claim."}
    write_json(output if output is not None else directory/"structure-review.json", result)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("session", type=Path)
    parser.add_argument("output", type=Path, help="New report path; existing reports are never replaced")
    args = parser.parse_args()
    result = review_structure(args.session.resolve(), args.output.resolve())
    print(json.dumps({"status": result["status"], "phases": len(result["phases"]),
                      "report": str(args.output.resolve())}, indent=2))
