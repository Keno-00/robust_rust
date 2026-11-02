"""Review recorded mission labels against an explicit mission-field hypothesis."""

import argparse
import json
from collections import Counter
from pathlib import Path

from apb_f8_common import write_json
from apb_mission_classification import decode, classify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_directory", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--correct-last-pawn-opposition", action="store_true",
                        help="Apply the user's explicit correction to the simultaneous final role labels")
    args = parser.parse_args()
    entries = []
    for path in sorted(args.capture_directory.glob("annotation-*.json")):
        record = json.loads(path.read_text(encoding="utf-8"))
        raw_label = record["label"]
        if raw_label not in ("teammate", "opposition", "outside_mission"):
            continue
        label = raw_label
        correction = None
        if args.correct_last_pawn_opposition and path.name == "annotation-0012.json":
            if record["point"]["address"] != "0x16EAB90C0" or raw_label != "teammate":
                raise RuntimeError("Correction does not match the explicitly identified annotation")
            label = "opposition"
            correction = "User confirmed the last pawn was an opponent; simultaneous role-key event"
        remote = decode(bytes.fromhex(record["body_hex"])[0x990:0x9A0])
        local = decode(bytes.fromhex(record["local_character_context"]["body_hex"])[0x990:0x9A0])
        prediction = classify(local, remote)
        entries.append({"file": path.name, "utc": record["utc"], "address": record["point"]["address"],
            "instance_signature": record["point"]["instance_signature"],
            "raw_label": raw_label, "ground_truth": label, "correction": correction,
            "operator_context": record.get("mission_context"), "local_fields": local,
            "remote_fields": remote, "prediction": prediction,
            "matches": label == prediction["role"], "slot_rvas": record["lead"]["slot_rvas"]})
    state_entries = []
    for path in sorted(args.capture_directory.glob("mission-state-*.json")):
        record = json.loads(path.read_text(encoding="utf-8"))
        fields = decode(bytes.fromhex(record["local_character_context"]["body_hex"])[0x990:0x9A0])
        state_entries.append({"file": path.name, "operator_context": record["mission_context"], "fields": fields})
    report = {"status": "labelled_evidence_fit_not_independent_live_validation",
        "ground_truth_policy": "Player-role labels override delayed operator mission-state annotation",
        "events": len(entries), "distinct_pawn_instances": len({(e['address'],e['instance_signature']) for e in entries}),
        "labels": dict(Counter(e["ground_truth"] for e in entries)),
        "matches": sum(e["matches"] for e in entries), "entries": entries, "state_snapshots": state_entries,
        "limitations": ["Only observed side values 0/1 and known inactive states accepted",
                        "Candidate token semantics, side reversal and other concurrent missions require live validation",
                        "No persistent player module root inferred from inline table slots"]}
    write_json(args.output, report)
    print(json.dumps({key: report[key] for key in ("events", "distinct_pawn_instances", "labels", "matches")}))


if __name__ == "__main__":
    main()
