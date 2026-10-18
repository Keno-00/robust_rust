"""One F8-paced EchoDriver scan for a typed root-to-camera ownership chain."""

import argparse
import json
import math
import struct
from pathlib import Path

import numpy as np

from apb_camera_chain_follow_f8 import follow_root
from apb_f8_common import Session, client_capture, module_base, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_chain", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    source = json.loads(args.source_chain.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-chain-refs-f8-v1":
        raise RuntimeError("Source is not a validated camera-chain reference report")
    links = source["links"]
    if not links or len(links) > 12:
        raise RuntimeError("Source has an invalid ownership chain")
    root_class = links[0]["class"]
    root_vtable_rva = int(links[0]["vtable_rva"], 16)
    session = Session(args.pid)
    try:
        if source["identity"]["image_sha256"] != session.identity["image_sha256"]:
            raise RuntimeError("Camera ownership chain was derived from another APB build")
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        print(f"Scanning for class {root_class} with vtable APB.exe+0x{root_vtable_rva:X}, "
              f"then {len(links)} typed links to camera. Keep APB foreground "
              "until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, "camera ownership-root scan")
        started = utc_now()
        stats = {"regions": 0, "pages_read": 0, "pages_failed": 0,
                 "bytes_read": 0, "raw_class_hits": 0, "typed_roots": 0}
        seen = set()
        candidates = {}
        rejected = []
        next_progress = 256 * 1024 * 1024
        for start, end in session.writable_regions():
            stats["regions"] += 1
            for address in range(start, end, 4096):
                size = min(4096, end - address)
                try:
                    data = session.read(address, size)
                except (OSError, RuntimeError):
                    stats["pages_failed"] += 1
                    continue
                stats["pages_read"] += 1
                stats["bytes_read"] += size
                if stats["bytes_read"] >= next_progress:
                    print(f"  Scanned {stats['bytes_read'] // (1024 * 1024)} MiB; "
                          f"{len(candidates)} complete chains.", flush=True)
                    next_progress += 256 * 1024 * 1024
                values = np.frombuffer(data[:size // 4 * 4], dtype="<u4")
                for index in np.flatnonzero(values == root_class):
                    stats["raw_class_hits"] += 1
                    root = address + int(index) * 4 - 0x3C
                    if root < 0x10000 or root % 8 or root in seen:
                        continue
                    seen.add(root)
                    try:
                        (vtable,) = struct.unpack("<Q", session.read(root, 8))
                        if vtable != base + root_vtable_rva:
                            continue
                        stats["typed_roots"] += 1
                        candidate = follow_root(session, base, root, links)
                        candidates[root] = candidate
                    except (OSError, RuntimeError, ValueError, struct.error) as error:
                        if len(rejected) < 1000:
                            rejected.append({"root": f"0x{root:X}", "error": str(error)})
            session.require_foreground()
        image = client_capture(session, args.output_directory / "root_scan.png")
        after = {}
        for root in candidates:
            try:
                after[f"0x{root:X}"] = follow_root(session, base, root, links)
            except (OSError, RuntimeError, ValueError, struct.error) as error:
                after[f"0x{root:X}"] = {"error": str(error)}
        shape_matches = [f"0x{root:X}" for root, candidate in candidates.items()
                         if all(math.isfinite(v) for v in (*candidate["camera"]["matrix"],
                                                           *candidate["camera"]["position"]))
                         and abs(candidate["camera"]["matrix"][14] + 9.99) < 0.02
                         and abs(candidate["camera"]["matrix"][15]) < 0.02]
        report = {"format": "apb-camera-root-scan-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_chain": str(args.source_chain),
                  "links": links, "stats": stats, "rejected_roots": rejected,
                  "candidates": [candidates[key] for key in sorted(candidates)],
                  "candidates_after_image": after, "shape_matches": shape_matches,
                  "image": image, "status": "unique_root_shape_unverified"
                  if len(shape_matches) == 1 else "ambiguous_root_shape"}
        write_json(args.output_directory / "root_scan.json", report)
        print(f"CAPTURE COMPLETE: {stats['bytes_read'] // (1024 * 1024)} MiB read, "
              f"{stats['typed_roots']} typed roots, {len(candidates)} complete chains, "
              f"{len(shape_matches)} camera-shape matches, "
              f"{stats['pages_failed']} failed pages.", flush=True)
        for root in shape_matches:
            candidate = candidates[int(root, 16)]
            print(f"  Root {root} -> camera {candidate['camera']['address']}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if len(shape_matches) != 1:
            raise RuntimeError(f"Expected one typed camera root, found {len(shape_matches)}; "
                               "inspect saved root_scan.json")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera ownership-root scan interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera ownership-root scan failed: {error}")
