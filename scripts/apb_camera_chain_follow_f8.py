"""One F8-paced EchoDriver follow of a saved camera-owner chain after a district change."""

import argparse
import json
import re
import struct
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, wait_f8, write_json


def follow_root(session, base, root, links):
    objects = []
    address = root
    for index, link in enumerate(links):
        header = session.read(address, 0x40)
        (vtable,) = struct.unpack_from("<Q", header, 0)
        (kind,) = struct.unpack_from("<I", header, 0x3C)
        expected_vtable = base + int(link["vtable_rva"], 16)
        if vtable != expected_vtable or kind != link["class"]:
            raise RuntimeError(f"Link {index} object 0x{address:X} has vtable "
                               f"0x{vtable:X}, class {kind}; expected "
                               f"0x{expected_vtable:X}, class {link['class']}")
        offset = int(link["offset"], 16)
        (target,) = struct.unpack("<Q", session.read(address + offset, 8))
        if target < 0x10000 or target % 8:
            raise RuntimeError(f"Link {index} at 0x{address:X}+0x{offset:X} "
                               f"has invalid target 0x{target:X}")
        objects.append({"address": f"0x{address:X}", "class": kind,
                        "vtable": f"0x{vtable:X}", "link_offset": f"0x{offset:X}",
                        "link_target": f"0x{target:X}"})
        address = target
    camera = camera_at(session, address)
    return {"root": f"0x{root:X}", "objects": objects, "camera": camera}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_references", type=Path)
    parser.add_argument("label")
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,47}", args.label)):
        parser.error("Use a valid PID, short lowercase label, and new output directory")
    source = json.loads(args.source_references.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-chain-refs-f8-v1":
        raise RuntimeError("Source is not a camera-chain reference report")
    links = source["links"]
    if not links or len(links) > 12:
        raise RuntimeError("Source chain has an invalid link count")
    slots = sorted({int(hit["source"], 16) for hit in source["references"]["hits"]})
    if not slots:
        raise RuntimeError("Source chain has no reference slots to follow")
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Saved pointer slots belong to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since source scan")
        args.output_directory.mkdir(parents=True)
        print(f"Following {len(slots)} saved top-object slots in {args.label}; "
              f"{len(links)} typed links to camera. Keep APB foreground until "
              "CAPTURE COMPLETE.", flush=True)
        wait_f8(session, f"{args.label} camera-owner chain follow")
        started = utc_now()
        records = []
        candidates = {}
        for slot in slots:
            item = {"slot": f"0x{slot:X}"}
            try:
                (root,) = struct.unpack("<Q", session.read(slot, 8))
                item["target"] = f"0x{root:X}"
                if root not in candidates:
                    candidate = follow_root(session, base, root, links)
                    candidates[root] = candidate
                item["camera"] = candidates[root]["camera"]["address"]
            except (OSError, RuntimeError, ValueError, struct.error) as error:
                item["error"] = str(error)
            records.append(item)
        image = client_capture(session, args.output_directory / "view.png")
        after = {}
        for root in candidates:
            try:
                after[f"0x{root:X}"] = follow_root(session, base, root, links)
            except (OSError, RuntimeError, ValueError, struct.error) as error:
                after[f"0x{root:X}"] = {"error": str(error)}
        report = {"format": "apb-camera-chain-follow-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_references": str(args.source_references),
                  "label": args.label, "links": links, "slots": records,
                  "candidates": [candidates[key] for key in sorted(candidates)],
                  "candidates_after_image": after, "image": image,
                  "status": "unclassified_chain_follow"}
        write_json(args.output_directory / "follow.json", report)
        cameras = {item["camera"]["address"] for item in candidates.values()}
        print(f"CAPTURE COMPLETE: {len(slots)} slots, {len(candidates)} valid root objects, "
              f"{len(cameras)} distinct camera objects.", flush=True)
        for address in sorted(cameras):
            print(f"  Candidate camera: {address}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera-owner chain follow interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera-owner chain follow failed: {error}")
