"""Four explicit F8 snapshots of vehicle possession via read-only EchoDriver."""

import argparse
import struct
from pathlib import Path

from apb_camera_live import locate_camera
from apb_character_ownership import WORLD_TYPE, CONTROLLER_TYPE, typed_header, pointer_field
from apb_district_candidates import CAMERA_ROOT, survey
from apb_f8_common import Session, module_base, client_capture, utc_now, wait_f8, write_json
from apb_manual_labels import instance_signature


PAWN = (0x2AAEFA0, 5778)
VEHICLE = (0x2B29FB8, 6064)


def capture_body(session, base, address, extent, directory):
    body = b"".join(session.read(address+offset, min(4096, extent-offset))
                    for offset in range(0, extent, 4096))
    path = directory / f"object-{address:016X}.bin"
    path.write_bytes(body)
    return {"address": hex(address), "vtable_rva": hex(struct.unpack_from('<Q', body)[0]-base),
            "marker": struct.unpack_from('<I', body, 0x3C)[0],
            "instance_signature": instance_signature(body), "extent": extent,
            "extent_status": "bounded_probe_not_verified_object_size",
            "fingerprint_unchanged": instance_signature(session.read(address, 0x40)) == instance_signature(body)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    args.output_directory.mkdir(parents=True, exist_ok=False)
    session = Session(args.pid)
    try:
        base = module_base(session)
        write_json(args.output_directory / "session.json", {"identity": session.identity,
            "module_base": hex(base), "status": "vehicle_relationship_evidence_not_runtime_filter"})
        prompts = (
            ("on_foot", "Stand ON FOOT beside the vehicle you will drive"),
            ("driving_stationary", "Enter that vehicle as DRIVER, stay seated and stationary"),
            ("driving_moved", "Drive a short distance, stop, and remain seated"),
            ("exited", "Exit the vehicle and stand still beside it"),
        )
        for sequence, (label, instruction) in enumerate(prompts):
            wait_f8(session, instruction)
            directory = args.output_directory / label
            directory.mkdir()
            _, survey_report = survey(session, base, directory, sequence)
            camera = locate_camera(session, base, CAMERA_ROOT, 128)
            world = pointer_field(session, camera["camera"], 0x2E0)
            typed_header(session, base, world, WORLD_TYPE)
            controller = pointer_field(session, world, 0x358)
            typed_header(session, base, controller, CONTROLLER_TYPE)
            report = {"utc": utc_now(), "stage": label, "identity": session.identity,
                "camera": camera, "world": hex(world), "controller": hex(controller),
                "possessed": hex(pointer_field(session, controller, 0x790)),
                "objects": [], "errors": [], "failed_survey_headers": survey_report["failed_headers"]}
            addresses = {controller: 0x3000}
            headers = (directory / f"survey-{sequence:04d}.headers.bin").read_bytes()
            for offset in range(0, len(headers), 72):
                address = struct.unpack_from('<Q', headers, offset)[0]
                header = headers[offset+8:offset+72]
                kind = (struct.unpack_from('<Q', header)[0]-base,
                        struct.unpack_from('<I', header, 0x3C)[0])
                if kind in (PAWN, VEHICLE):
                    addresses[address] = 0x3000
            possessed = int(report["possessed"], 16)
            if possessed:
                addresses[possessed] = 0x3000
            for address, extent in addresses.items():
                try:
                    report["objects"].append(capture_body(session, base, address, extent, directory))
                except (OSError, RuntimeError) as error:
                    report["errors"].append({"address": hex(address), "error": str(error)})
            report["controller_possession_unchanged"] = pointer_field(session, controller, 0x790) == possessed
            report["image"] = client_capture(session, directory / "frame.png")
            write_json(directory / "stage.json", report)
            print(f"STAGE COMPLETE: {label}; {len(report['objects'])} bodies, "
                  f"{len(report['errors'])} errors. Continue to the next instruction.", flush=True)
        write_json(args.output_directory / "summary.json", {"identity": session.identity,
            "stages_completed": 4, "status": "vehicle_relationship_capture_complete"})
        print(f"CAPTURE COMPLETE: {args.output_directory}", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, struct.error) as error:
        raise SystemExit(f"Vehicle capture failed: {error}")
