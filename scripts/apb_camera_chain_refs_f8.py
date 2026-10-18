"""One F8-paced EchoDriver reverse-reference scan for a validated camera-owner chain."""

import argparse
import json
import struct
from pathlib import Path

from apb_f8_common import Session, camera_at, module_base, scan_pointer_refs, utc_now, wait_f8, write_json


def parse_address(value):
    return int(value, 0)


def validate_link(session, base, link):
    address = int(link["object"], 16)
    offset = int(link["offset"], 16)
    expected_target = int(link["target"], 16)
    header = session.read(address, 0x40)
    (vtable,) = struct.unpack_from("<Q", header, 0)
    (kind,) = struct.unpack_from("<I", header, 0x3C)
    (target,) = struct.unpack("<Q", session.read(address + offset, 8))
    if (vtable != base + int(link["vtable_rva"], 16)
            or kind != link["class"] or target != expected_target):
        raise RuntimeError(f"Camera-owner chain changed at 0x{address:X}+0x{offset:X}: "
                           f"vtable 0x{vtable:X}, class {kind}, target 0x{target:X}")
    return {"object": link["object"], "offset": link["offset"],
            "target": link["target"], "vtable": f"0x{vtable:X}", "class": kind}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_references", type=Path)
    parser.add_argument("parent_address", type=parse_address)
    parser.add_argument("link_offset", type=parse_address)
    parser.add_argument("vtable_rva", type=parse_address)
    parser.add_argument("class_marker", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if (not 0 < args.pid < 2**32 or args.output_directory.exists()
            or args.parent_address < 0x10000 or args.parent_address % 8
            or not 0 <= args.link_offset <= 0x4000 or args.link_offset % 8
            or not 0 < args.vtable_rva < 0x80000000 or not 0 < args.class_marker < 65536):
        parser.error("Use a valid PID, aligned object/link, vtable RVA, class, and new output directory")
    source = json.loads(args.source_references.read_text(encoding="utf-8"))
    if source.get("format") == "apb-camera-owner-refs-f8-v1":
        child = source["owner_before"]["address"]
        camera_address = source["camera_before"]["address"]
        lower_links = [{"object": child, "offset": "0x8", "target": camera_address,
                        "vtable_rva": "0x2B95CF0", "class": 16998}]
    elif source.get("format") == "apb-camera-chain-refs-f8-v1":
        child = source["object_before"]["object"]
        camera_address = source["camera_address"]
        lower_links = source["links"]
    else:
        raise RuntimeError("Source is not an owner or chain reference report")
    expected_source = f"0x{args.parent_address + args.link_offset:X}"
    matching = [hit for hit in source["references"]["hits"]
                if hit["source"] == expected_source and hit["target"] == child]
    if len(matching) != 1:
        raise RuntimeError(f"Source report does not show 0x{args.parent_address:X}"
                           f"+0x{args.link_offset:X} -> {child}")
    new_link = {"object": f"0x{args.parent_address:X}",
                "offset": f"0x{args.link_offset:X}", "target": child,
                "vtable_rva": f"0x{args.vtable_rva:X}", "class": args.class_marker}
    links = [new_link, *lower_links]
    session = Session(args.pid)
    try:
        if source["identity"] != session.identity:
            raise RuntimeError("Source references belong to another APB process launch")
        base = module_base(session)
        if f"0x{base:X}" != source["module_base"]:
            raise RuntimeError("APB module base changed since source references")
        args.output_directory.mkdir(parents=True)
        print(f"Tracing parent 0x{args.parent_address:X}; validating {len(links)} "
              "links to camera. Keep APB foreground until CAPTURE COMPLETE.", flush=True)
        wait_f8(session, "camera parent reverse-reference scan")
        started = utc_now()
        before = [validate_link(session, base, link) for link in links]
        camera_at(session, int(camera_address, 16))
        refs = scan_pointer_refs(session, [new_link["object"]], base)
        after = [validate_link(session, base, link) for link in links]
        camera_at(session, int(camera_address, 16))
        report = {"format": "apb-camera-chain-refs-f8-v1", "started_utc": started,
                  "completed_utc": utc_now(), "identity": session.identity,
                  "module_base": f"0x{base:X}", "source_references": str(args.source_references),
                  "camera_address": camera_address, "object_before": before[0],
                  "object_after": after[0], "links": links,
                  "links_before": before, "links_after": after,
                  "references": refs, "status": "unclassified_chain_references"}
        write_json(args.output_directory / "chain_refs.json", report)
        static = [hit for hit in refs["hits"] if hit["source_module_rva"] is not None]
        print(f"CAPTURE COMPLETE: {len(refs['hits'])} parent references, "
              f"{len(static)} in APB.exe, {refs['pages_failed']} failed pages.", flush=True)
        for hit in static[:20]:
            print(f"  APB.exe+{hit['source_module_rva']} -> {hit['target']}", flush=True)
        print(f"Saved: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Camera parent reverse-reference scan interrupted")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Camera parent reverse-reference scan failed: {error}")
