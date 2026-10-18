"""Cross-session float64 recency review of saved pawn-relative evidence only."""

import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

import numpy as np

from apb_character_ownership import PAWN_TYPE, WORLD_TYPE
from apb_f8_common import utc_now, write_json
from apb_visibility_candidates import load


PAWN_WORLD = 0x2E0
PAWN_TIMESTAMP = 0xB4
WORLD_CLOCK = 0x6A0
TRAIN_SESSION = "20261003-101517-553-in-view"
VALIDATION_SESSION = "20261003-110406-602-observer-los"
RECENCY_SECONDS = 1.0  # Original proposed bound; never fitted on rechecks.
NEGATIVE_AGE_TOLERANCE = 0.25  # Sequential read/frame scheduling tolerance.


def scalar(matrix, offset, encoding="<f8"):
    body = np.ascontiguousarray(matrix)
    return np.ndarray((len(body),), dtype=encoding, buffer=body, offset=offset,
                      strides=(body.shape[1],)).copy()


def clock_fit(captures):
    discovery = [c for c in captures if c["phase"]["partition"] == "discovery"]
    if not discovery:
        raise RuntimeError("No discovery captures to check clock timing")
    times = np.concatenate([c["times"]["world"] for c in discovery])
    values = np.concatenate([scalar(c["matrices"]["world"], WORLD_CLOCK) for c in discovery])
    if not np.all(np.isfinite(values)) or not np.all(np.isfinite(times)):
        raise RuntimeError("Nonfinite float64 clock/timing evidence")
    centered = times-times.mean()
    slope = float(np.dot(centered, values-values.mean())/np.dot(centered, centered))
    intercept = float(values.mean()-slope*times.mean())
    checks = []
    for c in captures:
        actual = scalar(c["matrices"]["world"], WORLD_CLOCK)
        residual = float(np.max(np.abs(actual-(slope*c["times"]["world"]+intercept))))
        checks.append({"capture": c["name"], "phase": c["phase"]["name"],
                       "maximum_fit_residual_seconds": residual,
                       "passes": bool(0.9 <= slope <= 1.1 and residual <= 0.25)})
    return {"slope": slope, "intercept": intercept, "discovery_samples": len(times),
            "evaluation": checks, "all_checks_pass": all(c["passes"] for c in checks)}


def evaluate(timestamp, clock, skew, slope, clear):
    finite = np.isfinite(timestamp) & np.isfinite(clock)
    age = clock+slope*skew-timestamp
    valid = finite & (timestamp >= 0) & (age >= -NEGATIVE_AGE_TOLERANCE)
    predicts_clear = valid & (age <= RECENCY_SECONDS)
    # Invalid samples count as wrong for both operator labels.
    correct = valid & (predicts_clear == clear)
    change = np.diff(timestamp)
    return {"samples": len(timestamp), "correct_samples": int(np.count_nonzero(correct)),
            "correct_fraction": float(np.mean(correct)), "invalid_samples": int(np.count_nonzero(~valid)),
            "predicts_clear_samples": int(np.count_nonzero(predicts_clear)),
            "minimum_age_seconds": float(np.min(age)) if np.all(finite) else None,
            "maximum_age_seconds": float(np.max(age)) if np.all(finite) else None,
            "median_age_seconds": float(np.median(age)) if np.all(finite) else None,
            "timestamp_first": float(timestamp[0]), "timestamp_last": float(timestamp[-1]),
            "timestamp_changes": int(np.count_nonzero(np.abs(change) > 1e-6)),
            "timestamp_largest_backstep_seconds": float(max(0, -np.min(change))) if len(change) else 0,
            "clock_first": float(clock[0]), "clock_last": float(clock[-1])}


