"""Offline integrity and cross-segment bit association review; never verifies LOS."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

import numpy as np

from apb_manual_labels import instance_signature


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("session", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        raise RuntimeError("Use a new review output directory")
    args.output.mkdir()
    source = json.loads((args.session / "selection/capture.json").read_text())
    header_data = (args.session/'selection/survey-0001.headers.bin').read_bytes()
    if len(header_data)%72:
        raise RuntimeError("Truncated survey header file")
    registered_headers = {struct.unpack_from('<Q',header_data,i)[0]:header_data[i+8:i+72]
                          for i in range(0,len(header_data),72)}
    summaries, data = [], []
    for directory in sorted(args.session.glob("segment-*")):
        metadata_path = directory / "samples.jsonl"
        raw_path = directory / "raw.bin"
        unfinished = not metadata_path.exists()
        if unfinished:
            metadata_path = directory / "samples.jsonl.partial"
            raw_path = directory / "raw.bin.partial"
        raw = raw_path.read_bytes()
        rows, problems = [], []
        for line_number, line in enumerate(metadata_path.read_bytes().splitlines(), 1):
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                problems.append(f"Invalid JSON at line {line_number}; excluded")
                continue
            if any(r["raw_offset"] < 0 or r["size"] <= 0 or
                   r["raw_offset"]+r["size"] > len(raw) for r in row["regions"]):
                problems.append(f"Out-of-range bytes at line {line_number}; excluded")
                continue
            rows.append(row)
        if not rows:
            raise RuntimeError(f"No usable rows: {directory}")
        if any(r["round"] != i for i, r in enumerate(rows)):
            problems.append("Round sequence is not contiguous")
        layout = [(r["name"], r["size"]) for r in rows[0]["regions"]]
        if any([(r["name"], r["size"]) for r in row["regions"]] != layout for row in rows):
            raise RuntimeError("Region layout changed")
        expected_offset = 0
        for row in rows:
            for region in row["regions"]:
                if region["raw_offset"] != expected_offset:
                    raise RuntimeError("Raw byte ranges are not contiguous")
                expected_offset += region["size"]
        matrices = {}
        identities = {}
        for name, size in layout:
            buffers = [raw[r["raw_offset"]:r["raw_offset"]+r["size"]]
                       for row in rows for r in row["regions"] if r["name"] == name]
            matrices[name] = np.array([np.frombuffer(b, dtype=np.uint8) for b in buffers])
            fingerprints = Counter(instance_signature(b) for b in buffers)
            identities[name] = dict(fingerprints)
            if len(fingerprints) != 1:
                problems.append(f"{name}: changing instance fingerprint")
        first_pawn = matrices['pawn'][0].tobytes()
        matches = [p for p in source["points"] if p["instance_signature"] == instance_signature(first_pawn)]
        if len(matches) != 1:
            raise RuntimeError("Cannot uniquely match pawn fingerprint to selection")
        point = matches[0]
        pawn_address = int(point['address'],16)
        neighboring = [{'offset':hex(addr-pawn_address),'address':hex(addr),
                        'marker':struct.unpack_from('<I',header,0x3c)[0]}
                       for addr,header in registered_headers.items()
                       if pawn_address < addr < pawn_address+len(first_pawn)
                       and first_pawn[addr-pawn_address:addr-pawn_address+64] == header]
        pawn_cutoff = min([int(n['offset'],16) for n in neighboring],default=len(first_pawn))
        times = np.array([row["elapsed_s"] for row in rows])
        gaps = np.diff(times)
        if np.any(gaps <= 0):
            problems.append("Non-increasing sample timestamps")
        labels = np.array([row["observation"] for row in rows])
        runs = []
        start = 0
        for i in range(1, len(rows)+1):
            if i == len(rows) or labels[i] != labels[start]:
                runs.append({"label": str(labels[start]), "first_round": start, "last_round": i-1,
                             "start_s": float(times[start]), "end_s": float(times[i-1]), "rows": i-start})
                start = i
        stable = np.zeros(len(rows), dtype=bool)
        for run in runs:
            stable |= ((times >= run["start_s"]+0.5) & (times <= run["end_s"]-0.5))
        segment_meta = json.loads((directory / "segment.json").read_text()) if (directory / "segment.json").exists() else {}
        xyz = matrices['pawn'][:, 0x88:0x94].copy().view('<f4').reshape(-1, 3)
        camera = np.array([r["camera"]["position"] for r in rows])
        summary = {"segment": directory.name, "unfinished": unfinished,
                   "condition": segment_meta.get("condition", "unavailable: unfinished segment"),
                   "rows": len(rows), "duration_s": float(times[-1]-times[0]),
                   "achieved_hz": float((len(rows)-1)/(times[-1]-times[0])),
                   "gap_p50_s": float(np.median(gaps)), "max_gap_s": float(gaps.max()),
                   "labels": dict(Counter(labels.tolist())), "runs": runs,
                   "stable_labels": dict(Counter(labels[stable].tolist())),
                   "raw_bytes": len(raw), "referenced_bytes": expected_offset,
                   "unreferenced_tail_bytes": len(raw)-expected_offset,
                   "pawn": {"address": point["address"], "id": point["id"]},
                   "registered_neighbor_headers_inside_pawn_probe":neighboring,
                   "pawn_association_scan_cutoff":hex(pawn_cutoff),
                   "fingerprints": identities, "problems": problems,
                   "pawn_axis_ranges": np.ptp(xyz, axis=0).tolist(),
                   "camera_axis_ranges": np.ptp(camera, axis=0).tolist(),
                   "dynamic_byte_counts": {name:int(np.count_nonzero(np.any(m != m[0],axis=0))) for name,m in matrices.items()},
                   "dynamic_aligned_float_counts": {name:int(np.count_nonzero(np.any(m.copy().view('<u4').reshape(len(rows),-1) != m[0].copy().view('<u4'),axis=0))) for name,m in matrices.items()},
                   "inputs": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (metadata_path, raw_path)}}
        summaries.append(summary)
        data.append({"segment": directory.name, "matrices": matrices, "labels": labels,
                     "stable": stable, "times": times,'pawn_cutoff':pawn_cutoff})
    # Fit on the first completed segment with both clear and blocked plateaus.
    train = next(d for d,s in zip(data,summaries) if not s['unfinished']
                 and set(d['labels'][d['stable']]) >= {'clear', 'blocked'})
    candidates = []
    for name, matrix in train['matrices'].items():
        mask = train['stable'] & np.isin(train['labels'], ['clear', 'blocked'])
        truth = train['labels'][mask] == 'clear'
        for bit in range(8):
            values = ((matrix[mask] >> bit) & 1).astype(bool)
            score = (values[truth].mean(axis=0) + (~values[~truth]).mean(axis=0))/2
            orientation = score >= .5
            score = np.maximum(score, 1-score)
            for offset in np.flatnonzero(score >= .65):
                if offset < 0x40:
                    continue
                if name == 'pawn' and offset >= min(d['pawn_cutoff'] for d in data):
                    continue
                item = {"region": name, "offset": hex(int(offset)), "bit": bit,
                        "set_means_clear": bool(orientation[offset]), "training_balanced_accuracy": float(score[offset]),
                        "evaluation": {}}
                for d in data:
                    v = ((d['matrices'][name][:,offset] >> bit) & 1).astype(bool)
                    pred = v if orientation[offset] else ~v
                    m = d['stable']
                    clear, blocked, partial = [m & (d['labels'] == label) for label in ['clear','blocked','partial']]
                    cr = float(pred[clear].mean()) if clear.any() else None
                    br = float((~pred[blocked]).mean()) if blocked.any() else None
                    item['evaluation'][d['segment']] = {
                        'clear_correct': cr, 'blocked_correct': br,
                        'balanced_accuracy': (cr+br)/2 if cr is not None and br is not None else None,
                        'partial_predicted_clear_share': float(pred[partial].mean()) if partial.any() else None}
                heldout = [v['balanced_accuracy'] for k,v in item['evaluation'].items() if k != train['segment']
                           and v['balanced_accuracy'] is not None]
                item['minimum_heldout_balanced_accuracy'] = min(heldout) if heldout else None
                candidates.append(item)
    candidates.sort(key=lambda c: (c['minimum_heldout_balanced_accuracy'] or 0,
                                   c['training_balanced_accuracy']), reverse=True)
    scalar_candidates = []
    for name, matrix in train['matrices'].items():
        arrays = {}
        for d in data:
            words = d['matrices'][name].copy().view('<u4').reshape(len(d['labels']), -1)
            invalid = (words & 0x7f800000) == 0x7f800000
            # Arbitrary probe bytes include non-floats. Canonicalize invalid float
            # interpretations to NaN and exclude such columns, retaining raw inputs.
            words[invalid] = 0x7fc00000
            arrays[d['segment']] = words.view('<f4').astype(float)
        mask = train['stable'] & np.isin(train['labels'], ['clear','blocked'])
        truth = train['labels'][mask] == 'clear'
        for column in range(16, arrays[train['segment']].shape[1]):
            if name == 'pawn' and column*4 >= min(d['pawn_cutoff'] for d in data):
                continue
            values = arrays[train['segment']][:,column]
            if not np.isfinite(values).all() or np.max(np.abs(values)) > 1e7 or np.ptp(values) < 1e-6:
                continue
            for interpretation in ('raw_float','elapsed_minus_float'):
                v = values if interpretation == 'raw_float' else train['times']-values
                cutoffs = np.unique(np.quantile(v[mask], np.linspace(0,1,33)))
                pred = v[mask,None] > cutoffs[None,:]
                score = (pred[truth].mean(axis=0)+(~pred[~truth]).mean(axis=0))/2
                orientation = score >= .5
                score = np.maximum(score,1-score)
                best = int(score.argmax())
                if score[best] < .7:
                    continue
                if any(not np.isfinite(a[:,column]).all() for a in arrays.values()):
                    continue
                item = {'region':name,'offset':hex(column*4),'interpretation':interpretation,
                        'threshold':float(cutoffs[best]),'greater_means_clear':bool(orientation[best]),
                        'training_balanced_accuracy':float(score[best]),'evaluation':{}}
                heldout = []
                for d in data:
                    v = arrays[d['segment']][:,column]
                    if interpretation != 'raw_float':
                        # Segment clocks reset. Calibrate each segment's candidate timestamp
                        # origin on clear plateaus; this is descriptive, not held-out prediction.
                        v = d['times']-v
                        train_clear = train['stable'] & (train['labels']=='clear')
                        dc = d['stable'] & (d['labels']=='clear')
                        if dc.any():
                            v = v - np.median(v[dc]) + np.median((train['times']-values)[train_clear])
                    pr = v > cutoffs[best]
                    if not orientation[best]:
                        pr = ~pr
                    cl = d['stable'] & (d['labels']=='clear')
                    bl = d['stable'] & (d['labels']=='blocked')
                    cr = float(pr[cl].mean()) if cl.any() else None
                    br = float((~pr[bl]).mean()) if bl.any() else None
                    ba = (cr+br)/2 if cr is not None and br is not None else None
                    item['evaluation'][d['segment']] = {'clear_correct':cr,'blocked_correct':br,'balanced_accuracy':ba}
                    if d['segment'] != train['segment'] and ba is not None:
                        heldout.append(ba)
                item['minimum_other_segment_balanced_accuracy'] = min(heldout) if heldout else None
                scalar_candidates.append(item)
    scalar_candidates.sort(key=lambda c:(c['minimum_other_segment_balanced_accuracy'] or 0,c['training_balanced_accuracy']),reverse=True)
    result = {'format':'apb-visibility-trace-review-v1', 'source_session':str(args.session),
              'session_manifest_present':(args.session/'session.json').exists(),
              'identity':source['identity'], 'segments':summaries, 'training_segment':train['segment'],
              'transition_exclusion_s_each_side':0.5, 'candidate_bit_count':len(candidates),
              'top_candidate_bits':candidates[:60],
              'scalar_candidate_count':len(scalar_candidates),'top_scalar_candidates':scalar_candidates[:60],
              'timestamp_method_limit':'elapsed_minus_float is recalibrated using each segment clear plateaus; its other-segment scores are descriptive, not independent predictions.',
              'limits':'Exploratory bit associations across many offsets, not LOS verification. Labels are operator observations. No independent scene recording. Probe extents may include neighboring objects.'}
    (args.output/'review.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'segments':[{k:s[k] for k in ('segment','condition','rows','achieved_hz','labels','stable_labels','pawn_axis_ranges','camera_axis_ranges','problems','unreferenced_tail_bytes')} for s in summaries],
                      'training':train['segment'],'candidate_count':len(candidates),'top':candidates[:3],
                      'scalar_candidate_count':len(scalar_candidates),'top_scalars':scalar_candidates[:5]},indent=2))


if __name__ == '__main__':
    main()
