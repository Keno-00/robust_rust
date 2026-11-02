"""F8-paced EchoDriver references and context for a movement/jump-confirmed position lead."""

import argparse
import json
import math
import struct
from pathlib import Path

from apb_f8_common import (
    Session, client_capture, module_base, scan_pointer_refs,
    utc_now, wait_f8, write_json,
)


def source_lead(directory, address, field_trace=False):
    if field_trace:
        jump = json.loads((directory / "jump.json").read_text(encoding="utf-8"))
        if jump.get("format") != "apb-position-leads-jump-v1":
            raise RuntimeError("Field-trace mode requires a completed explicit position-field jump trace")
        manifest = json.loads(Path(jump["source_manifest"]).read_text(encoding="utf-8"))
        if manifest.get("format") != "apb-controlled-position-leads-v1" or manifest["identity"] != jump["identity"]:
            raise RuntimeError("Field trace and movement manifest do not share one APB identity")
        target = f"0x{address:X}"
        matching = [item for item in manifest["candidates"] if item["address"] == target]
        if len(matching) != 1:
            raise RuntimeError("Explicit address is not unique in the field-trace manifest")
        lead = matching[0]
        jumped = [item for item in jump["candidates"] if item["address"] == target
                  and item["position_offset"] == lead["position_offset"]]
        if (len(jumped) != 1 or not lead["headers_stable"] or lead["move_xy"] <= 0
                or lead["return_xy"] >= lead["move_xy"] or jumped[0]["axis_ranges"][2] <= 0
                or jump["max_sample_gap_s"] > 0.3):
            raise RuntimeError("Field lead lacks controlled movement, return and adequately sampled jump evidence")
        return (jump["identity"], int(lead["vtable_rva"], 16), lead, jumped[0],
                int(lead["position_offset"], 16), lead["class_marker"])
    movement = json.loads((directory / "movement_leads.json").read_text(encoding="utf-8"))
    jump = json.loads((directory / "witness_jump.json").read_text(encoding="utf-8"))
    returned = json.loads((directory / "witness_returned.json").read_text(encoding="utf-8"))
    if (movement.get("format") != "apb-witness-movement-leads-v1"
            or jump.get("format") != "apb-player-jump-v1"
            or returned.get("format") != "apb-players-f8-v1"):
        raise RuntimeError("Source directory lacks completed witness movement and jump reports")
    if movement["identity"] != jump["identity"] or movement["identity"] != returned["identity"]:
        raise RuntimeError("Source reports do not share one APB process identity")
    target = f"0x{address:X}"
    moved = [item for item in movement["candidates"] if item["address"] == target]
    jumped = [item for item in jump["candidate_summary"] if item["address"] == target]
    bodies = [item for item in returned["position_bodies"] if item["address"] == target]
    if len(moved) != 1 or len(jumped) != 1 or len(bodies) != 1 or "body_hex" not in bodies[0]:
        raise RuntimeError("Explicit address lacks movement, jump, or saved body evidence")
    if moved[0]["move_xy"] <= 0 or moved[0]["return_xy"] >= moved[0]["move_xy"] or jumped[0]["z_range"] <= 0:
        raise RuntimeError("Explicit address did not move away, return closer, and change Z")
    body = bytes.fromhex(bodies[0]["body_hex"])
    base = int(returned["module_base"], 16)
    return (movement["identity"], struct.unpack_from("<Q", body)[0] - base, moved[0], jumped[0],
            0x88, struct.unpack_from("<I", body, 0x3C)[0])


