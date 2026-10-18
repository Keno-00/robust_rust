"""Offline four-state raw evidence review; never reads a live process.

Bit patterns are correlations only, not validated LOS fields. Capture extents
are bounded probes and are not established native object sizes.
"""

import argparse
import hashlib
import json
import math
import struct
from datetime import datetime
from pathlib import Path


SOURCES = (("visibility-20261002-133641", "frame-0001", "partial_cursor_visible"),
           ("visibility-20261002-134307", "frame-0001", "partial_cursor_invisible"),
           ("visibility-20261002-140014", "frame-0001", "visible"),
           ("visibility-20261002-140014", "frame-0002", "invisible"))
SUBJECT = "0x9EBD10C0"


def compare(blobs):
    extent = min(map(len, blobs))
    changed = []
    patterns = {"some_region_unblocked_operator_labels": [1, 1, 1, 0],
                "cursor_on_unblocked_region_operator_labels": [1, 0, 1, 0]}
    matches = []
    for offset in range(0, extent - 3, 4):
        values = [struct.unpack_from("<I", blob, offset)[0] for blob in blobs]
        if len(set(values)) == 1:
            continue
        changed.append({"offset": hex(offset), "raw_u32": [hex(value) for value in values]})
        for bit in range(32):
            states = [(value >> bit) & 1 for value in values]
            for name, desired in patterns.items():
                if states == desired or states == [1-value for value in desired]:
                    matches.append({"offset": hex(offset), "bit": bit, "pattern": name,
                                    "states": states, "polarity": "direct" if states == desired else "inverse",
                                    "raw_u32": [hex(value) for value in values],
                                    "status": "four_sample_correlation_not_semantic_evidence"})
    return {"extent": extent, "changed_u32_count": len(changed), "changed_u32": changed,
            "pattern_correlations": matches,
            "limit": "No object size or field type established. Float, pointer and timestamp bits "
                     "can match by chance; no correlation is promoted to LOS."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output already exists")
    frames, points, metadata = [], [], []
    signatures, identities = set(), set()
    for session, frame, observation in SOURCES:
        directory = args.capture_root / session / frame
        capture_bytes = (directory / "capture.json").read_bytes()
        annotations_bytes = (directory / "annotations.json").read_bytes()
        capture = json.loads(capture_bytes)
        annotations = json.loads(annotations_bytes)
        point = next(p for p in capture["points"] if p["address"] == SUBJECT)
        label = next(p for p in annotations["labels"] if p["address"] == SUBJECT)
        if label["observation"] != observation or label["instance_signature"] != point["instance_signature"]:
            raise RuntimeError(f"Label/source mismatch: {directory}")
        signatures.add(point["instance_signature"])
        identities.add(json.dumps(capture["identity"], sort_keys=True))
        camera, after = capture["camera"], capture["camera_after"]
        movement = math.dist(camera["position"], after["position"])
        refs = point["evidence"]["pointer_context_unclassified"]
        image_hashes = {name: hashlib.sha256(Path(data["path"]).read_bytes()).hexdigest()
                        for name, data in capture["images"].items()}
        metadata.append({"directory": str(directory), "observation": observation,
                         "capture_sha256": hashlib.sha256(capture_bytes).hexdigest(),
                         "annotations_sha256": hashlib.sha256(annotations_bytes).hexdigest(),
                         "image_sha256": image_hashes, "label": label,
                         "projection": point["projection"], "ownership": point["ownership"],
                         "duration_seconds": (datetime.fromisoformat(capture["completed_utc"])
                                              - datetime.fromisoformat(capture["started_utc"])).total_seconds(),
                         "started_utc": capture["started_utc"],
                         "completed_utc": capture["completed_utc"],
                         "camera_position_delta_world_units": movement,
                         "camera_matrix_max_delta": max(abs(a-b) for a, b in zip(camera["matrix"], after["matrix"])),
                         "failed_survey_headers": capture["survey"]["failed_headers"],
                         "rejected_candidates": capture["rejected"],
                         "failed_pointer_probes": sum("read_error" in ref for ref in refs),
                         "typed_contexts": sum("body_hex" in ref for ref in refs)})
        frames.append(capture)
        points.append(point)
    if len(signatures) != 1 or len(identities) != 1:
        raise RuntimeError("Subject instance or process/build differs between captures")
    regions = {"pawn_bounded_probe": compare([bytes.fromhex(p["evidence"]["body_hex"]) for p in points])}
    refs_by_frame = [{(r["offset"], r["pointer"], r.get("vtable_rva")): r
                      for r in p["evidence"]["pointer_context_unclassified"] if "body_hex" in r}
                     for p in points]
    common = set.intersection(*(set(refs) for refs in refs_by_frame))
    for key in sorted(common):
        if key[1].lower() == SUBJECT.lower():
            continue
        regions[f"pawn{key[0]}->{key[1]}_vtable_{key[2]}"] = compare(
            [bytes.fromhex(refs[key]["body_hex"]) for refs in refs_by_frame])
    components = set.intersection(*(set(p["ownership"]["components"]) for p in points))
    for component in sorted(components):
        blobs = []
        for capture in frames:
            sequence = capture["survey"]["sequence"]
            directory = Path(capture["images"]["before"]["path"]).parent
            raw = (directory / f"survey-{sequence:04d}.candidate-bodies.bin").read_bytes()
            stride = 8 + 0x800
            if len(raw) % stride:
                raise RuntimeError("Candidate body record length invalid")
            found = [raw[offset+8:offset+stride] for offset in range(0, len(raw), stride)
                     if struct.unpack_from("<Q", raw, offset)[0] == int(component, 16)]
            if len(found) != 1:
                raise RuntimeError(f"Expected exactly one component body: {component}")
            blobs.append(found[0])
        # A reciprocal membership check in each frame validates the live owner,
        # while these earlier sequential survey bodies retain independent timing.
        regions[f"registered_component_{component}_survey_probe"] = compare(blobs)
    report = {"format": "apb-los-four-state-offline-review-v1", "subject": SUBJECT,
              "same_instance": True, "same_process_and_build": True,
              "frames": metadata, "regions": regions,
              "glass_semantics": "Operator reports solid glass blocks shooting; optical visibility "
                                 "is distinct. Collision/penetration not independently measured.",
              "conclusion": "No LOS field, raycast implementation or bone mapping is validated "
                            "by these four observations. Correlations require repeat transitions, "
                            "other subjects and independent evaluation."}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    for name, region in regions.items():
        print(f"{name}: {region['changed_u32_count']} changed words; "
              f"{len(region['pattern_correlations'])} unvalidated bit correlations")
    print(f"Saved {args.output}")


if __name__ == "__main__":
    main()
