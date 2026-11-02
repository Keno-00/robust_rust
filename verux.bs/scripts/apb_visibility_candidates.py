"""Offline candidate filtering for visibility captures; results remain hypotheses."""

import argparse
import hashlib
import json
import math
from pathlib import Path

import numpy as np

from apb_manual_labels import instance_signature
from apb_f8_common import write_json


MIN_SCORE = 0.95


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(directory):
    scope = json.loads((directory / "session-scope.json").read_text(encoding="utf-8"))
    if scope["format"] != "apb-visibility-discovery-v2":
        raise ValueError("Expected visibility discovery v2; use the legacy review for v1 sessions")
    regions = scope["regions"]
    captures, evidence = [], []
    previous_time = -math.inf
    for folder in sorted(directory.glob("capture-*")):
        manifest_path = folder / "capture.json"
        if not manifest_path.exists():
            continue  # Unfinished evidence is explicitly listed below, never fitted.
        meta = json.loads(manifest_path.read_text(encoding="utf-8"))
        if meta["status"] != "completed_operator_confirmed_capture":
            continue
        if meta["identity"] != scope["identity"]:
            raise RuntimeError(f"Another process launch in {folder}")
        raw_path, rows_path = folder / "raw.bin", folder / "samples.jsonl"
        if digest(raw_path) != meta["raw_sha256"] or digest(rows_path) != meta["samples_sha256"]:
            raise RuntimeError(f"Capture hashes differ: {folder}")
        raw = raw_path.read_bytes()
        rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines()]
        if len(rows) != meta["rounds"] or len(rows) < 2:
            raise RuntimeError(f"Invalid sample count: {folder}")
        position = 0
        matrices, clocks = {r["name"]: [] for r in regions}, {r["name"]: [] for r in regions}
        for index, row in enumerate(rows):
            if row["round"] != index or row["observation"] != meta["phase"]["observation"]:
                raise RuntimeError(f"Changed label/sample sequence: {folder}")
            if [(r["name"], r["size"]) for r in row["regions"]] != [(r["name"], r["size"]) for r in regions]:
                raise RuntimeError(f"Changed layout: {folder}")
            if not all(math.isfinite(row[key]) for key in ('elapsed_s', 'session_s', 'completed_s')):
                raise RuntimeError(f'Nonfinite sample timing: {folder}')
            for region, entry in zip(regions, row["regions"]):
                if not all(math.isfinite(entry[key]) for key in ('start_s', 'end_s')):
                    raise RuntimeError(f'Nonfinite region timing: {folder}')
                if entry["raw_offset"] != position or entry["end_s"] < entry["start_s"]:
                    raise RuntimeError(f"Invalid byte/timing range: {folder}")
                body = raw[position:position+region["size"]]
                if len(body) != region["size"] or instance_signature(body) != region["signature"]:
                    raise RuntimeError(f"Invalid instance/range: {folder}/{region['name']}")
                matrices[region["name"]].append(np.frombuffer(body, dtype=np.uint8))
                clocks[region["name"]].append((entry["start_s"]+entry["end_s"])/2)
                position += region["size"]
        if position != len(raw):
            raise RuntimeError(f"Unreferenced raw bytes: {folder}")
        times = [entry['start_s'] for row in rows for entry in row['regions']]
        if any(t < previous_time for t in times[:1]) or np.any(np.diff(times) < 0):
            raise RuntimeError(f'Non-increasing session-wide region timing: {folder}')
        previous_time = times[-1]
        elapsed = np.array([row["elapsed_s"] for row in rows])
        if np.any(np.diff(elapsed) <= 0):
            raise RuntimeError(f"Non-increasing timestamps: {folder}")
        keep = ((elapsed >= elapsed[0]+scope["trim_seconds"])
                & (elapsed <= elapsed[-1]-scope["trim_seconds"]))
        if keep.sum() < 2:
            raise RuntimeError(f"Not enough samples after trimming: {folder}")
        captures.append({"name": folder.name, "phase": meta["phase"],
            "matrices": {name: np.array(values)[keep] for name, values in matrices.items()},
            "times": {name: np.array(values)[keep] for name, values in clocks.items()}})
        evidence.append({"capture": folder.name, "phase": meta["phase"]["name"],
            "raw_sha256": meta["raw_sha256"], "samples_sha256": meta["samples_sha256"],
            "capture_sha256": digest(manifest_path), "kept_samples": int(keep.sum())})
    excluded = [{"capture": folder.name, "reason": json.loads((folder / 'capture.json').read_text(encoding='utf-8')).get('reason')}
                for folder in sorted(directory.glob('capture-*')) if (folder / 'capture.json').exists()
                and json.loads((folder / 'capture.json').read_text(encoding='utf-8')).get('status') != 'completed_operator_confirmed_capture']
    unfinished = [folder.name for folder in sorted(directory.glob("capture-*"))
                  if not (folder / "capture.json").exists()]
    return scope, captures, evidence, unfinished, excluded


