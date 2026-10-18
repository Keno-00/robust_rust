"""Find pointers near a verified cursor field in a saved EchoDriver page scan.

These are one-session pointer leads. Module ownership and restart stability must
be checked separately before a chain can be used in the product.
"""

import argparse
import json
import struct
from datetime import datetime, timezone
from pathlib import Path

import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("snapshot_directory", type=Path)
    parser.add_argument("target", type=lambda value: int(value, 0))
    parser.add_argument("output", type=Path)
    parser.add_argument("--radius", type=lambda value: int(value, 0), default=0x1000)
    args = parser.parse_args()
    if args.output.exists() or not 0 < args.target < 0x800000000000 or not 0 <= args.radius <= 0x10000:
        parser.error("Use a valid target/radius and a new output path")
    metadata = json.loads((args.snapshot_directory / "white1.json").read_text(encoding="utf-8"))
    if metadata.get("format") != "apb-cursor-unknown-v1":
        raise ValueError("Unsupported EchoDriver snapshot format")
    source = args.snapshot_directory / "white1.pages"
    hits = []
    pages = 0
    low = args.target - args.radius
    with source.open("rb", buffering=1024 * 1024) as stream:
        while header := stream.read(12):
            if len(header) != 12:
                raise ValueError("Truncated page header")
            address, size = struct.unpack("<QI", header)
            data = stream.read(size)
            if len(data) != size or size == 0 or size > 4096:
                raise ValueError(f"Truncated or invalid page at 0x{address:X}")
            aligned = size // 8 * 8
            values = np.frombuffer(data[:aligned], dtype="<u8")
            indices = np.flatnonzero((values >= low) & (values <= args.target) & ((values & 7) == 0))
            for index in indices:
                pointer = int(values[index])
                hits.append({
                    "source": f"0x{address + int(index) * 8:X}",
                    "pointee": f"0x{pointer:X}",
                    "field_offset": f"0x{args.target - pointer:X}",
                })
            pages += 1
            if pages % 200000 == 0:
                print(f"Scanned {pages} pages; found {len(hits)} nearby pointers", flush=True)
    report = {
        "format": "apb-cursor-pointer-refs-v1",
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "identity": metadata["identity"],
        "target": f"0x{args.target:X}",
        "radius": f"0x{args.radius:X}",
        "pages_scanned": pages,
        "hits": hits,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"Found {len(hits)} nearby pointers across {pages} saved pages; output={args.output}")
    for hit in hits[:80]:
        print(f"  {hit['source']} -> {hit['pointee']} + {hit['field_offset']}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Pointer reference scan failed: {error}")
