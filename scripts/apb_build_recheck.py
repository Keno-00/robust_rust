"""Recheck a bounded local annotation profile from an explicit new-build capture."""

import argparse
import json
import math
import re
import struct
from pathlib import Path

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
    parser.add_argument("--name-array-root", type=lambda value: int(value, 0))
    args = parser.parse_args()
    if not math.isfinite(args.radius) or args.radius <= 0:
        parser.error("Radius must be finite and positive")
    source = json.loads(args.source.read_text(encoding="utf-8"))
    if source.get("format") != "apb-build-candidates-v1" or source.get("status") != "unverified_build_candidates":
        parser.error("Source must be a completed candidate capture")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Source belongs to another process launch or build")
        base = module_base(session)
        if base != int(source["module_base"], 16):
            raise RuntimeError("Module base changed")
        args.output_directory.mkdir(parents=True, exist_ok=False)
        camera = read_typed(session, base, source["camera"], camera=True)
        selected = []
        excluded = []
        for spec in source["candidates"]:
            if math.dist(spec["position"], camera["position"]) > args.radius:
                continue
            try:
                point = read_typed(session, base, spec)
                if math.dist(point["xyz"], camera["position"]) <= args.radius:
                    selected.append({**spec, "position": point["xyz"]})
            except OSError as error:
                excluded.append({"address": spec["address"], "position_offset": spec["position_offset"],
                                 "reason": str(error)})
            except RuntimeError as error:
                if "fingerprint/type changed" not in str(error):
                    raise
                excluded.append({"address": spec["address"], "position_offset": spec["position_offset"],
                                 "reason": str(error)})
        profile = {"format": source["format"], "identity": session.identity, "module_base": hex(base),
                   "status": "unverified_build_candidates", "source": str(args.source.resolve()),
                   "selection": {"radius_world_units": args.radius, "centre": camera["position"],
                                 "status": "explicit_local_capture_bound_not_object_classifier"},
                   "camera": {**source["camera"], "position": camera["position"], "matrix": camera["matrix"]},
                   "candidates": selected, "excluded_during_recheck": excluded}
        evidence = {"identity": session.identity, "camera": camera, "linked_objects": [], "name_entry_probes": []}
        world = struct.unpack("<Q", session.read(int(camera["address"], 16)+0x2E0, 8))[0]
        body = session.read(world, 0x400)
        evidence["linked_objects"].append({"label": "camera_world_link", "address": hex(world),
            "vtable_rva": hex(struct.unpack_from("<Q", body)[0]-base),
            "marker": struct.unpack_from("<I", body, 0x3C)[0], "body_hex": body.hex()})
        controller = struct.unpack_from("<Q", body, 0x358)[0]
        controller_body = session.read(controller, 0x1000)
        evidence["linked_objects"].append({"label": "world_controller_link", "address": hex(controller),
            "vtable_rva": hex(struct.unpack_from("<Q", controller_body)[0]-base),
            "marker": struct.unpack_from("<I", controller_body, 0x3C)[0], "body_hex": controller_body.hex()})
        if args.name_array_root is not None:
            table, count, capacity = struct.unpack("<Qii", session.read(base+args.name_array_root, 16))
            if not 0 < count <= capacity <= 1000000:
                raise RuntimeError("Explicit name-array root does not have plausible count/capacity")
            markers = sorted({spec["marker"] for spec in selected} | {camera["marker"]})
            for marker in markers:
                if not 0 <= marker < count:
                    raise RuntimeError(f"Name index {marker} outside explicit table count {count}")
                pointer = struct.unpack("<Q", session.read(table+marker*8, 8))[0]
                raw = session.read(pointer, 128)
                evidence["name_entry_probes"].append({"index": marker, "address": hex(pointer),
                    "body_hex": raw.hex(), "ascii_excerpts": [s.decode("ascii") for s in re.findall(rb"[ -~]{4,}", raw)],
                    "status": "unverified_name_entry_layout"})
        for name, data in (("profile.json", profile), ("recheck.json", evidence)):
            with (args.output_directory / name).open("x", encoding="utf-8") as stream:
                json.dump(data, stream, indent=2, allow_nan=False)
        print(f"RECHECK COMPLETE: {len(selected)} local candidates; {len(excluded)} expired/unreadable candidates excluded")
        print(f"Profile: {args.output_directory / 'profile.json'}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    main()
