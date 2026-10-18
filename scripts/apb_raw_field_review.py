"""Review +0x84 and string/scalar leads in saved APB captures; no live reads.

Counts describe captured observations, never distinct players. Probe extents
are not verified object sizes. This script assigns no health/death/bone names.
"""

import argparse
import hashlib
import json
import math
import re
import struct
from collections import Counter, defaultdict
from pathlib import Path

from apb_relationship_review import IMAGE_SHA256, DEFAULT_ROOT, number, signature, object_type

TYPES = {
    (0x2AAEFA0, 5778): "character_pawn",
    (0x3142FB0, 4741): "position_component",
    (0x2AE68D8, 294): "other_position_type",
    (0x2AFF240, 1062): "lifecycle_linked_type",
}
SCALARS = (0x328, 0x374, 0x448, 0x644, 0x688, 0x6B4, 0x6B8, 0x6BC, 0xE08, 0xEB8)
INTEGERS = (0x3E8, 0x3EC, 0x424, 0x434, 0x4B4, 0x514)


def integer(body, offset):
    return struct.unpack_from("<I", body, offset)[0]


def identity(record):
    value = record["identity"]
    if value["image_sha256"].upper() != IMAGE_SHA256:
        raise ValueError("Executable hash differs from reviewed build")
    return value["pid"], value["created_filetime"]


def strings(body):
    result = []
    for encoding, pattern in (("ascii", rb"[ -~]{6,}"),
                              ("utf-16-le", rb"(?:[ -~]\x00){6,}")):
        for match in re.finditer(pattern, body):
            text = match.group().decode(encoding)
            if sum(c.isalpha() for c in text) / len(text) < 0.65:
                continue
            result.append({"offset": hex(match.start()), "encoding": encoding,
                           "text": text, "ownership": "unverified_probe_extent"})
    return result


