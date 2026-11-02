"""Offline pointer and field review of EchoDriver component annotation captures.

This reads saved files only. A matching pointer pattern is a reference lead,
not proof of ownership, a current resource value, or a writer instruction.
"""

import argparse
import collections
import hashlib
import json
import math
import struct
from datetime import datetime
from pathlib import Path

import numpy as np


def occurrences(raw, target):
    needle = struct.pack("<Q", target)
    start = 0
    while (offset := raw.find(needle, start)) >= 0:
        yield offset
        start = offset + 1


def number(raw, offset):
    integer = struct.unpack_from("<I", raw, offset)[0]
    scalar = struct.unpack_from("<f", raw, offset)[0]
    return {"u32": integer, "f32": scalar if math.isfinite(scalar) else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("session", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Choose a new output file; original evidence is preserved")
    records, sources = [], []

    def read(path):
        raw = path.read_bytes()
        sources.append({"file": str(path), "bytes": len(raw),
                        "sha256": hashlib.sha256(raw).hexdigest()})
        return raw

    for directory in sorted(args.session.glob("annotation-*")):
        item = json.loads(read(directory / "annotation.json"))
        point = item["point"]
        contexts = [{"link_offset": None, "address": int(point["address"], 16),
                     "raw": bytes.fromhex(item["evidence"]["body_hex"])}]
        for link in item["evidence"]["pointer_context_unclassified"]:
            if "body_hex" in link:
                contexts.append({"link_offset": int(link["offset"], 16),
                                 "address": int(link["pointer"], 16),
                                 "raw": bytes.fromhex(link["body_hex"])})
        records.append({"annotation": directory.name, "sample_utc": item["sample_utc"],
                        "comment": item["cursor_comment"], "point": point, "contexts": contexts})
    if not records:
        parser.error("No saved annotations found")
    identities = {(r["point"]["address"], r["point"]["instance_signature"]) for r in records}
    if len(identities) != 1:
        parser.error("This focused review requires one exact pawn instance")
    pawn = records[0]["contexts"][0]["address"]
    module_base = struct.unpack_from("<Q", records[0]["contexts"][0]["raw"])[0] - 0x2AAEFA0
    targets = {"pawn": pawn, "integer_a_field": pawn + 0x4B4, "integer_b_field": pawn + 0x514}
    related = sorted({c["address"] for r in records for c in r["contexts"]})
    nearby = {address: {} for address in related}
    references = {}
    formats = {"headers": (0x40, 8), "candidate-bodies": (0x800, 8),
               "linked-bodies": (0x800, 8), "table": (None, 0)}
    files_scanned = collections.Counter()
    for path in sorted(args.session.glob("survey-*.bin")):
        sequence = int(path.name.split(".")[0].split("-")[1])
        kind = path.name.split(".")[1]
        if kind not in formats:
            raise RuntimeError(f"Unknown survey format: {path}")
        raw = read(path)
        payload_size, prefix_size = formats[kind]
        stride = payload_size + prefix_size if payload_size is not None else None
        if stride and len(raw) % stride:
            raise RuntimeError(f"Incomplete survey record: {path}")
        files_scanned[kind] += 1
        if kind == "headers":
            addresses = np.ndarray((len(raw)//stride,), dtype="<u8", buffer=raw, strides=(stride,))
            for address in related:
                for index in np.flatnonzero((addresses > address) & (addresses < address + 0x3000)):
                    candidate = int(addresses[index])
                    header = raw[int(index)*stride+8:int(index)*stride+8+0x40]
                    vt, marker = struct.unpack_from("<Q", header)[0], struct.unpack_from("<I", header, 0x3C)[0]
                    if module_base <= vt < module_base + 0x4000000:
                        key = (candidate, vt, marker)
                        nearby[address].setdefault(key, []).append(sequence)
        for name, target in targets.items():
            for location in occurrences(raw, target):
                if stride:
                    record_offset, offset = divmod(location, stride)
                    # Skip address metadata and patterns crossing record boundaries.
                    if offset < prefix_size or offset + 8 > stride:
                        continue
                    offset -= prefix_size
                    address = struct.unpack_from("<Q", raw, record_offset*stride)[0]
                    body_start = record_offset*stride+prefix_size
                    vt = struct.unpack_from("<Q", raw, body_start)[0]
                    marker = struct.unpack_from("<I", raw, body_start+0x3C)[0]
                else:
                    address, offset = module_base + 0x3C00000, location
                    vt, marker = None, None
                key = (name, kind, address, offset, vt, marker)
                references.setdefault(key, []).append(sequence)
    context_reviews = []
    for record in records:
        contexts = []
        for context in record["contexts"]:
            address, raw = context["address"], context["raw"]
            adjacent = []
            for (candidate, vt, marker), sequences in sorted(nearby[address].items()):
                offset = candidate-address
                if offset + 0x40 <= len(raw) and (struct.unpack_from("<Q", raw, offset)[0],
                        struct.unpack_from("<I", raw, offset+0x3C)[0]) == (vt, marker):
                    adjacent.append({"offset": hex(offset), "address": hex(candidate),
                                     "vtable_rva": hex(vt-module_base), "marker": marker,
                                     "survey_sequences": sorted(set(sequences))})
            # A registered object header matching probe bytes marks a possible
            # neighboring object. It is NOT a measured allocation boundary.
            prefix_end = min([int(x["offset"], 16) for x in adjacent] or [len(raw)])
            contexts.append({"link_offset": hex(context["link_offset"]) if context["link_offset"] is not None else None,
                "address": hex(address), "probe_bytes": len(raw), "comparison_prefix_end": hex(prefix_end),
                "registered_embedded_headers": adjacent,
                "pawn_pattern_offsets": [hex(o) for o in occurrences(raw, pawn)],
                "field_address_pattern_offsets": {n: [hex(o) for o in occurrences(raw, t)]
                                                   for n, t in targets.items() if n != "pawn"}})
        context_reviews.append({"annotation": record["annotation"], "sample_utc": record["sample_utc"],
                                "comment": record["comment"], "contexts": contexts})
    comparisons = []
    for before, after in ((1, 2), (2, 3), (4, 5)):
        if after >= len(records):
            continue
        left = {(c["link_offset"], c["address"]): (c, d) for c, d in
                zip(records[before]["contexts"], context_reviews[before]["contexts"])}
        right = {(c["link_offset"], c["address"]): (c, d) for c, d in
                 zip(records[after]["contexts"], context_reviews[after]["contexts"])}
        changed = []
        for key in sorted(left.keys() & right.keys(), key=lambda k: (k[0] or -1, k[1])):
            a, ad = left[key]
            b, bd = right[key]
            end = min(int(ad["comparison_prefix_end"], 16), int(bd["comparison_prefix_end"], 16))
            words = []
            for offset in range(0, end-3, 4):
                if a["raw"][offset:offset+4] != b["raw"][offset:offset+4]:
                    av, bv = number(a["raw"], offset), number(b["raw"], offset)
                    words.append({"offset": hex(offset), "address": hex(a["address"]+offset),
                                  "before": av, "after": bv})
            changed.append({"link_offset": hex(key[0]) if key[0] is not None else None,
                            "address": hex(key[1]), "prefix_end": hex(end), "changed_words": words})
        comparisons.append({"before": records[before]["annotation"], "after": records[after]["annotation"],
                            "contexts": changed})
    linked_timelines = []
    offsets = sorted({c["link_offset"] for r in records for c in r["contexts"] if c["link_offset"] is not None})
    for offset in offsets:
        snapshots = []
        for record in records:
            body = record["contexts"][0]["raw"]
            target = struct.unpack_from("<Q", body, offset)[0]
            context = next((c for c in record["contexts"] if c["link_offset"] == offset), None)
            snapshots.append({"annotation": record["annotation"], "pointer": hex(target),
                "typed_context_saved": context is not None,
                "vtable_rva": hex(struct.unpack_from("<Q", context["raw"])[0]-module_base) if context else None,
                "marker": struct.unpack_from("<I", context["raw"], 0x3C)[0] if context else None})
        linked_timelines.append({"pawn_offset": hex(offset), "pointer_field_address": hex(pawn+offset),
                                 "changed_target": len({s["pointer"] for s in snapshots}) > 1,
                                 "snapshots": snapshots})
    event_path = args.session / "events.jsonl"
    event_hash = hashlib.sha256()
    pointer_transitions = collections.defaultdict(list)
    previous, positions, episodes, active = {}, [], [], None
    sample_count = 0
    with event_path.open("rb") as stream:
        for line in stream:
            event_hash.update(line)
            event = json.loads(line)
            point = event.get("selected") or {}
            if (point.get("address"), point.get("instance_signature")) not in identities:
                continue
            fields = point.get("decoded_fields")
            if not fields:
                continue
            sample_count += 1
            body = bytes.fromhex(fields["raw_body_hex"])
            xyz = struct.unpack_from("<fff", body, 0x5F0)
            positions.append([abs(a-b) for a, b in zip(xyz, point["position"])])
            for offset in (0x178, 0x980):
                value = struct.unpack_from("<Q", body, offset)[0]
                if previous.get(offset) != value:
                    pointer_transitions[hex(offset)].append({"utc": event["utc"], "pointer": hex(value)})
                previous[offset] = value
            value = body[0x707]
            now = datetime.fromisoformat(event["utc"]).timestamp()
            if value and active is None:
                active = {"started_utc": event["utc"], "start_time": now, "initial_byte": value,
                          "peak_byte": value, "max_sample_gap_seconds": 0, "samples": 0, "last_time": now}
            if active is not None:
                active["samples"] += 1
                active["peak_byte"] = max(active["peak_byte"], value)
                active["max_sample_gap_seconds"] = max(active["max_sample_gap_seconds"], now-active["last_time"])
                active["last_time"] = now
                if not value:
                    active["ended_utc"] = event["utc"]
                    active["elapsed_seconds"] = now-active.pop("start_time")
                    active.pop("last_time")
                    episodes.append(active)
                    active = None
    sources.append({"file": str(event_path), "bytes": event_path.stat().st_size, "sha256": event_hash.hexdigest()})
    changing_state_leads = []
    for offset in (0x254, 0x914):
        snapshots = []
        for record in records:
            context = next(c for c in record["contexts"] if c["link_offset"] == 0x988)
            snapshots.append(number(context["raw"], offset)["f32"])
        changing_state_leads.append({"chain": f"pawn + 0x988 -> object + {hex(offset)}",
                                    "captured_address": hex(context["address"]+offset),
                                    "annotation_f32_values": snapshots,
                                    "status": "binary_state_or_weight_lead_not_resource_quantity"})
    report = {"format": "offline_damage_pointer_review_v1", "source_session": str(args.session),
        "pawn": hex(pawn), "instance_signature": records[0]["point"]["instance_signature"],
        "integer_fields": [{"offset": hex(o), "address": hex(pawn+o),
                             "annotation_values": [number(r["contexts"][0]["raw"], o)["u32"] for r in records]}
                            for o in (0x4B4, 0x514)],
        "survey_files_scanned": dict(files_scanned),
        "saved_pointer_pattern_references": [{"target": k[0], "source_kind": k[1], "source_base": hex(k[2]),
            "offset": hex(k[3]), "address": hex(k[2]+k[3]),
            "vtable_rva": hex(k[4]-module_base) if k[4] is not None else None, "marker": k[5],
            "survey_sequences": sorted(set(v))} for k, v in sorted(references.items())],
        "linked_timelines": linked_timelines, "annotation_contexts": context_reviews,
        "selected_pawn_event_samples": sample_count, "event_pointer_transitions": dict(pointer_transitions),
        "changing_state_leads": changing_state_leads,
        "countdown_byte_lead": {"offset": "0x707", "address": hex(pawn+0x707),
            "encoding": "u8", "annotation_values": [r["contexts"][0]["raw"][0x707] for r in records],
            "completed_episodes": episodes, "incomplete_episode": active,
            "status": "timer_or_state_lead_stun_annotation_coincides_not_confirmed_stamina"},
        "rejected_quantity_lead": {"offset": "0x5f8", "address": hex(pawn+0x5F8),
            "annotation_values": [number(r["contexts"][0]["raw"], 0x5F8)["f32"] for r in records],
            "xyz_candidate_offset": "0x5f0", "median_absolute_xyz_vs_position": np.median(positions, axis=0).tolist() if positions else None,
            "samples_xyz_within_one_unit": sum(max(d) <= 1 for d in positions),
            "status": "position_like_vector_z_not_good_health_or_stamina_lead"},
        "damage_window_changed_words": comparisons, "source_manifest": sources,
        "limitations": ["All reads are offline saved EchoDriver captures; no live process reads.",
            "Damage comments are delayed snapshots, not event-aligned pre-hit and post-hit measurements.",
            "References are exact 8-byte patterns, including unaligned matches; semantic use is unverified.",
            "Registered embedded headers help reject neighboring memory but do not establish allocation sizes.",
            "Neither object ownership, health/stamina meaning nor the instruction writing values is inferred."]}
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")
    print(json.dumps({"output": str(args.output), "survey_files_scanned": dict(files_scanned),
                      "reference_locations": len(references), "annotations": len(records)}))


if __name__ == "__main__":
    main()
