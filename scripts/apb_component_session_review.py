"""Offline readable-overlay review: comments, stance coupling, motion and damage leads."""

import argparse
import collections
import hashlib
import json
import math
import struct
from datetime import datetime
from pathlib import Path

import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("session", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Choose a new output path")
    counts, errors, observations = collections.Counter(), [], collections.defaultdict(list)
    stance = collections.Counter()
    with (args.session / "events.jsonl").open(encoding="utf-8") as stream:
        for line in stream:
            event = json.loads(line)
            counts[event["state"]] += 1
            if event["state"] == "capture_unavailable":
                errors.append(event)
            if event["state"] != "sample" or not (event.get("selected") or {}).get("decoded_fields"):
                continue
            point = event["selected"]
            fields = point["decoded_fields"]
            key = (point["address"], point["instance_signature"])
            observations[key].append((datetime.fromisoformat(event["utc"]).timestamp(), point))
            bounds = fields.get("collision", {})
            stance[(fields["eye_height_lead"], fields["height_related_a"], fields["height_related_b"],
                    bounds.get("half_height"), bounds.get("radius"))] += 1
    motion = []
    for key, samples in observations.items():
        measured, candidates, intervals = [], [], []
        still_nonzero = 0
        for (t0, p0), (t1, p1) in zip(samples, samples[1:]):
            dt = t1-t0
            if not 0.08 <= dt <= 0.65 or p0["ownership"]["vehicle_attachment"] or p1["ownership"]["vehicle_attachment"]:
                continue
            derivative = [(b-a)/dt for a, b in zip(p0["position"], p1["position"])]
            raw = [(a+b)/2 for a, b in zip(p0["decoded_fields"]["motion_vector_lead"],
                                         p1["decoded_fields"]["motion_vector_lead"])]
            if math.dist(p0["position"], p1["position"]) <= 0.01 and math.sqrt(sum(v*v for v in raw)) > 1:
                still_nonzero += 1
            measured.append(derivative)
            candidates.append(raw)
            intervals.append(dt)
        a, b = np.array(measured), np.array(candidates)
        axes = []
        if len(measured) > 2:
            for axis in range(3):
                mask = np.maximum(abs(a[:, axis]), abs(b[:, axis])) >= 30
                x, y = a[mask, axis], b[mask, axis]
                correlation = float(np.corrcoef(x, y)[0, 1]) if len(x)>2 and np.std(x)>0 and np.std(y)>0 else None
                fit = float(np.dot(x, y)/np.dot(y, y)) if len(x) and np.dot(y, y)>0 else None
                axes.append({"axis": "XYZ"[axis], "active_intervals": int(mask.sum()),
                             "pearson_measured_vs_motion": correlation,
                             "measured_units_per_motion_unit_fit": fit,
                             "median_absolute_difference": float(np.median(abs(x-y))) if len(x) else None})
        motion.append({"address": key[0], "instance_signature": key[1], "samples": len(samples),
                       "eligible_intervals": len(intervals), "still_position_with_nonzero_motion": still_nonzero,
                       "axes": axes, "limit": "Event times bracket sequential reads, not physics ticks; "
                       "replication, pose, timestamp placement and teleports can confound derivatives."})
    annotations = []
    bodies = []
    contexts = []
    for directory in sorted(args.session.glob("annotation-*")):
        path = directory / "annotation.json"
        data = path.read_bytes()
        record = json.loads(data)
        point = record["point"]
        fields = point["decoded_fields"]
        annotations.append({"file": str(path), "sha256": hashlib.sha256(data).hexdigest(),
            "sample_utc": record["sample_utc"], "annotated_utc": record["annotated_utc"],
            "address": point["address"], "instance_signature": point["instance_signature"],
            "comment": record["cursor_comment"], "label": point["operator_observation"],
            "integer_a": fields["integer_a"], "integer_b": fields["integer_b"],
            "eye_height_lead": fields["eye_height_lead"],
            "collision_half_height": fields.get("collision", {}).get("half_height"),
            "motion_vector_lead": fields["motion_vector_lead"]})
        bodies.append(bytes.fromhex(record["evidence"]["body_hex"]))
        contexts.append({(r['offset'],r['pointer'],r.get('vtable_rva')): bytes.fromhex(r['body_hex'])
                         for r in record['evidence']['pointer_context_unclassified'] if 'body_hex' in r})
    damage_differences = []
    # Full diagnostic words are preserved; searches cannot identify the writer
    # instruction/opcode, allocation ownership or actual damage timing.
    for before, after in ((1, 2), (2, 3), (4, 5)):
        if after >= len(bodies):
            continue
        words = []
        matches = []
        for offset in range(0, min(len(bodies[before]), len(bodies[after]))-3, 4):
            x = struct.unpack_from("<I", bodies[before], offset)[0]
            y = struct.unpack_from("<I", bodies[after], offset)[0]
            if x == y:
                continue
            words.append({"offset": hex(offset), "before_u32": x, "after_u32": y})
            fx = struct.unpack_from("<f", bodies[before], offset)[0]
            fy = struct.unpack_from("<f", bodies[after], offset)[0]
            for encoding, delta in (("u32", y-x), ("f32", fy-fx)):
                if not math.isfinite(delta):
                    continue
                for nominal in (750, 575, 300, 75, 57.5, 30, 0.75, 0.575, 0.3):
                    if abs(abs(delta)-nominal) <= 0.001:
                        matches.append({"offset": hex(offset), "encoding": encoding,
                                        "delta": delta, "nominal_match": nominal,
                                        "status": "coincidence_possible_unvalidated"})
        damage_differences.append({"before_annotation": before, "after_annotation": after,
                                  "changed_words": words, "nominal_damage_delta_matches": matches})
    context_damage_matches = []
    for before, after in ((1,2),(2,3)):
        common = set(contexts[before]) & set(contexts[after])
        for key in sorted(common):
            xbody, ybody = contexts[before][key], contexts[after][key]
            if key[1].lower() == annotations[before]['address'].lower():
                continue
            matches = []
            changes = 0
            for offset in range(0, min(len(xbody), len(ybody))-3, 4):
                x, y = struct.unpack_from('<I',xbody,offset)[0],struct.unpack_from('<I',ybody,offset)[0]
                if x == y:
                    continue
                changes += 1
                fx,fy = struct.unpack_from('<f',xbody,offset)[0],struct.unpack_from('<f',ybody,offset)[0]
                for encoding,delta in (('u32',y-x),('f32',fy-fx)):
                    if not math.isfinite(delta):
                        continue
                    for nominal in (750,575,300,75,57.5,30,0.75,0.575,0.3):
                        if abs(abs(delta)-nominal)<=0.001:
                            matches.append({'offset':hex(offset),'encoding':encoding,'delta':delta,
                                            'nominal_match':nominal,'status':'unvalidated'})
            context_damage_matches.append({'before_annotation':before,'after_annotation':after,
                'pawn_link_offset':key[0],'pointer':key[1],'vtable_rva':key[2],
                'changed_words':changes,'nominal_damage_delta_matches':matches})
    result = {"format": "apb-component-session-offline-review-v1", "events": dict(counts),
              "capture_errors": errors, "annotations": annotations,
              "stance_combinations": [{"eye_height_lead": key[0], "related_a": key[1], "related_b": key[2],
                                       "collision_half_height": key[3], "radius": key[4], "samples": count}
                                      for key, count in stance.items()],
              "motion_comparison": motion, "raw_damage_annotation_differences": damage_differences,
              "related_context_damage_differences": context_damage_matches,
              "constant_integer_fields": {key[0]: {"A": sorted({p['decoded_fields']['integer_a'] for _,p in samples}),
                                                    "B": sorted({p['decoded_fields']['integer_b'] for _,p in samples})}
                                          for key,samples in observations.items()},
              "limits": "Operator damage/weapon/stun comments are labels, not instrumented hit events. "
                        "Data dumps cannot identify machine-code writer opcodes. Probe extents are unverified. "
                        "LOS labels persist until changed and are not automatically valid for later comments."}
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
        stream.write("\n")
    print("events", dict(counts))
    print("stance", result["stance_combinations"])
    print("motion", motion)
    print("damage delta matches", [(r['before_annotation'], r['after_annotation'],r['nominal_damage_delta_matches'])
                                   for r in damage_differences])
    print('related context delta matches', [r for r in context_damage_matches if r['nominal_damage_delta_matches']])
    print("Saved", args.output)


if __name__ == "__main__":
    main()