class Review:
    def __init__(self, root):
        self.root = root
        self.inputs = {}
        self.counts = defaultdict(Counter)
        self.instances = defaultdict(set)
        self.examples = defaultdict(list)
        self.annotation_rows = []
        self.identity_rows = []
        self.vehicle_rows = []
        self.string_rows = []
        self.lifecycle_rows = []
        self.context_capture_failures = []

    def read(self, path):
        raw = path.read_bytes()
        self.inputs[str(path.relative_to(self.root))] = {
            "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        return raw

    def json(self, path):
        return json.loads(self.read(path))

    def observation(self, body, address, base, process, source):
        kind = object_type(body, base)
        if kind not in TYPES:
            raise ValueError(f"{source}: unexpected type {kind}")
        value = integer(body, 0x84)
        return {"source": source, "process": list(process), "address": hex(address),
                "type": TYPES[kind], "vtable_rva": hex(kind[0]), "marker": kind[1],
                "signature": signature(body), "field_84": value,
                "field_84_hex": hex(value),
                "set_bits": [hex(1 << b) for b in range(32) if value & (1 << b)]}

    def pawn_row(self, body, address, base, process, source):
        row = self.observation(body, address, base, process, source)
        if row["type"] != "character_pawn":
            raise ValueError(f"{source}: expected character pawn")
        row["xyz"] = list(struct.unpack_from("<fff", body, 0x88))
        row["attachment_178"] = hex(struct.unpack_from("<Q", body, 0x178)[0])
        row["float_candidates"] = {}
        row["integer_candidates"] = {hex(o): integer(body, o) for o in INTEGERS}
        for offset in SCALARS:
            value = struct.unpack_from("<f", body, offset)[0]
            row["float_candidates"][hex(offset)] = value if math.isfinite(value) else None
        return row

    def surveys(self):
        files = sorted(self.root.glob("*/survey-*.candidate-bodies.bin"))
        files += sorted(self.root.glob("*/survey-*.linked-bodies.bin"))
        if not files:
            raise ValueError("No saved survey bodies found")
        seen = set()
        sessions = {}
        for path in files:
            dataset = path.parent.name
            if dataset not in sessions:
                metadata_name = ("identity_inspection.json" if dataset.startswith(
                    ("identity-inspect-", "vehicle-fix-validation-")) else "session.json")
                session = self.json(path.parent / metadata_name)
                sessions[dataset] = number(session["module_base"]), identity(session)
            base, process = sessions[dataset]
            raw = self.read(path)
            if len(raw) % 0x808:
                raise ValueError(f"{path}: incomplete u64-address/0x800-body record")
            for offset in range(0, len(raw), 0x808):
                address = struct.unpack_from("<Q", raw, offset)[0]
                body = raw[offset + 8:offset + 0x808]
                kind = object_type(body, base)
                if kind not in TYPES:
                    raise ValueError(f"{path}: unexpected body type {kind}")
                stamp = signature(body)
                # The pawn body can appear in both candidate and linked files.
                # Preserve differing sequential reads of the same instance.
                key = (dataset, path.name[:11], address, stamp, hashlib.sha256(body).digest())
                if key in seen:
                    continue
                seen.add(key)
                value = integer(body, 0x84)
                group = (dataset, TYPES[kind])
                self.counts[group][value] += 1
                self.instances[group].add((process, address, stamp))
                exkey = (TYPES[kind], value)
                if len(self.examples[exkey]) < 3:
                    self.examples[exkey].append(self.observation(
                        body, address, base, process, str(path.relative_to(self.root))))
                if dataset == "district-lifecycle-20260930-1" and address == 0x2BB04E400:
                    row = self.observation(body, address, base, process, str(path.relative_to(self.root)))
                    row["survey"] = int(path.name[7:11])
                    row["link_f8"] = hex(struct.unpack_from("<Q", body, 0xF8)[0])
                    row["link_420"] = hex(struct.unpack_from("<Q", body, 0x420)[0])
                    row["xyz_d0"] = list(struct.unpack_from("<fff", body, 0xD0))
                    self.lifecycle_rows.append(row)

    def identities(self):
        paths = sorted(self.root.glob("identity-inspect-*/identity_inspection.json"))
        if len(paths) != 6:
            raise ValueError("Expected six identity captures")
        for path in paths:
            report = self.json(path)
            process = identity(report)
            base = number(report["module_base"])
            for obj in report["objects"].values():
                address = number(obj["address"])
                binpath = path.parent / f"object-{address:016X}.bin"
                body = self.read(binpath)
                if (object_type(body, base) != (number(obj["vtable_rva"]), obj["marker"])
                        or signature(body) != obj["instance_signature"]):
                    raise ValueError(f"{binpath}: saved header differs from metadata")
                if object_type(body, base) == (0x2AAEFA0, 5778):
                    self.identity_rows.append(self.pawn_row(
                        body, address, base, process, str(binpath.relative_to(self.root))))
                    for candidate in strings(body):
                        self.string_rows.append({"source": str(binpath.relative_to(self.root)),
                            "object_address": hex(address), "kind": "inline_decoding", **candidate})
                for candidate in obj.get("possible_strings", []):
                    text = candidate["text"]
                    if len(text) < 6 or not text.isascii() or not any(c.isalpha() for c in text):
                        continue
                    field = number(candidate["offset"])
                    pointer, count, capacity = struct.unpack_from("<Qii", body, field)
                    if (pointer, count, capacity) != (
                            number(candidate["address"]), candidate["count"], candidate["capacity"]):
                        raise ValueError(f"{binpath}: string descriptor differs from metadata")
                    self.string_rows.append({"source": str(path.relative_to(self.root)),
                        "object_address": hex(address), "vtable_rva": obj["vtable_rva"],
                        "marker": obj["marker"], "kind": "saved_pointer_string_metadata",
                        **candidate, "ownership": "unverified_probe_extent"})

    def annotations(self):
        paths = sorted((self.root / "mission-labels-20260930-2").glob("annotation-*.json"))
        if len(paths) != 14:
            raise ValueError("Expected 14 mission-label annotations")
        for path in paths:
            report = self.json(path)
            process = identity(report)
            session = self.json(path.parent / "session.json")
            base = number(session["module_base"])
            if process != identity(session):
                raise ValueError(f"{path}: process differs from session")
            for role, bodyhex, lead in (
                    ("labelled", report["body_hex"], report["point"]),
                    ("local", report["local_character_context"]["body_hex"],
                     report["local_character_context"]["lead"])):
                body = bytes.fromhex(bodyhex)
                row = self.pawn_row(body, number(lead["address"]), base, process,
                                    str(path.relative_to(self.root)))
                if role == "labelled" and signature(body) != lead["instance_signature"]:
                    raise ValueError(f"{path}: annotation fingerprint differs")
                row["role"] = role
                row["operator_label"] = report["label"]
                row["label_note"] = "annotation-0012 later corrected by user to opposition"
                self.annotation_rows.append(row)
            for context in report["pointer_context_unclassified"]:
                if "read_error" in context:
                    self.context_capture_failures.append({"source": str(path.relative_to(self.root)),
                                                          **context})
                    continue
                for candidate in strings(bytes.fromhex(context["body_hex"])):
                    self.string_rows.append({"source": str(path.relative_to(self.root)),
                        "object_address": context["pointer"], "pawn_pointer_field": context["offset"],
                        "kind": "pointer_context_inline_decoding", **candidate})

    def vehicles(self):
        root = self.root / "vehicle-ownership-20260930-1"
        session = self.json(root / "session.json")
        base = number(session["module_base"])
        for stage in ("on_foot", "driving_stationary", "driving_moved", "exited"):
            path = root / stage / "stage.json"
            report = self.json(path)
            process = identity(report)
            for obj in report["objects"]:
                if (number(obj["vtable_rva"]), obj["marker"]) != (0x2AAEFA0, 5778):
                    continue
                binpath = path.parent / f"object-{number(obj['address']):016X}.bin"
                body = self.read(binpath)
                if signature(body) != obj["instance_signature"]:
                    raise ValueError(f"{binpath}: vehicle-stage fingerprint differs")
                row = self.pawn_row(body, number(obj["address"]), base, process,
                                    str(binpath.relative_to(self.root)))
                row["stage"] = stage
                self.vehicle_rows.append(row)

    def result(self):
        return {"status": "offline_raw_values_semantics_unverified", "root": str(self.root),
            "image_sha256": IMAGE_SHA256,
            "limitations": ["No live memory reads; only recorded build and states",
                "Survey counts are repeated object observations, not player counts",
                "Exact candidate/linked duplicates merged by dataset/survey/address/signature/body; differing reads retained",
                "Probe extents are not verified object sizes; strings may be neighboring allocations",
                "Numeric fields have no newly validated health, death, bone or enum names"],
            "survey_distributions": [{"dataset": d, "type": t,
                "instance_fingerprints": len(self.instances[(d, t)]),
                "observations": sum(c.values()),
                "values": {hex(v): n for v, n in sorted(c.items())}}
                for (d, t), c in sorted(self.counts.items())],
            "field_84_examples": [{"type": t, "value": hex(v), "examples": examples}
                for (t, v), examples in sorted(self.examples.items())],
            "identity_pawns": self.identity_rows, "mission_annotations": self.annotation_rows,
            "vehicle_stages": self.vehicle_rows,
            "lifecycle_lead": sorted(self.lifecycle_rows, key=lambda x: x["survey"]),
            "string_candidates": self.string_rows,
            "context_capture_failures": self.context_capture_failures, "inputs": self.inputs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Choose a new output path; existing evidence is never overwritten")
    review = Review(args.root)
    review.surveys()
    review.identities()
    review.annotations()
    review.vehicles()
    result = review.result()
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps({"inputs": len(review.inputs), "survey_observations": sum(
        sum(c.values()) for c in review.counts.values()), "identity_pawns": len(review.identity_rows),
        "annotation_rows": len(review.annotation_rows), "vehicle_rows": len(review.vehicle_rows),
        "string_candidates": len(review.string_rows),
        "saved_context_capture_failures": len(review.context_capture_failures), "output": str(args.output)}))


if __name__ == "__main__":
    main()
