"""Capture new-build module arrays and provisional object positions through EchoDriver."""

import argparse
import hashlib
import json
import math
import struct
import sys
from collections import Counter
from pathlib import Path

import numpy as np

from apb_cursor_scan import Session
from apb_cursor_modules import module_base
from apb_manual_labels import instance_signature


def valid_pointer(value):
    return 0x10000 <= value < 0x800000000000 and value % 8 == 0


def read_span(session, address, size):
    return b"".join(session.read(address + offset, min(4096, size - offset))
                    for offset in range(0, size, 4096))


def save_json(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--array-entry-limit", type=int, default=131072)
    parser.add_argument("--position-offsets", type=lambda v: int(v, 0), nargs="+",
                        default=[0x88, 0xD0, 0x108])
    args = parser.parse_args()
    if not 1 <= args.array_entry_limit <= 500000:
        parser.error("Array limit must be between 1 and 500000")
    if any(offset < 0x40 or offset + 12 > 0x180 or offset % 4 for offset in args.position_offsets):
        parser.error("Position offsets must be aligned within the bounded 0x180 object prefix")
    args.output_directory.mkdir(parents=True, exist_ok=False)
    session = Session(args.pid)
    report = {"format": "apb-build-candidates-v1", "identity": session.identity,
              "status": "capture_in_progress", "sections": [], "array_roots": [],
              "objects": [], "camera_candidates": [], "candidates": [], "errors": [],
              "position_offsets": args.position_offsets, "array_entry_limit": args.array_entry_limit}
    try:
        base = module_base(session)
        report["module_base"] = hex(base)
        header = session.read(base, 4096)
        pe = struct.unpack_from("<I", header, 0x3C)[0]
        if pe + 24 > len(header) or header[pe:pe+4] != b"PE\0\0":
            raise RuntimeError("Invalid PE header")
        section_count = struct.unpack_from("<H", header, pe+6)[0]
        optional_size = struct.unpack_from("<H", header, pe+20)[0]
        image_size = struct.unpack_from("<I", header, pe+0x50)[0]
        if pe + 24 + optional_size + section_count*40 > len(header):
            raise RuntimeError("PE section table exceeds captured header")
        report["image_size"] = image_size
        sections = []
        for index in range(section_count):
            offset = pe + 24 + optional_size + index*40
            size, rva = struct.unpack_from("<II", header, offset+8)
            flags = struct.unpack_from("<I", header, offset+36)[0]
            if size and rva + size > image_size:
                raise RuntimeError("PE section extends outside image")
            sections.append((rva, size, flags))
        roots = []
        for rva, size, flags in sections:
            if not (flags & 0x80000000 and flags & 0x40) or flags & 0x20000000:
                continue
            print(f"Capturing writable module section {hex(rva)} + {hex(size)}", flush=True)
            data = read_span(session, base+rva, size)
            filename = f"module-{rva:08X}.bin"
            (args.output_directory / filename).write_bytes(data)
            report["sections"].append({"rva": hex(rva), "size": size, "path": filename,
                                       "sha256": hashlib.sha256(data).hexdigest()})
            values = np.frombuffer(data[:len(data)//8*8], dtype="<u8")
            pointers = (values >= 0x10000) & (values < 0x800000000000) & (values % 8 == 0)
            pointers &= (values < base) | (values >= base+image_size)
            for index in np.flatnonzero(pointers[:-1]):
                index = int(index)
                count, capacity = struct.unpack_from("<ii", data, index*8+8)
                if not 1 <= count <= capacity <= 1000000:
                    continue
                roots.append({"slot_rva": hex(rva+index*8), "address": hex(int(values[index])),
                              "count": count, "capacity": capacity})
        print(f"Inspecting {len(roots)} pointer/count/capacity candidates", flush=True)
        object_refs = {}
        for root_index, root in enumerate(roots):
            address = int(root["address"], 16)
            extent = min(root["count"], args.array_entry_limit)
            root["entries_requested"] = extent
            root["truncated"] = extent < root["count"]
            try:
                data = read_span(session, address, extent*8)
            except OSError as error:
                root["read_error"] = str(error)
                report["array_roots"].append(root)
                continue
            filename = f"array-{root_index:04d}.bin"
            (args.output_directory / filename).write_bytes(data)
            root["path"] = filename
            root["sha256"] = hashlib.sha256(data).hexdigest()
            for index, (pointer,) in enumerate(struct.iter_unpack("<Q", data)):
                if valid_pointer(pointer) and not base <= pointer < base+image_size:
                    object_refs.setdefault(pointer, []).append({"root_rva": root["slot_rva"], "index": index})
            root["header_unchanged"] = session.read(base+int(root["slot_rva"], 16), 16) == struct.pack(
                "<Qii", address, root["count"], root["capacity"])
            report["array_roots"].append(root)
        print(f"Inspecting {len(object_refs)} unique referenced objects", flush=True)
        counts = Counter()
        positions = []
        for index, (address, refs) in enumerate(object_refs.items()):
            if index % 1000 == 0:
                print(f"Objects {index}/{len(object_refs)} | typed {len(report['objects'])} | "
                      f"camera shapes {len(report['camera_candidates'])}", flush=True)
                if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
                    raise RuntimeError("APB exited during capture")
            try:
                body = session.read(address, 0x700)
            except OSError as error:
                report["errors"].append({"address": hex(address), "error": str(error)})
                continue
            vtable = struct.unpack_from("<Q", body)[0]
            if not any(base+rva <= vtable < base+rva+size and not flags & 0x80000000
                       for rva, size, flags in sections):
                continue
            marker = struct.unpack_from("<I", body, 0x3C)[0]
            signature = instance_signature(body)
            common = {"address": hex(address), "vtable_rva": hex(vtable-base),
                      "marker": marker, "instance_signature": signature}
            report["objects"].append({**common, "references": refs, "body_prefix_hex": body[:0x180].hex()})
            counts[(vtable-base, marker)] += 1
            matrix = struct.unpack_from("<16f", body, 0x680)
            camera_xyz = struct.unpack_from("<fff", body, 0x6C0)
            if (all(math.isfinite(v) for v in (*matrix, *camera_xyz))
                    and abs(matrix[14]+9.99) <= 0.02 and abs(matrix[15]) <= 0.02
                    and all(abs(v) < 1000000 for v in camera_xyz)
                    and all(abs(v) <= 10 for v in matrix[:14])
                    and max(abs(v) for v in matrix[:12]) >= 0.1):
                report["camera_candidates"].append({**common, "matrix_offset": 0x680,
                    "position_offset": 0x6C0, "matrix": matrix, "position": camera_xyz,
                    "references": refs, "body_hex": body.hex()})
            for offset in args.position_offsets:
                xyz = struct.unpack_from("<fff", body, offset)
                if (all(math.isfinite(v) and abs(v) < 1000000 for v in xyz)
                        and max(abs(v) for v in xyz) >= 1
                        and all(v == 0 or abs(v) >= 1e-6 for v in xyz)):
                    positions.append({**common, "position_offset": offset, "position": xyz})
        report["type_counts"] = [{"vtable_rva": hex(vtable), "marker": marker, "count": count}
                                 for (vtable, marker), count in counts.most_common()]
        for camera in report["camera_candidates"]:
            camera["fingerprint_unchanged"] = instance_signature(
                session.read(int(camera["address"], 16), 0x40)) == camera["instance_signature"]
        cameras = [camera for camera in report["camera_candidates"] if camera["fingerprint_unchanged"]]
        report["candidates"] = positions
        if len(cameras) != 1:
            raise RuntimeError(f"Expected one structural camera candidate; found {len(cameras)}. "
                               "Inspect camera_candidates in the saved report before selecting a camera.")
        report["camera"] = cameras[0]
        report["candidates"] = [p for p in positions if math.dist(p["position"], cameras[0]["position"]) <= 100000]
        report["status"] = "unverified_build_candidates"
        print(f"CAPTURE COMPLETE: camera {cameras[0]['address']}; "
              f"{len(report['candidates'])} provisional XYZ candidates", flush=True)
    except BaseException as error:
        report["status"] = "capture_failed"
        report["failure"] = str(error)
        raise
    finally:
        session.close()
        save_json(args.output_directory / "candidates.json", report)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, struct.error) as error:
        raise SystemExit(f"New-build candidate scan failed: {error}")