def matching_temporal_fields(captures, regions):
    """Search all byte offsets, independently of static thresholds/world clocks."""
    result, coverage = [], []
    clear = [c for c in captures if c["phase"]["observation"] == "clear"]
    blocked = [c for c in captures if c["phase"]["observation"] == "blocked"]
    if not clear or not blocked:
        return result, coverage
    for name in regions:
        width = min(c["matrices"][name].shape[1] for c in captures)
        for encoding, size in (("<f4", 4), ("<f8", 8)):
            mask = np.arange(width-size+1) >= 0x40
            invalid_fields = np.zeros(len(mask), dtype=bool)
            for c in clear+blocked:
                m = np.ascontiguousarray(c["matrices"][name])
                # Reinterpreting arbitrary bytes can yield NaNs/overflow. Exclude
                # these explicitly and report how many columns were affected.
                with np.errstate(invalid="ignore", over="ignore"):
                    a = np.ndarray((len(m), width-size+1), dtype=encoding, buffer=m,
                                   strides=(m.shape[1], 1)).astype(np.float64)
                    finite = np.all(np.isfinite(a), axis=0)
                    slope = (a[-1]-a[0])/(c["times"][name][-1]-c["times"][name][0])
                    changes = np.diff(a, axis=0)
                    nondecreasing = np.all(changes >= -0.05, axis=0)
                invalid_fields |= ~finite
                if c["phase"]["observation"] == "clear":
                    mask &= finite & (a[0] >= 0) & (slope >= 0.8) & (slope <= 1.2) & nondecreasing
                else:
                    mask &= finite & (a[0] >= 0) & (np.abs(slope) <= 0.05) & nondecreasing
            offsets = [hex(int(i)) for i in np.flatnonzero(mask)]
            coverage.append({"region": name, "encoding": encoding, "all_byte_offsets_from": "0x40",
                             "last_offset": hex(width-size), "tested_offsets": width-size+1-0x40,
                             "nonfinite_columns_excluded": int(np.count_nonzero(invalid_fields)), "matching_offsets": offsets})
            result.extend({"region": name, "encoding": encoding, "offset": off,
                           "pattern": "Advances near 1 second/second in every clear phase, holds in every blocked phase"}
                          for off in offsets)
    return result, coverage