def snapshot(session, base, address, vtable_rva, offset, marker):
    body = session.read(address, 0x800)
    if struct.unpack_from("<Q", body)[0] != base + vtable_rva:
        raise RuntimeError(f"Position lead 0x{address:X} changed vtable; recapture its movement and jumps")
    actual_marker = struct.unpack_from("<I", body, 0x3C)[0]
    if actual_marker != marker:
        raise RuntimeError(f"Position lead 0x{address:X} changed marker {marker} -> {actual_marker}")
    xyz = struct.unpack_from("<fff", body, offset)
    if not all(math.isfinite(value) and abs(value) < 1_000_000 for value in xyz):
        raise RuntimeError(f"Invalid position lead at 0x{address:X}+0x{offset:X}: {xyz}")
    return {"address": f"0x{address:X}", "vtable_rva": f"0x{vtable_rva:X}",
            "class_marker": actual_marker, "position_offset": f"0x{offset:X}",
            "position": xyz, "body_hex": body.hex()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("source_directory", type=Path)
    parser.add_argument("address", type=lambda value: int(value, 0))
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--field-trace", action="store_true",
                        help="Use an explicit movement-derived field trace instead of the class-4741 witness trace")
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    identity, vtable_rva, moved, jumped, offset, marker = source_lead(
        args.source_directory, args.address, args.field_trace)
    session = Session(args.pid)
    try:
        if identity != session.identity:
            raise RuntimeError("Witness evidence belongs to another APB launch; recapture the witness")
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        print(f"Read-only references to position lead 0x{args.address:X}+0x{offset:X}. "
              "Keep desktop APB foreground until CAPTURE COMPLETE. Character movement is allowed; "
              "the laptop can stay active. This does not classify a pawn.", flush=True)
        wait_f8(session, "witness-position reference scan; F8 starts")
        started = utc_now()
        before = snapshot(session, base, args.address, vtable_rva, offset, marker)
        write_json(args.output_directory / "position_before.json", before)
        refs = scan_pointer_refs(session, [f"0x{args.address:X}"], base)
        write_json(args.output_directory / "reference_scan.json", refs)
        # Save surrounding pages so referring object headers can be investigated offline.
        slots = []
        pages = set()
        for hit in refs["hits"]:
            slot = int(hit["source"], 16)
            record = {**hit}
            try:
                pointer = struct.unpack("<Q", session.read(slot, 8))[0]
                record.update({"target_after_scan": f"0x{pointer:X}",
                               "still_references_lead": pointer == args.address})
            except (OSError, RuntimeError) as error:
                record["read_error"] = str(error)
            slots.append(record)
            page = slot & ~0xFFF
            pages.update(page + delta for delta in (-0x2000, -0x1000, 0, 0x1000)
                         if page + delta >= 0x10000)
        write_json(args.output_directory / "slots_after_scan.json", slots)
        context = []
        for page in sorted(pages):
            item = {"address": f"0x{page:X}"}
            try:
                raw = session.read(page, 4096)
                name = f"page-{page:016X}.bin"
                (args.output_directory / name).write_bytes(raw)
                item["file"] = name
            except (OSError, RuntimeError) as error:
                item["read_error"] = str(error)
            context.append(item)
        write_json(args.output_directory / "context_pages.json", context)
        errors = []
        image = None
        after = None
        try:
            image = client_capture(session, args.output_directory / "view.png")
        except (OSError, RuntimeError) as error:
            errors.append({"phase": "image", "error": str(error)})
        try:
            after = snapshot(session, base, args.address, vtable_rva, offset, marker)
        except (OSError, RuntimeError) as error:
            errors.append({"phase": "position_after", "error": str(error)})
        write_json(args.output_directory / "references.json", {
            "format": "apb-player-position-refs-f8-v1", "started_utc": started,
            "completed_utc": utc_now(), "identity": session.identity,
            "module_base": f"0x{base:X}", "source_directory": str(args.source_directory),
            "movement_evidence": moved, "jump_evidence": jumped,
            "position_before": before, "position_after": after,
            "references": refs, "slots_after_scan": slots, "context_pages": context,
            "image": image, "errors": errors,
            "status": "validation_failed" if errors else "unclassified_position_reference_context",
        })
        if errors:
            raise RuntimeError(f"Reference results retained in {args.output_directory}; "
                               + "; ".join(f"{item['phase']}: {item['error']}" for item in errors))
        active = sum(item.get("still_references_lead", False) for item in slots)
        static = [hit for hit in refs["hits"] if hit["source_module_rva"] is not None]
        print(f"CAPTURE COMPLETE: {len(refs['hits'])} exact references, {active} still present, "
              f"{len(static)} in APB.exe, {refs['pages_failed']} failed scan pages. "
              f"Saved: {args.output_directory}", flush=True)
        if not active:
            raise RuntimeError("No reference survived the scan; inspect saved coverage and context")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        raise SystemExit("Player-position reference scan interrupted; completed files are retained")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Player-position reference scan failed: {error}")
