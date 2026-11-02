"""Bounded EchoDriver evidence capture for character identity and local ownership.

Preserves typed object links and possible FString fields without assigning
ownership or accepting an address as a persistent identity.
"""

import argparse
import json
import struct
from pathlib import Path

from apb_camera_live import locate_camera, valid_pointer
from apb_district_candidates import CAMERA_ROOT, read_position, survey
from apb_f8_common import Session, module_base, utc_now, write_json
from apb_manual_labels import instance_signature


def inspect_object(session, base, address, directory, objects, extent=0x1000):
    key = hex(address)
    if key in objects and objects[key]["capture_extent"] >= extent:
        return objects[key]
    body = b"".join(session.read(address+offset, min(4096, extent-offset))
                    for offset in range(0, extent, 4096))
    vtable = struct.unpack_from("<Q", body)[0]
    if not base <= vtable < base + 0x5380000:
        raise RuntimeError(f"Object {key} has no module vtable")
    (directory / f"object-{address:016X}.bin").write_bytes(body)
    record = {"address": key, "utc": utc_now(), "vtable_rva": hex(vtable-base),
              "capture_extent": extent, "extent_status": "bounded_probe_not_verified_object_size",
              "marker": struct.unpack_from("<I", body, 0x3C)[0],
              "instance_signature": instance_signature(body), "links": [],
              "possible_strings": [], "pointer_read_failures": []}
    objects[key] = record
    seen = {}
    # Captured APB pointer fields include four-byte-aligned +0x48C. The
    # pointer target remains eight-byte aligned; its field need not be.
    for offset in range(8, len(body) - 7, 4):
        pointer = struct.unpack_from("<Q", body, offset)[0]
        if not valid_pointer(pointer):
            continue
        if pointer not in seen:
            try:
                header = session.read(pointer, 0x40)
                target_vtable = struct.unpack_from("<Q", header)[0]
                seen[pointer] = ({"address": hex(pointer), "vtable_rva": hex(target_vtable-base),
                    "marker": struct.unpack_from("<I", header, 0x3C)[0],
                    "instance_signature": instance_signature(header)}
                    if base <= target_vtable < base + 0x5380000 else None)
            except (OSError, RuntimeError) as error:
                seen[pointer] = None
                record["pointer_read_failures"].append({"offset": hex(offset),
                    "address": hex(pointer), "error": str(error)})
        if seen[pointer]:
            record["links"].append({"offset": hex(offset), **seen[pointer]})
        if offset + 16 <= len(body):
            count, capacity = struct.unpack_from("<ii", body, offset+8)
            if 2 <= count <= 96 and count <= capacity <= 1024:
                try:
                    raw = session.read(pointer, count*2)
                    text = raw.decode("utf-16-le").rstrip("\0")
                    if raw.endswith(b"\0\0") and text and all(c.isprintable() for c in text):
                        record["possible_strings"].append({"offset": hex(offset),
                            "address": hex(pointer), "count": count, "capacity": capacity,
                            "text": text, "status": "unverified_FString_layout"})
                except (OSError, RuntimeError, UnicodeError) as error:
                    record["pointer_read_failures"].append({"offset": hex(offset),
                        "address": hex(pointer), "kind": "possible_string", "error": str(error)})
    record["fingerprint_unchanged_after_capture"] = (
        instance_signature(session.read(address, 0x40)) == record["instance_signature"])
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    args.output_directory.mkdir(parents=True, exist_ok=False)
    session = Session(args.pid)
    try:
        base = module_base(session)
        report = {"identity": session.identity, "module_base": hex(base),
                  "started_utc": utc_now(), "characters": [], "objects": {},
                  "errors": [], "status": "unclassified_identity_and_ownership_evidence"}
        candidates, _ = survey(session, base, args.output_directory, 0, include_pawns=True)
        objects = report["objects"]
        for lead in candidates.values():
            try:
                xyz, signature, relation = read_position(session, base, lead, True, True)
            except (OSError, RuntimeError):
                continue  # Expected rejection by the explicitly selected typed-link gate.
            entry = {"position": lead["address"], "xyz": xyz,
                     "position_signature": signature, "relation": relation}
            report["characters"].append(entry)
            try:
                character = inspect_object(session, base, int(relation["address"], 16), args.output_directory, objects, 0x3000)
                for link in character["links"]:
                    inspect_object(session, base, int(link["address"], 16), args.output_directory, objects)
            except (OSError, RuntimeError) as error:
                report["errors"].append({"position": lead["address"], "error": str(error)})
        camera = locate_camera(session, base, CAMERA_ROOT, 128)
        report["camera"] = camera
        camera_object = inspect_object(session, base, camera["camera"], args.output_directory, objects)
        for link in camera_object["links"]:
            try:
                inspect_object(session, base, int(link["address"], 16), args.output_directory, objects,
                               0x3000 if link["offset"] == "0x2e0" else 0x1000)
            except (OSError, RuntimeError) as error:
                report["errors"].append({"camera_link": link, "error": str(error)})
        # Capture exact array-looking fields separately. Their membership and
        # layout are hypotheses, and may be neighbouring allocations.
        report["component_array_probes"] = []
        for character in report["characters"]:
            address = int(character["relation"]["address"], 16)
            for field in (0x128, 0x140, 0x238, 0x660, 0x870):
                raw = session.read(address+field, 16)
                pointer, count, capacity = struct.unpack("<Qii", raw)
                item = {"character": hex(address), "field": hex(field), "address": hex(pointer),
                        "count": count, "capacity": capacity}
                try:
                    if not valid_pointer(pointer) or not 0 < count <= capacity <= 64:
                        raise RuntimeError("Array probe has invalid bounds")
                    array = session.read(pointer, count*8)
                    item["entries"] = [hex(value[0]) for value in struct.iter_unpack("<Q", array)]
                    item["position_is_member"] = int(character["position"], 16) in {
                        value[0] for value in struct.iter_unpack("<Q", array)}
                    item["unchanged_after_capture"] = session.read(address+field, 16) == raw
                except (OSError, RuntimeError) as error:
                    item["error"] = str(error)
                report["component_array_probes"].append(item)
        report["completed_utc"] = utc_now()
        write_json(args.output_directory / "identity_inspection.json", report)
        from apb_character_ownership import resolve_local, owned_position, filter_owned
        validation = {"identity": session.identity, "utc": utc_now(), "errors": [],
                      "status": "live_ownership_validation"}
        try:
            selected = locate_camera(session, base, CAMERA_ROOT, 128)
            local = resolve_local(session, base, selected["camera"])
            points = []
            for character in report["characters"]:
                address = int(character["position"], 16)
                try:
                    xyz, signature, relation = read_position(session, base, candidates[address], True, True)
                    point = {"address": character["position"], "position": xyz,
                             "instance_signature": signature, "structure_relation": relation}
                    point["ownership"] = owned_position(session, base, point)
                    points.append(point)
                except (OSError, RuntimeError) as error:
                    validation["errors"].append({"position": character["position"], "error": str(error)})
            if resolve_local(session, base, selected["camera"]) != local:
                raise RuntimeError("Local ownership changed during validation")
            visible, excluded, ambiguous = filter_owned(points, local)
            validation.update(local=local, visible=visible, excluded_local=excluded, ambiguous=ambiguous)
        except (OSError, RuntimeError) as error:
            validation["errors"].append({"local_ownership_error": str(error)})
        write_json(args.output_directory / "ownership_validation.json", validation)
        print(f"OWNERSHIP VALIDATION: visible {len(validation.get('visible', []))}, "
              f"self excluded {len(validation.get('excluded_local', []))}, "
              f"errors {len(validation['errors'])}", flush=True)
        if validation["errors"]:
            raise RuntimeError("Ownership validation failed; inspect ownership_validation.json")
        print(f"IDENTITY INSPECTION COMPLETE: {len(report['characters'])} characters, "
              f"{len(objects)} typed bodies, {len(report['errors'])} capture errors; "
              f"saved {args.output_directory}", flush=True)
        if not report["characters"]:
            raise RuntimeError("No structure matches; inspect the saved survey and retry in a loaded district")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, struct.error) as error:
        raise SystemExit(f"Identity inspection failed: {error}")
