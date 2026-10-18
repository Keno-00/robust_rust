"""One F8-paced EchoDriver scan for the prior camera marker outside the observed table."""

import argparse
import math
import struct
import sys
from pathlib import Path

import numpy as np

from apb_f8_common import CAMERA_CLASS, Session, client_capture, module_base, utc_now, wait_f8, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    session = Session(args.pid)
    try:
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        print("Camera marker scan: stand still in APB and keep the view steady. "
              "F8 starts one scan of writable pages through EchoDriver.", flush=True)
        wait_f8(session, "camera marker scan")
        started = utc_now()
        stats = {"regions": 0, "pages_read": 0, "pages_failed": 0, "bytes_read": 0,
                 "raw_marker_hits": 0, "candidate_object_reads_failed": 0}
        leads = []
        seen = set()
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
                          f"{len(leads)} readable marker leads.", flush=True)
                    next_progress += 256 * 1024 * 1024
                values = np.frombuffer(data[:size // 4 * 4], dtype="<u4")
                for index in np.flatnonzero(values == CAMERA_CLASS):
                    stats["raw_marker_hits"] += 1
                    marker_address = address + int(index) * 4
                    object_address = marker_address - 0x3C
                    if object_address < 0x10000 or object_address % 8 or object_address in seen:
                        continue
                    seen.add(object_address)
                    try:
                        header = session.read(object_address, 0x40)
                        if struct.unpack_from("<I", header, 0x3C)[0] != CAMERA_CLASS:
                            continue
                        block = session.read(object_address + 0x680, 0x50)
                    except (OSError, RuntimeError):
                        stats["candidate_object_reads_failed"] += 1
                        continue
                    matrix = struct.unpack_from("<16f", block, 0)
                    position = struct.unpack_from("<3f", block, 0x40)
                    leads.append({"object": f"0x{object_address:X}", "marker_field": f"0x{marker_address:X}",
                                  "header_hex": header.hex(), "matrix": matrix, "position": position,
                                  "finite": all(math.isfinite(value) for value in (*matrix, *position))})
            session.require_foreground()
        image = client_capture(session, args.output_directory / "marker_scan.png")
        shape_matches = [lead["object"] for lead in leads
                         if lead["finite"] and abs(lead["matrix"][14] + 9.99) < 0.02
                         and abs(lead["matrix"][15]) < 0.02]
        report = {"format": "apb-camera-marker-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "marker": CAMERA_CLASS,
                  "stats": stats, "leads": leads, "image": image,
                  "shape_signature": {"matrix_14_near": -9.99, "matrix_15_near": 0.0,
                                      "absolute_tolerance": 0.02},
                  "shape_matches": shape_matches,
                  "status": "unique_camera_shape_unverified" if len(shape_matches) == 1
                            else "ambiguous_camera_shape"}
        write_json(args.output_directory / "marker_scan.json", report)
        print(f"CAPTURE COMPLETE: {stats['bytes_read'] // (1024 * 1024)} MiB read, "
              f"{len(leads)} readable marker leads, {len(shape_matches)} camera-shape matches, "
              f"{stats['pages_failed']} failed pages.", flush=True)
        if len(shape_matches) == 1:
            print(f"  Provisional camera object: {shape_matches[0]}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
        if len(shape_matches) != 1:
            raise RuntimeError(f"Expected one camera-shape match, found {len(shape_matches)}; "
                               f"inspect {args.output_directory / 'marker_scan.json'}")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Camera marker scan interrupted; completed files remain for diagnosis.", file=sys.stderr)
        raise SystemExit(1)
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera marker scan failed: {error}")
