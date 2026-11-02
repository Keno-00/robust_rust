"""Review saved APB object relationships; never attaches to or reads a process.

Inputs are preserved captures. Output records measured links, capture limits and
SHA-256 provenance. Numeric offsets are hypotheses tied to the captured build.
"""

import argparse
import hashlib
import json
import math
import struct
from collections import Counter
from pathlib import Path


IMAGE_SHA256 = "3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640"
PAWN = (0x2AAEFA0, 5778)
VEHICLE = (0x2B29FB8, 6064)
WORLD = (0x3161E78, 430)
HEAD = 0x48C
NEXT = 0x350
DEFAULT_ROOT = Path(r"C:\Users\Admin\Documents\source_repos\drayebr\build-rpm\snapshots")


def number(value):
    return int(value, 0) if isinstance(value, str) else value


def qword(body, offset):
    return struct.unpack_from("<Q", body, offset)[0]


def object_type(body, base):
    return qword(body, 0) - base, struct.unpack_from("<I", body, 0x3C)[0]


def type_record(body, base):
    rva, marker = object_type(body, base)
    return {"vtable_rva": hex(rva), "marker": marker}


def signature(body):
    return body[:8].hex() + ":" + body[0x24:0x40].hex()


def xyz(body):
    values = struct.unpack_from("<fff", body, 0x88)
    if not all(math.isfinite(v) and abs(v) < 1_000_000 for v in values):
        raise ValueError("Invalid saved position at +0x88")
    return values


def ranks(values):
    order = sorted(range(len(values)), key=values.__getitem__)
    result = [0.0] * len(values)
    start = 0
    while start < len(order):
        end = start + 1
        while end < len(order) and values[order[start]] == values[order[end]]:
            end += 1
        for i in order[start:end]:
            result[i] = (start + end - 1) / 2
        start = end
    return result


def spearman(x, y):
    if len(x) < 3:
        return None
    a, b = ranks(x), ranks(y)
    am, bm = sum(a) / len(a), sum(b) / len(b)
    numerator = sum((u - am) * (v - bm) for u, v in zip(a, b))
    denominator = math.sqrt(sum((u - am) ** 2 for u in a)
                            * sum((v - bm) ** 2 for v in b))
    return numerator / denominator if denominator else None


