"""Capture current-level actor/component annotation candidates from explicit discovery evidence."""

import argparse
import json
import math
import struct
from pathlib import Path

from apb_build_candidates import read_span, valid_pointer
from apb_cursor_scan import Session
from apb_cursor_modules import module_base
from apb_manual_labels import instance_signature
from apb_object_annotation_overlay import read_typed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--radius", type=float, default=5000)
    parser.add_argument("--name-array-root", type=lambda value: int(value, 0), required=True)
    args = parser.parse_args()
    source = json.loads(args.source.read_text(encoding="utf-8"))
    session = Session(args.pid)
    report = {"identity": session.identity, "status": "capture_in_progress", "arrays": [], "errors": []}
    try:
        if source.get("format") != "apb-build-candidates-v1" or source["identity"] != session.identity:
            raise RuntimeError("Source is not a candidate capture for this process/build")
        if not math.isfinite(args.radius) or args.radius <= 0:
            raise ValueError("Radius must be finite and positive")
        args.output_directory.mkdir(parents=True, exist_ok=False)
        base = module_base(session)
        camera = read_typed(session, base, source["camera"], camera=True)
        name_table, name_count, name_capacity = struct.unpack("<Qii", session.read(base+args.name_array_root, 16))
        if not valid_pointer(name_table) or not 0 < name_count <= name_capacity <= 1000000:
            raise RuntimeError("Invalid explicit name table")
        names = {}

        def name(index):
            if index not in names:
                if not 0 <= index < name_count:
                    raise RuntimeError(f"Object name index {index} outside table bounds")
                address = struct.unpack("<Q", session.read(name_table+index*8, 8))[0]
                body = session.read(address, 512)
                tag = struct.unpack_from("<I", body, 8)[0]
                if tag >> 1 != index:
                    raise RuntimeError(f"Name-entry index check failed for {index}")
                if tag & 1:
                    units = struct.unpack_from("<244H", body, 24)
                    length = units.index(0)
                    text = body[24:24+length*2].decode("utf-16-le")
                else:
                    text = body[24:].split(b"\0", 1)[0].decode("ascii")
                names[index] = text
            return names[index]

        world = struct.unpack("<Q", session.read(int(camera["address"], 16)+0x2E0, 8))[0]
        world_body = session.read(world, 0x400)
        level = struct.unpack_from("<Q", world_body, 0x44)[0]
        level_body = session.read(level, 4096)
        (args.output_directory / "level.bin").write_bytes(level_body)
        report.update(world=hex(world), level=hex(level), level_marker=struct.unpack_from("<I", level_body, 0x3C)[0])
        report["level_name"] = name(report["level_marker"])
        candidates = []
        for offset in range(0x40, len(level_body)-15, 4):
            pointer, count, capacity = struct.unpack_from("<Qii", level_body, offset)
            if not valid_pointer(pointer) or not 1 <= count <= capacity <= 100000:
                continue
            root = {"offset": hex(offset), "address": hex(pointer), "count": count, "capacity": capacity}
            try:
                data = read_span(session, pointer, count*8)
            except OSError as error:
                root["read_error"] = str(error)
                report["arrays"].append(root)
                continue
            values = [v[0] for v in struct.iter_unpack("<Q", data)]
            root["contains_world"] = world in values
            if root["contains_world"]:
                candidates.append((root, values))
                (args.output_directory / f"level-array-{offset:04X}.bin").write_bytes(data)
            report["arrays"].append(root)
        if len(candidates) != 1:
            raise RuntimeError(f"Expected one level actor-array containing WorldInfo; found {len(candidates)}")
        root, addresses = candidates[0]
        report["actor_array"] = root
        print(f"Current level {report['level_name']} + {root['offset']}: {len(addresses)} actor slots", flush=True)
        points, objects, seen = [], [], set()

        def capture(address, offsets, owner=None):
            if address in seen or not valid_pointer(address):
                return None
            seen.add(address)
            body = session.read(address, 0x300)
            vtable = struct.unpack_from("<Q", body)[0]
            if not base <= vtable < base + source["image_size"]:
                raise RuntimeError(f"Object has no APB vtable at {hex(address)}")
            marker = struct.unpack_from("<I", body, 0x3C)[0]
            spec = {"address": hex(address), "vtable_rva": hex(vtable-base), "marker": marker,
                    "instance_signature": instance_signature(body), "object_name": name(marker), "owner": owner}
            objects.append({**spec, "body_hex": body.hex()})
            for offset in offsets:
                xyz = struct.unpack_from("<3f", body, offset)
                if (all(math.isfinite(v) and abs(v) < 1000000 for v in xyz)
                        and max(abs(v) for v in xyz) >= 1
                        and all(v == 0 or abs(v) >= 1e-6 for v in xyz)
                        and math.dist(xyz, camera["position"]) <= args.radius):
                    points.append({**spec, "position_offset": offset, "position": xyz})
            return body

        level_vtable = struct.unpack_from("<Q", level_body)[0]-base
        level_roots = [{"level": hex(level), **root}]
        for spec in source["objects"]:
            level_address = int(spec["address"], 16)
            if level_address == level or int(spec["vtable_rva"], 16) != level_vtable:
                continue
            current = session.read(level_address, 0x180)
            if instance_signature(current) != spec["instance_signature"]:
                report["errors"].append({"level": spec["address"], "error": "Streamed level fingerprint expired"})
                continue
            pointer, count, capacity = struct.unpack_from("<Qii", current, int(root["offset"], 16))
            if count == 0:
                report["errors"].append({"level": spec["address"], "error": "Streamed level has no actor slots"})
                continue
            if not valid_pointer(pointer) or not 0 < count <= capacity <= 100000:
                raise RuntimeError(f"Streamed level has invalid actor-array bounds: {spec['address']}")
            data = read_span(session, pointer, count*8)
            (args.output_directory / f"streamed-{level_address:X}.actors.bin").write_bytes(data)
            level_roots.append({"level": hex(level_address), "offset": root["offset"],
                                "address": hex(pointer), "count": count, "capacity": capacity})
            addresses.extend(value[0] for value in struct.iter_unpack("<Q", data))
        print(f"Inspecting {len(addresses)} actor slots across {len(level_roots)} captured levels", flush=True)
        for index, address in enumerate(addresses):
            if not address:
                continue
            if index % 500 == 0:
                print(f"Actor slots {index}/{len(addresses)} | local XYZ candidates {len(points)}", flush=True)
            try:
                body = capture(address, (0x88,))
                if body is None:
                    continue
                pointer, count, capacity = struct.unpack_from("<Qii", body, 0x140)
                if not valid_pointer(pointer) or not 0 < count <= capacity <= 2048:
                    continue
                data = read_span(session, pointer, count*8)
                for (component,) in struct.iter_unpack("<Q", data):
                    if component:
                        capture(component, (0x88, 0xD0, 0x108), hex(address))
            except OSError as error:
                report["errors"].append({"address": hex(address), "error": str(error)})
        for level_root in level_roots:
            if session.read(int(level_root["level"], 16)+int(level_root["offset"], 16), 16) != struct.pack(
                    "<Qii", int(level_root["address"], 16), level_root["count"], level_root["capacity"]):
                raise RuntimeError(f"Level actor-array root changed during capture: {level_root['level']}; repeat at a steady scene")
        read_typed(session, base, source["camera"], camera=True)
        report.update(status="unverified_level_candidates", objects=objects, names=names, level_roots=level_roots,
                      candidate_count=len(points))
        profile = {"format": source["format"], "identity": session.identity, "module_base": hex(base),
                   "status": "unverified_build_candidates", "source": str(args.source.resolve()),
                   "selection": {"level": hex(level), "actor_array_offset": root["offset"],
                                 "radius_world_units": args.radius, "centre": camera["position"],
                                 "captured_levels": level_roots},
                   "camera": source["camera"], "candidates": points}
        with (args.output_directory / "profile.json").open("x", encoding="utf-8") as stream:
            json.dump(profile, stream, indent=2, allow_nan=False)
        print(f"CAPTURE COMPLETE: {len(points)} local actor/component XYZ candidates", flush=True)
    except BaseException as error:
        report.update(status="capture_failed", failure=str(error))
        raise
    finally:
        session.close()
        if args.output_directory.exists():
            with (args.output_directory / "level-capture.json").open("x", encoding="utf-8") as stream:
                json.dump(report, stream, indent=2, allow_nan=False)


if __name__ == "__main__":
    main()
