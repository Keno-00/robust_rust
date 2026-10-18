"""Rank raw scalar leads from the explicit laptop health/stamina F8 phases.

No offset is promoted to runtime use. Values describe the first sample of each
operator-labelled phase; later burst samples expose changes during capture.
"""

import argparse
import hashlib
import json
import math
import statistics
import struct
from pathlib import Path

PHASES = (
    ("baseline_full", "Laptop: full HEALTH and STAMINA; stand still on foot"),
    ("health_damaged", "Laptop: take health damage; stamina FULL; F8 promptly before healing"),
    ("health_more_damaged", "Laptop: take MORE health damage; stay alive; stamina FULL; F8 promptly"),
    ("health_recovered", "Laptop: HEALTH and STAMINA fully recovered; stand still"),
    ("stamina_partial", "Laptop: partly deplete STAMINA using less-lethal damage (e.g. PIG); health FULL; F8 promptly"),
    ("stamina_low", "Laptop: take MORE less-lethal STAMINA damage; health FULL; F8 promptly"),
    ("stamina_recovered", "Laptop: HEALTH and STAMINA fully recovered; stand still"),
    ("health_repeat_damaged", "Laptop: take health damage AGAIN; stamina FULL; stay alive; F8 promptly"),
    ("health_repeat_recovered", "Laptop: HEALTH and STAMINA fully recovered again"),
    ("stamina_repeat_low", "Laptop: take less-lethal STAMINA damage AGAIN; health FULL; F8 promptly"),
    ("stamina_repeat_recovered", "Laptop: HEALTH and STAMINA fully recovered again"),
)
FORMATS = (("u8", "B", 1), ("u16", "H", 2), ("u32", "I", 4),
           ("i32", "i", 4), ("f32", "f", 4), ("f64", "d", 4))
FULL = (0, 3, 6, 8, 10)


def matches(values, resource):
    full = [values[i] for i in FULL]
    scale = max(1.0, max(abs(v) for v in values))
    tolerance = max(1e-5, scale * 0.002)
    baseline = statistics.median(full)
    if max(full) - min(full) > tolerance:
        return None
    changed = (1, 2, 7) if resource == "health" else (4, 5, 9)
    control = (4, 5, 9) if resource == "health" else (1, 2, 7)
    if any(abs(values[i] - baseline) > tolerance for i in control):
        return None
    if resource == "health":
        increasing = values[1] > baseline + tolerance and values[2] > values[1] + tolerance and values[7] > baseline + tolerance
        decreasing = values[1] < baseline - tolerance and values[2] < values[1] - tolerance and values[7] < baseline - tolerance
    else:
        increasing = values[4] > baseline + tolerance and values[5] > values[4] + tolerance and values[9] > baseline + tolerance
        decreasing = values[4] < baseline - tolerance and values[5] < values[4] - tolerance and values[9] < baseline - tolerance
    if not (increasing or decreasing):
        return None
    return {"direction": "increases_on_depletion" if increasing else "decreases_on_depletion",
            "relative_change": min(abs(values[i] - baseline) for i in changed) / scale,
            "tolerance": tolerance}


