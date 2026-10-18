"""User-paced, read-only APB cursor discovery through EchoDriver.

Keep APB foreground and press F8 when each requested cursor color is steady.
The first white and red captures scan writable memory; later captures read only
changed candidate pages. No address is accepted as a product offset here.
"""

import argparse
import ctypes
import json
import struct
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

from apb_cursor_scan import Session


PAGE_SIZE = 4096
CANDIDATE = struct.Struct("<QII")
TRIPLE = struct.Struct("<QIII")
F8 = 0x77


def metadata(session, phase, stats):
    return {
        "format": "apb-cursor-unknown-v1",
        "captured_utc": datetime.now(timezone.utc).isoformat(),
        "identity": session.identity,
        "phase": phase,
        **stats,
    }


def write_meta(path, content):
    path.write_text(json.dumps(content, indent=2), encoding="utf-8")


def wait_f8(session, label):
    user32 = session.user32
    user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
    user32.GetAsyncKeyState.restype = ctypes.c_short
    print(f"\nPrepare {label} in APB, then press F8. Wait for the capture-complete message before changing state.", flush=True)
    while user32.GetAsyncKeyState(F8) & 0x8000:
        time.sleep(0.05)
    while True:
        if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
            raise RuntimeError("APB process exited while waiting for F8")
        if user32.GetAsyncKeyState(F8) & 0x8000:
            try:
                session.require_foreground()
            except RuntimeError:
                print("F8 ignored: APB is not foreground. Refocus the game.", flush=True)
                while user32.GetAsyncKeyState(F8) & 0x8000:
                    time.sleep(0.05)
            else:
                while user32.GetAsyncKeyState(F8) & 0x8000:
                    time.sleep(0.05)
                return
        time.sleep(0.05)


def capture_baseline(session, directory):
    output = directory / "white1.pages"
    temporary = directory / "white1.pages.partial"
    stats = {"regions": 0, "pages_read": 0, "pages_failed": 0, "bytes_read": 0}
    with temporary.open("xb", buffering=1024 * 1024) as stream:
        for start, end in session.writable_regions():
            stats["regions"] += 1
            for address in range(start, end, PAGE_SIZE):
                size = min(PAGE_SIZE, end - address)
                try:
                    data = session.read(address, size)
                except (OSError, RuntimeError):
                    stats["pages_failed"] += 1
                    continue
                stream.write(struct.pack("<QI", address, size))
                stream.write(data)
                stats["pages_read"] += 1
                stats["bytes_read"] += size
            if stats["regions"] % 200 == 0:
                session.require_foreground()
                print(f"  White: {stats['bytes_read'] // (1024 * 1024)} MiB, {stats['regions']} regions", flush=True)
    session.require_foreground()
    temporary.replace(output)
    write_meta(directory / "white1.json", metadata(session, "white1", stats))
    print(f"WHITE complete: {stats['bytes_read'] // (1024 * 1024)} MiB, {stats['pages_failed']} failed pages.", flush=True)


def compare_red(session, directory):
    baseline = json.loads((directory / "white1.json").read_text(encoding="utf-8"))
    if baseline["identity"] != session.identity:
        raise RuntimeError("White snapshot belongs to another process launch")
    temporary = directory / "red1.candidates.partial"
    output = directory / "red1.candidates"
    stats = {"pages_read": 0, "pages_failed": 0, "candidate_count": 0}
    with (directory / "white1.pages").open("rb", buffering=1024 * 1024) as source, temporary.open("xb", buffering=1024 * 1024) as stream:
        while header := source.read(12):
            if len(header) != 12:
                raise RuntimeError("White page snapshot has a truncated header")
            address, size = struct.unpack("<QI", header)
            old = source.read(size)
            if len(old) != size or size == 0 or size > PAGE_SIZE:
                raise RuntimeError(f"White page snapshot is truncated at 0x{address:X}")
            try:
                new = session.read(address, size)
            except (OSError, RuntimeError):
                stats["pages_failed"] += 1
                continue
            stats["pages_read"] += 1
            aligned = size // 4 * 4
            before = np.frombuffer(old[:aligned], dtype="<u4")
            after = np.frombuffer(new[:aligned], dtype="<u4")
            changed = np.flatnonzero((before != after) & (before <= 65535) & (after <= 65535))
            for index in changed:
                stream.write(CANDIDATE.pack(address + int(index) * 4, int(before[index]), int(after[index])))
            stats["candidate_count"] += len(changed)
            if stats["pages_read"] % 100000 == 0:
                session.require_foreground()
                print(f"  Red: {stats['pages_read']} pages, {stats['candidate_count']} changed small integers", flush=True)
    session.require_foreground()
    temporary.replace(output)
    write_meta(directory / "red1.json", metadata(session, "red1", stats))
    print(f"RED complete: {stats['candidate_count']} changed small integers, {stats['pages_failed']} failed pages.", flush=True)