class Evidence:
    def __init__(self, root):
        self.root = root
        self.inputs = {}

    def read(self, path):
        raw = path.read_bytes()
        self.inputs[str(path.relative_to(self.root))] = {
            "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        return raw

    def json(self, path):
        return json.loads(self.read(path))

    def check_identity(self, report):
        if report["identity"]["image_sha256"].upper() != IMAGE_SHA256:
            raise ValueError("Capture executable hash differs from reviewed build")

    def objects(self, directory, records, base):
        bodies = {}
        for record in records:
            address = number(record["address"])
            path = directory / f"object-{address:016X}.bin"
            body = self.read(path)
            expected = number(record["vtable_rva"]), record["marker"]
            if object_type(body, base) != expected:
                raise ValueError(f"{path}: raw type differs from metadata")
            if signature(body) != record["instance_signature"]:
                raise ValueError(f"{path}: raw fingerprint differs from metadata")
            bodies[address] = body
        return bodies


def identity_review(evidence, path):
    report = evidence.json(path)
    evidence.check_identity(report)
    base = number(report["module_base"])
    bodies = evidence.objects(path.parent, report["objects"].values(), base)
    camera = number(report["camera"]["camera"])
    world = qword(bodies[camera], 0x2E0)
    if object_type(bodies[world], base) != WORLD:
        raise ValueError(f"{path}: camera link is not the captured World type")
    controller = qword(bodies[world], 0x358)
    local = qword(bodies[controller], 0x790)
    head = qword(bodies[world], HEAD)
    captured_pawns = {a for a, b in bodies.items() if object_type(b, base) == PAWN}
    cursor, seen, chain, stop = head, set(), [], "null"
    while cursor:
        if cursor in seen:
            stop = "cycle"
            break
        if cursor not in bodies:
            stop = "uncaptured_target"
            break
        body = bodies[cursor]
        if object_type(body, base) not in (PAWN, VEHICLE):
            stop = "unclassified_node_type"
            break
        seen.add(cursor)
        next_address = qword(body, NEXT)
        node = {"address": hex(cursor), **type_record(body, base),
                "next": hex(next_address), "world": hex(qword(body, 0x2E0)),
                "world_matches": qword(body, 0x2E0) == world,
                "xyz": xyz(body), "captured_fingerprint": signature(body)}
        if next_address in bodies:
            node["next_address_delta_bytes"] = next_address - cursor
            if object_type(bodies[next_address], base) in (PAWN, VEHICLE):
                node["next_xyz_distance_units"] = math.dist(xyz(body), xyz(bodies[next_address]))
        chain.append(node)
        cursor = next_address
    pairs = []
    addresses = sorted(captured_pawns)
    for i, a in enumerate(addresses):
        for b in addresses[i + 1:]:
            pairs.append({"a": hex(a), "b": hex(b), "address_distance_bytes": b - a,
                          "xyz_distance_units": math.dist(xyz(bodies[a]), xyz(bodies[b]))})
    # Exploratory only: repeated captures and pair endpoints are dependent.
    correlation = spearman([p["address_distance_bytes"] for p in pairs],
                           [p["xyz_distance_units"] for p in pairs])
    offsets = [hex(o) for o in range(0x340, 0x900, 4)
               if qword(bodies[world], o) in captured_pawns]
    return {"dataset": path.parent.name, "identity": report["identity"],
            "started_utc": report["started_utc"], "completed_utc": report["completed_utc"],
            "module_base": hex(base), "camera": hex(camera), "world": hex(world),
            "controller": hex(controller), "local_character": hex(local),
            "local_reciprocal_controller_matches": qword(bodies[local], 0x3E0) == controller,
            "head": hex(head), "chain": chain, "stop_reason": stop,
            "captured_character_count": len(captured_pawns),
            "chain_character_count": sum(object_type(bodies[a], base) == PAWN for a in seen),
            "captured_characters_outside_chain": [hex(a) for a in sorted(captured_pawns - seen)],
            "world_prefix_offsets_pointing_to_captured_characters": offsets,
            "metadata_errors": report.get("errors", []),
            "unchanged_fingerprints": all(r["fingerprint_unchanged_after_capture"]
                                           for r in report["objects"].values()),
            "pair_distances": pairs, "exploratory_spearman_address_vs_xyz_distance": correlation,
            "coverage_limit": "captured objects only; sequential reads, one process; no district census"}


def annotation_review(evidence, path):
    report = evidence.json(path)
    evidence.check_identity(report)
    remote = bytes.fromhex(report["body_hex"])
    base = qword(remote, 0) - number(report["lead"]["vtable_rva"])
    if object_type(remote, base)[1] != report["lead"]["marker"]:
        raise ValueError(f"{path}: raw annotation marker differs from metadata")
    contexts = report.get("pointer_context_unclassified", [])
    worlds = [(number(x["pointer"]), bytes.fromhex(x["body_hex"])) for x in contexts
              if x.get("body_hex") and object_type(bytes.fromhex(x["body_hex"]), base) == WORLD]
    record = {"dataset": path.parent.name, "annotation": path.name, "utc": report["utc"],
              "identity": report["identity"], "module_base": hex(base),
              "survey_sequence": report["survey_sequence"],
              "label": report["label"], "world_contexts": []}
    # Preserve each read separately: do not merge bodies across annotations.
    for address, body in worlds:
        item = {"world": hex(address), "captured_world_body_bytes": len(body)}
        if len(body) >= HEAD + 8:
            head = qword(body, HEAD)
            item["head"] = hex(head)
            headers_path = path.parent / f"survey-{report['survey_sequence']:04d}.headers.bin"
            headers = evidence.read(headers_path)
            if len(headers) % 72:
                raise ValueError(f"{headers_path}: partial header record")
            matches = [headers[i + 8:i + 72] for i in range(0, len(headers), 72)
                       if qword(headers, i) == head]
            item["head_in_preceding_survey"] = bool(matches)
            item["head_header_types"] = [type_record(h, base) for h in matches]
        else:
            item["limit"] = "captured World prefix ends before +0x48C"
        record["world_contexts"].append(item)
    if len(remote) >= NEXT + 8 and object_type(remote, base) == PAWN:
        nxt = qword(remote, NEXT)
        record["character"] = {"address": report["lead"]["address"],
                               "world": hex(qword(remote, 0x2E0)), "next": hex(nxt),
                               "next_context_types": [type_record(bytes.fromhex(x["body_hex"]), base)
                                   for x in contexts if number(x["pointer"]) == nxt and x.get("body_hex")]}
    record["limit"] = "annotation and preceding survey are sequential, not an atomic snapshot"
    return record


def vehicle_review(evidence, path):
    report = evidence.json(path)
    evidence.check_identity(report)
    controller = number(report["controller"])
    controller_record = next(r for r in report["objects"] if number(r["address"]) == controller)
    controller_body = evidence.read(path.parent / f"object-{controller:016X}.bin")
    base = qword(controller_body, 0) - number(controller_record["vtable_rva"])
    bodies = evidence.objects(path.parent, report["objects"], base)
    possessed = qword(bodies[controller], 0x790)
    character = qword(bodies[controller], 0x1380)
    return {"dataset": path.parent.parent.name, "stage": report["stage"], "utc": report["utc"],
            "identity": report["identity"], "module_base": hex(base), "world": report["world"],
            "world_body_captured": number(report["world"]) in bodies,
            "controller": hex(controller), "possessed": hex(possessed),
            "possessed_type": type_record(bodies[possessed], base) if possessed in bodies else None,
            "possessed_reciprocal_controller_matches": (qword(bodies[possessed], 0x3E0) == controller
                                                        if possessed in bodies else None),
            "character_1380": hex(character), "character_1388": hex(qword(bodies[controller], 0x1388)),
            "character_controller": hex(qword(bodies[character], 0x3E0)),
            "character_attachment": hex(qword(bodies[character], 0x178)),
            "next_links": [{"address": hex(a), **type_record(b, base), "next": hex(qword(b, NEXT)),
                            "world_matches": qword(b, 0x2E0) == number(report["world"])}
                           for a, b in bodies.items() if object_type(b, base) in (PAWN, VEHICLE)],
            "metadata_errors": report["errors"], "failed_survey_headers": report["failed_survey_headers"],
            "limit": "no captured World head; nodes may be stale or changed during sequential capture"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.root.is_dir():
        parser.error(f"Capture root is unavailable: {args.root}")
    if args.output.exists():
        parser.error(f"Use a new output path; existing evidence is preserved: {args.output}")
    inventory = Counter()
    total_bytes = 0
    for path in args.root.rglob("*"):
        if path.is_file():
            inventory[path.suffix.lower()] += 1
            total_bytes += path.stat().st_size
    evidence = Evidence(args.root)
    identities = [identity_review(evidence, args.root / f"identity-inspect-20260930-{i}"
                                 / "identity_inspection.json") for i in range(1, 7)]
    annotations = []
    for name, count in (("mission-labels-20260930-2", 14), ("fresh-launch-ownership-20260930-1", 2)):
        paths = sorted((args.root / name).glob("annotation-*.json"))
        if len(paths) != count:
            raise ValueError(f"{name}: expected {count} saved annotations, found {len(paths)}")
        annotations.extend(annotation_review(evidence, p) for p in paths)
    vehicles = [vehicle_review(evidence, args.root / "vehicle-ownership-20260930-1" / stage / "stage.json")
                for stage in ("on_foot", "driving_stationary", "driving_moved", "exited")]
    result = {"schema_version": 1, "capture_root": str(args.root.resolve()),
              "build_sha256": IMAGE_SHA256,
              "inventory": {"files": sum(inventory.values()), "bytes": total_bytes,
                            "extensions": dict(sorted(inventory.items()))},
              "hypothesis_offsets": {"world_list_head": hex(HEAD), "pawn_next": hex(NEXT)},
              "identity_captures": identities, "annotations": annotations, "vehicle_captures": vehicles,
              "limitations": ["No live process read or production resolver change",
                              "Numeric offsets apply only to the captured executable hash",
                              "Capture spans are bounded probes, not verified object allocation sizes",
                              "Address distance is measured in bytes; XYZ distance uses uncalibrated game units",
                              "Pair correlations are exploratory; endpoints and repeated captures are dependent"],
              "inputs": dict(sorted(evidence.inputs.items()))}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output.resolve()), "inventory": result["inventory"],
                      "identity_chain_counts": [r["chain_character_count"] for r in identities],
                      "identity_chain_stops": [r["stop_reason"] for r in identities],
                      "inputs_hashed": len(evidence.inputs)}, indent=2))


if __name__ == "__main__":
    main()