def scalar_array(capture, region):
    matrix = capture["matrices"][region]
    return np.ascontiguousarray(matrix[:, :matrix.shape[1]//4*4]).view("<f4").astype(np.float64)


def evaluate(captures, values, predict, negative):
    result = []
    for capture in captures:
        observation = capture["phase"]["observation"]
        field_values = values[capture['name']]
        finite = np.isfinite(field_values)
        predicted = predict(field_values)
        expected = True if observation == "clear" else False if observation == negative else None
        result.append({"capture": capture["name"], "phase": capture["phase"]["name"],
            "partition": capture["phase"]["partition"], "observation": observation,
            "predicted_clear_fraction": float(np.mean(predicted)),
            "correct_fraction": float(np.mean(finite & (predicted == expected))) if expected is not None else None,
            "invalid_samples": int(np.count_nonzero(~finite)), "samples": len(predicted)})
    return result


def summarize_evaluation(rows, negative):
    comparison = [r["correct_fraction"] for r in rows
                  if r["partition"] != "discovery" and r["correct_fraction"] is not None]
    rechecks = {r["observation"] for r in rows if r["partition"] == "recheck"}
    return {"minimum_control_or_recheck_score": min(comparison) if comparison else None,
            "all_comparisons_pass": ({"clear", negative} <= rechecks
                and any(r["partition"] == "control" for r in rows)
                and bool(comparison) and min(comparison) >= MIN_SCORE),
            "recheck_observations_present": sorted(rechecks)}


def threshold(clear, negative):
    """Find a separated training range only; do not tune on control/recheck data."""
    if not np.all(np.isfinite(clear)) or not np.all(np.isfinite(negative)):
        return None
    cmin, cmax, nmin, nmax = min(clear), max(clear), min(negative), max(negative)
    if cmax < nmin:
        return {"cutoff": float((cmax+nmin)/2), "clear_when": "less", "discovery_gap": float(nmin-cmax)}
    if nmax < cmin:
        return {"cutoff": float((nmax+cmin)/2), "clear_when": "greater", "discovery_gap": float(cmin-nmax)}
    return None


def classification_report(captures, values, negative, rule):
    predict = (lambda v: np.isfinite(v) & (v < rule["cutoff"])) if rule["clear_when"] == "less" \
        else (lambda v: np.isfinite(v) & (v > rule["cutoff"]))
    rows = evaluate(captures, values, predict, negative)
    return {**rule, "negative_observation": negative, "evaluation": rows, **summarize_evaluation(rows, negative)}


def review(directory):
    scope, captures, evidence, unfinished, excluded = load(directory)
    discovery = [c for c in captures if c["phase"]["partition"] == "discovery"]
    observations = {c["phase"]["observation"] for c in discovery}
    report = {"format": "apb-visibility-candidates-v2", "identity": scope["identity"],
        "source_session": str(directory.resolve()), "evidence": evidence, "unfinished_captures": unfinished, "discarded_captures": excluded,
        "status": "operator_labeled_hypotheses_only", "scene_scope": scope.get("scene_scope", "in_view_clear_vs_opaque_wall"),
        "protocol_name": scope.get("protocol_name", "target-motion"), "bits": [], "scalars": [], "clocks": [], "timestamps": [],
        "scope_sha256": digest(directory / 'session-scope.json'),
        "search_limits": {"state_fields": "individual bits and aligned float32; verified pawn XYZ excluded",
            "timestamp_fields": "aligned nonnegative float32, discovery nondecreasing within 0.05s; clear age <=1s, all discovery ages -0.25..3600s",
            "clock_fields": "world aligned float32; slope 0.9..1.1, residual <=0.25s",
            "comparison_pass_fraction": MIN_SCORE},
        "method": "Discovery ranges only. Clock fit and thresholds fixed before control/recheck evaluation. "
                  "Only in-view clear versus opaque-wall blocked fits are in scope. No scene recognition or automatic product targeting."}
    if not {"clear", "blocked"} <= observations:
        report["counts"] = {"bits": 0, "scalars": 0, "clocks": 0, "timestamps": 0}
        report["note"] = "Need both clear and blocked discovery captures before filtering"
        return report
    for region in scope["regions"]:
        name = region["name"]
        # A world's own bits are not target-specific state candidates. Its floats
        # remain eligible clock leads. Ignore immutable header identity bytes.
        if name == "world":
            continue
        clear_matrix = np.concatenate([c['matrices'][name] for c in discovery
                                       if c['phase']['observation'] == 'clear'])
        allowed = np.arange(region['size']) >= 0x40
        if name == 'pawn':
            allowed[0x88:0x94] = False  # Independently verified XYZ.
        for negative in ('blocked',):
            if negative not in observations:
                continue
            other_matrix = np.concatenate([c['matrices'][name] for c in discovery
                                            if c['phase']['observation'] == negative])
            for bit in range(8):
                cbits, nbits = (clear_matrix >> bit) & 1, (other_matrix >> bit) & 1
                low, high = cbits.min(axis=0), cbits.max(axis=0)
                nlow, nhigh = nbits.min(axis=0), nbits.max(axis=0)
                offsets = np.flatnonzero(allowed & (low == high) & (nlow == nhigh) & (low != nlow))
                for offset in offsets:
                    offset = int(offset)
                    clear_value = int(low[offset])
                    vals = {c['name']: ((c['matrices'][name][:, offset] >> bit) & 1) for c in captures}
                    rows = evaluate(captures, vals, lambda v: v == clear_value, negative)
                    report['bits'].append({'region': name, 'offset': hex(offset), 'bit': bit,
                        'clear_value': clear_value, 'negative_observation': negative,
                        'evaluation': rows, **summarize_evaluation(rows, negative)})
        arrays = {c["name"]: scalar_array(c, name) for c in captures}
        for column in range(0x40//4, region["size"]//4):
            if name == "pawn" and 0x88 <= column*4 < 0x94:
                continue
            values = {key: value[:, column] for key, value in arrays.items()}
            clear = np.concatenate([values[c["name"]] for c in discovery if c["phase"]["observation"] == "clear"])
            for negative in ("blocked",):
                if negative not in observations:
                    continue
                other = np.concatenate([values[c["name"]] for c in discovery if c["phase"]["observation"] == negative])
                rule = threshold(clear, other)
                if rule:
                    report["scalars"].append({"region": name, "offset": hex(column*4),
                        "interpretation": "aligned_float32", **classification_report(captures, values, negative, rule)})
    # Session-wide PC time is used only to discover candidate engine clocks.
    # The render-age hypothesis then subtracts two captured fields, never PC
    # uptime directly from an arbitrary engine timestamp.
    world_arrays = {c["name"]: scalar_array(c, "world") for c in captures}
    t = np.concatenate([c["times"]["world"] for c in discovery])
    if np.ptp(t) >= 4:
        for column in range(0x40//4, next(r["size"] for r in scope["regions"] if r["name"] == "world")//4):
            vals = {key: value[:, column] for key, value in world_arrays.items()}
            v = np.concatenate([vals[c["name"]] for c in discovery])
            if not np.all(np.isfinite(v)) or np.any(v < 0):
                continue
            slope = float(np.dot(t-t.mean(), v-v.mean()) / np.dot(t-t.mean(), t-t.mean()))
            intercept = float(v.mean()-slope*t.mean())
            residual = float(np.max(np.abs(v-(slope*t+intercept))))
            if not 0.9 <= slope <= 1.1 or residual > 0.25 or np.any(np.diff(v) < -0.03):
                continue
            checks = []
            for c in captures:
                actual = vals[c['name']]
                finite = bool(np.all(np.isfinite(actual)))
                residual_value = float(np.max(np.abs(actual-(slope*c['times']['world']+intercept)))) if finite else None
                checks.append({'capture': c['name'], 'partition': c['phase']['partition'],
                    'max_residual_s': residual_value, 'passes': finite and residual_value <= 0.25})
            report['clocks'].append({'region': 'world', 'offset': hex(column*4),
                'slope': slope, 'intercept': intercept, 'discovery_max_residual_s': residual,
                'evaluation': checks, 'all_clock_comparisons_pass': all(c['passes'] for c in checks), 'values': vals})
    for region in scope["regions"]:
        name = region["name"]
        if name == "world":
            continue
        arrays = {c["name"]: scalar_array(c, name) for c in captures}
        for clock in report["clocks"]:
            for column in range(0x40//4, region["size"]//4):
                if name == "pawn" and 0x88 <= column*4 < 0x94:
                    continue
                timestamps = {key: value[:, column] for key, value in arrays.items()}
                # Compensate measured sequential-read skew using discovery slope.
                ages = {c["name"]: clock["values"][c["name"]]
                        + clock["slope"]*(c["times"][name]-c["times"]["world"])
                        - timestamps[c["name"]] for c in captures}
                train_timestamp = np.concatenate([timestamps[c["name"]] for c in discovery])
                train_age = np.concatenate([ages[c["name"]] for c in discovery])
                if (not np.all(np.isfinite(train_age)) or not np.all(np.isfinite(train_timestamp))
                        or np.any(train_timestamp < 0) or np.any(train_age < -0.25)
                        or np.any(train_age > 3600) or np.any(np.diff(train_timestamp) < -0.05)):
                    continue
                clear = np.concatenate([ages[c["name"]] for c in discovery if c["phase"]["observation"] == "clear"])
                if np.max(clear) > 1.0:
                    continue  # Explicit exploratory recency bound, not product rule.
                for negative in ("blocked",):
                    if negative not in observations:
                        continue
                    other = np.concatenate([ages[c["name"]] for c in discovery if c["phase"]["observation"] == negative])
                    rule = threshold(clear, other)
                    if rule and rule["clear_when"] == "less":
                        fit = classification_report(captures, ages, negative, rule)
                        fit['all_comparisons_pass'] = fit['all_comparisons_pass'] and clock['all_clock_comparisons_pass']
                        report["timestamps"].append({"region": name, "offset": hex(column*4),
                            "clock_offset": clock["offset"], "clock_slope": clock["slope"],
                            "interpretation": "candidate_world_float_minus_candidate_render_float",
                            "clock_comparisons_pass": clock["all_clock_comparisons_pass"], **fit})
    for clock in report["clocks"]:
        del clock["values"]
    for category in ("bits", "scalars", "timestamps"):
        report[category].sort(key=lambda c: (c["all_comparisons_pass"],
            c["minimum_control_or_recheck_score"] if c["minimum_control_or_recheck_score"] is not None else -1), reverse=True)
        for index, candidate in enumerate(report[category], 1):
            candidate["id"] = f"{category}-{index}"
    report["counts"] = {category: len(report[category]) for category in ("bits", "scalars", "clocks", "timestamps")}
    report["comparison_pass_counts"] = {category: sum(c["all_comparisons_pass"] for c in report[category])
                                        for category in ("bits", "scalars", "timestamps")}
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("session", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = review(args.session)
    write_json(args.output, result)
    print(json.dumps({"counts": result["counts"], "comparison_pass_counts": result.get("comparison_pass_counts", {}),
                      "status": result["status"], "report": str(args.output)}, indent=2))


if __name__ == "__main__":
    main()