def review_session(root):
    scope, captures, evidence, unfinished, discarded = load(root)
    if not captures:
        return None
    fit = clock_fit(captures)
    base = int(scope["module_base"], 16)
    phases = []
    for c in captures:
        clear = c["phase"]["observation"] == "clear"
        trimmed = evaluate(scalar(c["matrices"]["pawn"], PAWN_TIMESTAMP),
                           scalar(c["matrices"]["world"], WORLD_CLOCK),
                           c["times"]["pawn"]-c["times"]["world"], fit["slope"], clear)
        rows = [json.loads(line) for line in (root/c["name"]/"samples.jsonl").read_text(encoding="utf-8").splitlines()]
        raw = (root/c["name"]/"raw.bin").read_bytes()
        timestamps, clocks, skews = [], [], []
        for row in rows:
            parts = {r["name"]: r for r in row["regions"]}
            pawn, world = parts["pawn"], parts["world"]
            po, wo = pawn["raw_offset"], world["raw_offset"]
            if (struct.unpack_from("<Q", raw, po)[0]-base, struct.unpack_from("<I", raw, po+0x3C)[0]) != PAWN_TYPE:
                raise RuntimeError("Raw timestamp owner is not the exact pawn type")
            if (struct.unpack_from("<Q", raw, wo)[0]-base, struct.unpack_from("<I", raw, wo+0x3C)[0]) != WORLD_TYPE:
                raise RuntimeError("Raw clock owner is not the exact World type")
            expected_world = next(r["address"] for r in scope["regions"] if r["name"] == "world")
            if struct.unpack_from("<Q", raw, po+PAWN_WORLD)[0] != expected_world:
                raise RuntimeError("Pawn +0x2E0 does not reference the recorded World clock owner")
            timestamps.append(struct.unpack_from("<d", raw, po+PAWN_TIMESTAMP)[0])
            clocks.append(struct.unpack_from("<d", raw, wo+WORLD_CLOCK)[0])
            skews.append((pawn["start_s"]+pawn["end_s"]-world["start_s"]-world["end_s"])/2)
        untrimmed = evaluate(np.array(timestamps), np.array(clocks), np.array(skews), fit["slope"], clear)
        phases.append({"capture": c["name"], "phase": c["phase"], "raw": untrimmed, "trimmed": trimmed})
    return {"session": root.name, "source": str(root.resolve()), "identity": scope["identity"],
            "pawn": scope["selected_pawn"]["address"], "pawn_signature": next(r["signature"] for r in scope["regions"] if r["name"] == "pawn"),
            "protocol": scope.get("protocol_name", "target-motion"), "clock_fit": fit,
            "scope_sha256": hashlib.sha256((root/"session-scope.json").read_bytes()).hexdigest(),
            "evidence": evidence, "unfinished_captures": unfinished, "discarded_captures": discarded,
            "phases": phases, "raw_world_reference_checks": sum(p["raw"]["samples"] for p in phases)}


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_root", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    args.capture_root, args.output_directory = args.capture_root.resolve(), args.output_directory.resolve()
    if args.output_directory.exists() or not args.output_directory.parent.is_dir():
        parser.error("Use a new output directory with an existing parent")
    args.output_directory.mkdir()
    sessions, skipped = [], []
    for root in sorted(args.capture_root.glob("20261003-*")):
        if not root.is_dir() or not (root/"session-scope.json").exists():
            continue
        entry = review_session(root)
        if entry is None:
            skipped.append({"session": root.name, "reason": "No completed captures"})
        else:
            sessions.append(entry)
    names = {s["session"] for s in sessions}
    if not {TRAIN_SESSION, VALIDATION_SESSION} <= names:
        raise RuntimeError("Required training/held-out sessions missing")
    if len({s["identity"]["image_sha256"] for s in sessions}) != 1:
        raise RuntimeError("Mixed executable builds; do not combine relative offsets")
    temporal = []
    for name in (TRAIN_SESSION, VALIDATION_SESSION):
        scope, captures, *_ = load(args.capture_root/name)
        fields, coverage = matching_temporal_fields(captures, [r["name"] for r in scope["regions"] if r["name"] != "world"])
        temporal.append({"session": name, "matches": fields, "coverage": coverage})
    all_phases = [p for s in sessions for p in s["phases"]]
    raw_samples = sum(p["raw"]["samples"] for p in all_phases)
    trimmed_samples = sum(p["trimmed"]["samples"] for p in all_phases)
    report = {"format": "apb-visibility-float64-recency-cross-session-v1", "created_utc": utc_now(),
              "method": "Relative offsets and exact types across missions/instances. Independent float32/float64 temporal pattern scan at every byte offset. "
                        "Original <=1-second recency bound fixed; no label-based threshold fitting. Discovery-only per-session clock slope corrects measured sequential-read skew.",
              "status": "last_render_time_like_candidate_not_engine_identity_proof",
              "layout": {"pawn_type": [hex(PAWN_TYPE[0]), PAWN_TYPE[1]], "world_type": [hex(WORLD_TYPE[0]), WORLD_TYPE[1]],
                         "pawn_to_world_pointer_offset": hex(PAWN_WORLD), "pawn_timestamp_offset": hex(PAWN_TIMESTAMP),
                         "world_clock_offset": hex(WORLD_CLOCK), "numeric_encoding": "little-endian float64",
                         "threshold_seconds": RECENCY_SECONDS, "negative_age_tolerance_seconds": NEGATIVE_AGE_TOLERANCE},
              "training_session": TRAIN_SESSION, "held_out_observer_session": VALIDATION_SESSION,
              "sessions": sessions, "skipped_sessions": skipped, "temporal_search": temporal,
              "totals": {"sessions": len(sessions), "pawn_instances": len({(s["pawn"].lower(), s["pawn_signature"]) for s in sessions}),
                         "complete_phases": len(all_phases), "raw_samples": raw_samples, "trimmed_samples": trimmed_samples,
                         "raw_correct": sum(p["raw"]["correct_samples"] for p in all_phases),
                         "trimmed_correct": sum(p["trimmed"]["correct_samples"] for p in all_phases),
                         "all_clock_fits_pass": all(s["clock_fit"]["all_checks_pass"] for s in sessions),
                         "all_complete_raw_samples_match_operator_labels": all(p["raw"]["correct_fraction"] == 1 for p in all_phases)},
              "limitations": ["No native engine property name or timestamp producer identified.",
                              "Recency can lag an occlusion transition; this is not an instantaneous geometric ray trace.",
                              "Operator labels and endpoint screenshots do not prove every intervening scene frame.",
                              "Incomplete/discarded captures are retained and excluded; one executable build and APB launch are represented.",
                              "No aiming/product predicate enabled; no live process reads in this analysis."]}
    write_json(args.output_directory/"recency-evidence.json", report)
    print(json.dumps({"totals": report["totals"], "temporal_matches": [{"session": t["session"], "matches": t["matches"]} for t in temporal],
                      "report": str(args.output_directory/"recency-evidence.json")}, indent=2))


if __name__ == "__main__":
    run()
