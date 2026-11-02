"""User-paced recheck of unknown-value APB cursor candidates via EchoDriver."""

import argparse
import json
import struct
from datetime import datetime, timezone
from pathlib import Path

from apb_cursor_scan import Session
from apb_cursor_unknown import TRIPLE, wait_f8


PAGE_SIZE = 4096


def read_phase(session, records, label):
    session.require_foreground()
    kept = []
    pages_read = pages_failed = 0
    current_page = -1
    page_bytes = None
    value_index = {"white": 1, "red": 2, "green": 3}[label]
    for record in records:
        address = record[0]
        page = address & ~(PAGE_SIZE - 1)
        if page != current_page:
            current_page = page
            try:
                page_bytes = session.read(page, PAGE_SIZE)
                pages_read += 1
            except (OSError, RuntimeError):
                page_bytes = None
                pages_failed += 1
            if (pages_read + pages_failed) % 200 == 0:
                session.require_foreground()
        if page_bytes is None:
            continue
        (observed,) = struct.unpack_from("<I", page_bytes, address - page)
        if observed == record[value_index]:
            kept.append(record)
    session.require_foreground()
    print(f"{label.upper()} recheck: {len(kept)} of {len(records)} candidates, {pages_read} pages read, {pages_failed} pages failed.", flush=True)
    return kept, {"input": len(records), "kept": len(kept), "pages_read": pages_read, "pages_failed": pages_failed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output must be a new path")
    source_meta = json.loads((args.source_directory / "green.json").read_text(encoding="utf-8"))
    if source_meta.get("format") != "apb-cursor-unknown-v1":
        raise ValueError("Unsupported candidate format")
    raw = (args.source_directory / "green.candidates").read_bytes()
    if len(raw) != source_meta["candidate_count"] * TRIPLE.size:
        raise ValueError("Candidate file size does not match metadata")
    records = list(struct.iter_unpack("<QIII", raw))
    session = Session(args.pid)
    try:
        if session.identity != source_meta["identity"]:
            raise ValueError("Candidates belong to another APB process launch")
        stats = {}
        for label in ("white", "red", "green"):
            wait_f8(session, label)
            print(f"Rechecking {label.upper()} now; keep the cursor steady.", flush=True)
            records, stats[label] = read_phase(session, records, label)
            if not records:
                raise RuntimeError(f"No candidates survived {label}; check the displayed cursor state and capture coverage")
        report = {
            "format": "apb-cursor-recheck-v1",
            "captured_utc": datetime.now(timezone.utc).isoformat(),
            "identity": session.identity,
            "source": str(args.source_directory),
            "stats": stats,
            "candidates": [
                {"address": f"0x{address:X}", "white": white, "red": red, "green": green}
                for address, white, red, green in records
            ],
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
        triples = [item for item in records if len(set(item[1:])) == 3 and max(item[1:]) <= 255]
        print(f"Recheck complete: {len(records)} total survivors, {len(triples)} distinct three-state small integers.", flush=True)
        for address, white, red, green in triples[:80]:
            print(f"  0x{address:X}: W={white} R={red} G={green}", flush=True)
        print(f"All survivors: {args.output}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Cursor recheck interrupted.")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Cursor recheck failed: {error}")