def filter_candidates(session, directory, source_name, output_name, expected):
    source_meta = json.loads((directory / f"{source_name}.json").read_text(encoding="utf-8"))
    if source_meta["identity"] != session.identity:
        raise RuntimeError("Candidate list belongs to another process launch")
    source = directory / f"{source_name}.candidates"
    output = directory / f"{output_name}.candidates"
    temporary = directory / f"{output_name}.candidates.partial"
    stats = {"previous_candidates": source_meta["candidate_count"], "candidate_count": 0, "pages_read": 0, "pages_failed": 0}
    if source.stat().st_size != source_meta["candidate_count"] * CANDIDATE.size:
        raise RuntimeError(f"Candidate file size mismatch: {source}")
    with source.open("rb", buffering=1024 * 1024) as old, temporary.open("xb", buffering=1024 * 1024) as stream:
        previous = -1
        current_page = -1
        page_bytes = None
        while record := old.read(CANDIDATE.size):
            address, white, red = CANDIDATE.unpack(record)
            if address <= previous or address % 4:
                raise RuntimeError(f"Candidate list is unsorted or unaligned at 0x{address:X}")
            previous = address
            page = address & ~(PAGE_SIZE - 1)
            if page != current_page:
                current_page = page
                try:
                    page_bytes = session.read(page, PAGE_SIZE)
                    stats["pages_read"] += 1
                except (OSError, RuntimeError):
                    page_bytes = None
                    stats["pages_failed"] += 1
                if stats["pages_read"] and stats["pages_read"] % 10000 == 0:
                    session.require_foreground()
                    print(f"  {output_name}: {stats['pages_read']} pages, {stats['candidate_count']} candidates", flush=True)
            if page_bytes is not None:
                (observed,) = struct.unpack_from("<I", page_bytes, address - page)
                wanted = white if expected == "white" else red
                if observed == wanted:
                    stream.write(record)
                    stats["candidate_count"] += 1
        session.require_foreground()
    temporary.replace(output)
    write_meta(directory / f"{output_name}.json", metadata(session, output_name, stats))
    print(f"{output_name.upper()} complete: {stats['candidate_count']} of {stats['previous_candidates']} candidates, {stats['pages_failed']} failed pages.", flush=True)


def capture_green(session, directory):
    source_meta = json.loads((directory / "red2.json").read_text(encoding="utf-8"))
    source = directory / "red2.candidates"
    if source_meta["identity"] != session.identity or source.stat().st_size != source_meta["candidate_count"] * CANDIDATE.size:
        raise RuntimeError("Red candidate list is invalid or belongs to another process")
    stats = {"previous_candidates": source_meta["candidate_count"], "candidate_count": 0, "pages_read": 0, "pages_failed": 0}
    results = []
    with source.open("rb", buffering=1024 * 1024) as old, (directory / "green.candidates.partial").open("xb", buffering=1024 * 1024) as stream:
        current_page = -1
        page_bytes = None
        while record := old.read(CANDIDATE.size):
            address, white, red = CANDIDATE.unpack(record)
            page = address & ~(PAGE_SIZE - 1)
            if page != current_page:
                current_page = page
                try:
                    page_bytes = session.read(page, PAGE_SIZE)
                    stats["pages_read"] += 1
                except (OSError, RuntimeError):
                    page_bytes = None
                    stats["pages_failed"] += 1
                if stats["pages_read"] and stats["pages_read"] % 10000 == 0:
                    session.require_foreground()
                    print(f"  Green: {stats['pages_read']} pages, {stats['candidate_count']} candidates", flush=True)
            if page_bytes is None:
                continue
            (green,) = struct.unpack_from("<I", page_bytes, address - page)
            stream.write(TRIPLE.pack(address, white, red, green))
            stats["candidate_count"] += 1
            if green <= 65535:
                results.append((address, white, red, green))
        session.require_foreground()
    (directory / "green.candidates.partial").replace(directory / "green.candidates")
    write_meta(directory / "green.json", metadata(session, "green", stats))
    results.sort(key=lambda item: (item[1] == item[3], max(item[1:]), item[0]))
    print(f"GREEN complete: {stats['candidate_count']} remaining candidates, {stats['pages_failed']} failed pages.", flush=True)
    print("Top values (white, red, green); these are leads, not verified offsets:", flush=True)
    for address, white, red, green in results[:60]:
        print(f"  0x{address:X}: {white}, {red}, {green}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    session = Session(args.pid)
    try:
        args.output_directory.mkdir(parents=True)
        phases = [
            ("white", lambda: capture_baseline(session, args.output_directory)),
            ("red", lambda: compare_red(session, args.output_directory)),
            ("white", lambda: filter_candidates(session, args.output_directory, "red1", "white2", "white")),
            ("red", lambda: filter_candidates(session, args.output_directory, "white2", "red2", "red")),
            ("green", lambda: capture_green(session, args.output_directory)),
        ]
        print("F8 starts each capture only while APB is foreground. You can take as long as needed between states.", flush=True)
        for label, operation in phases:
            wait_f8(session, label)
            print(f"Capturing {label.upper()} now; keep the cursor steady.", flush=True)
            operation()
        print("All captures complete. Press Enter to close this terminal or review the printed leads.", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Cursor discovery interrupted; partial outputs remain for diagnosis.")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Cursor discovery failed: {error}")