def review(directory):
    stages = []
    inputs = {}

    def read(path):
        raw = path.read_bytes()
        inputs[str(path.relative_to(directory))] = {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        return raw

    session = json.loads(read(directory / "session.json"))
    base = int(session["module_base"], 16)

    def body(record):
        raw = read(directory / record["path"])
        fingerprint = raw[:8].hex() + ":" + raw[0x24:0x40].hex()
        if (len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]
                or fingerprint != record["signature"]
                or struct.unpack_from("<Q", raw)[0] - base != int(record["vtable_rva"], 16)
                or struct.unpack_from("<I", raw, 0x3C)[0] != record["marker"]):
            raise ValueError(f"Raw object does not match its capture metadata: {record['path']}")
        return raw

    for name, _ in PHASES:
        path = directory / name / "stage.json"
        stage = json.loads(read(path))
        if stage["phase"] != name or stage["identity"] != session["identity"] or not stage["complete"]:
            raise ValueError(f"Incomplete or mismatched stage: {path}")
        stages.append(stage)
    subjects = set(stages[0]["samples"][0]["objects"])
    all_subjects = set(subjects)
    for stage in stages:
        subjects &= set(stage["samples"][0]["objects"])
        all_subjects |= set(stage["samples"][0]["objects"])
    excluded = [{"subject": subject, "reason": "not present in all first-sample phases"}
                for subject in sorted(all_subjects - subjects)]
    candidates = []
    checks = []
    for subject in sorted(subjects):
        records = [stage["samples"][0]["objects"][subject] for stage in stages]
        identities = {(r["address"], r["signature"], r["vtable_rva"], r["marker"]) for r in records}
        if len(identities) != 1:
            excluded.append({"subject": subject, "reason": "object instance changed across phases"})
            continue
        bodies = [body(r) for r in records]
        if len({len(b) for b in bodies}) != 1:
            raise ValueError(f"Capture lengths differ for {subject}")
        bursts = [[body(sample["objects"][subject])
                   for sample in stage["samples"]
                   if subject in sample["objects"]
                   and sample["objects"][subject]["signature"] == records[0]["signature"]]
                  for stage in stages]
        for encoding, code, stride in FORMATS:
            width = struct.calcsize("<" + code)
            for offset in range(0x40, len(bodies[0]) - width + 1, stride):
                values = [struct.unpack_from("<" + code, b, offset)[0] for b in bodies]
                if not all(math.isfinite(v) and abs(v) <= 1e7 for v in values):
                    continue
                # Tiny subnormal float interpretations of integers are not meaningful leads.
                if code in ("f", "d") and any(v != 0 and abs(v) < 1e-6 for v in values):
                    continue
                if subject == "laptop/pawn" and offset in (0x84, 0x4B4, 0x514) and encoding == "u32":
                    checks.append({"offset": hex(offset), "encoding": encoding, "values": values})
                for resource in ("health", "stamina"):
                    match = matches(values, resource)
                    if match is None:
                        continue
                    burst_values = [[struct.unpack_from("<" + code, b, offset)[0] for b in phase]
                                    for phase in bursts]
                    valid_burst = all(math.isfinite(v) for phase in burst_values for v in phase)
                    candidates.append({"resource": resource, "subject": subject,
                        "address": records[0]["address"], "signature": records[0]["signature"],
                        "vtable_rva": records[0]["vtable_rva"], "marker": records[0]["marker"],
                        "offset": hex(offset), "encoding": encoding, "values": values,
                        "burst_values": burst_values if valid_burst else None,
                        "unstable_during_burst": not valid_burst or any(
                            max(v) - min(v) > match["tolerance"] for v in burst_values if v),
                        **match})
    candidates.sort(key=lambda row: (row["resource"], row["unstable_during_burst"],
                                    row["subject"].startswith("local/"), -row["relative_change"],
                                    row["subject"], int(row["offset"], 16)))
    return {"status": "candidate_correlations_not_verified_health_or_stamina",
        "identity": session["identity"], "phases": [name for name, _ in PHASES],
        "target": session["target"], "subjects_compared": len(subjects),
        "excluded_subjects": excluded, "known_raw_checks": checks, "candidates": candidates,
        "capture_probe_failures": [{"phase": stage["phase"], "sample": sample["sequence"],
                                     "failures": sample["probe_failures"]}
                                    for stage in stages for sample in stage["samples"] if sample["probe_failures"]],
        "limitations": ["Laptop HUD states are operator labels, not values read from the laptop",
            "Local body is a control; it is not labelled with the laptop's actual resource state",
            "Raw probe extents are not verified object sizes; correlations may be neighboring data",
            "Packed, encoded, indirect or unreplicated values may not match scalar scanning",
            "First burst samples are compared; timestamps and burst variation are preserved",
            "No match is inconclusive; no reader or subject is silently substituted"], "inputs": inputs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Choose a new output path")
    result = review(args.directory)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"REVIEW COMPLETE: {len(result['candidates'])} raw candidates; inspect {args.output}")


if __name__ == "__main__":
    main()
