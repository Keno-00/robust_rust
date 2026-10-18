"""Inspect an explicitly recorded owner and its two position references via EchoDriver."""

import argparse
import json
import struct
from pathlib import Path

from apb_f8_common import Session, module_base, utc_now, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("jump_report", type=Path)
    parser.add_argument("context_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    evidence = json.loads(args.jump_report.read_text(encoding="utf-8"))
    address = 0x15D15F400
    saved = (args.context_directory / "page-000000015D15F000.bin").read_bytes()[0x400:0xC00]
    expected_vtable = struct.unpack_from("<Q", saved)[0]
    expected_marker = struct.unpack_from("<I", saved, 0x3C)[0]
    if expected_vtable != 0x142B65A80 or expected_marker != 6959:
        raise RuntimeError("Saved owner context does not match the recorded hypothesis")
    args.output_directory.mkdir(parents=True, exist_ok=False)
    session = Session(args.pid)
    try:
        if evidence["identity"] != session.identity:
            raise RuntimeError("Owner hypothesis belongs to another APB launch")
        base = module_base(session)
        body = session.read(address, 4096)
        actual_vtable = struct.unpack_from("<Q", body)[0]
        actual_marker = struct.unpack_from("<I", body, 0x3C)[0]
        valid = actual_vtable == expected_vtable and actual_marker == expected_marker
        report = {"format": "apb-explicit-player-owner-inspection-v1", "utc": utc_now(),
                  "identity": session.identity, "module_base": f"0x{base:X}",
                  "address": f"0x{address:X}", "vtable": f"0x{actual_vtable:X}",
                  "marker": actual_marker, "header_matches": valid, "body_hex": body.hex(),
                  "fields": [], "module_references": [], "module_read_errors": []}
        write_json(args.output_directory / "owner_header.json", report)
        if not valid:
            raise RuntimeError("Recorded owner changed type; saved owner_header.json")
        report["typed_pointer_fields"] = []
        for offset in range(0x40, len(body), 8):
            pointer = struct.unpack_from("<Q", body, offset)[0]
            if not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
                continue
            try:
                target = session.read(pointer, 0x100)
            except (OSError, RuntimeError):
                continue
            vtable = struct.unpack_from("<Q", target)[0]
            if base + 0x2A00000 <= vtable < base + 0x3700000:
                report["typed_pointer_fields"].append({"offset": f"0x{offset:X}",
                    "pointer": f"0x{pointer:X}", "vtable_rva": f"0x{vtable-base:X}",
                    "marker": struct.unpack_from("<I", target, 0x3C)[0], "body_hex": target.hex()})
        for offset in (0x708, 0x768):
            pointer = struct.unpack_from("<Q", body, offset)[0]
            item = {"offset": f"0x{offset:X}", "pointer": f"0x{pointer:X}"}
            try:
                target = session.read(pointer, 0x100)
                item.update(vtable=f"0x{struct.unpack_from('<Q', target)[0]:X}",
                            marker=struct.unpack_from("<I", target, 0x3C)[0],
                            xyz_d0=struct.unpack_from("<fff", target, 0xD0), body_hex=target.hex())
            except (OSError, RuntimeError) as error:
                item["error"] = str(error)
            report["fields"].append(item)
        pe = struct.unpack("<I", session.read(base + 0x3C, 4))[0]
        if pe > 0x1000 or session.read(base + pe, 4) != b"PE\0\0":
            raise RuntimeError("Invalid module PE header")
        size = struct.unpack("<I", session.read(base + pe + 0x50, 4))[0]
        if not 0x1000 <= size <= 0x80000000:
            raise RuntimeError("Invalid module size")
        for start, end in session.writable_regions():
            for page in range(max(start, base), min(end, base + size), 4096):
                try:
                    raw = session.read(page, min(4096, end-page, base+size-page))
                except (OSError, RuntimeError) as error:
                    report["module_read_errors"].append({"page": f"0x{page:X}", "error": str(error)})
                    continue
                for index, (pointer,) in enumerate(struct.iter_unpack("<Q", raw[:len(raw)//8*8])):
                    if pointer == address:
                        report["module_references"].append(f"0x{page + index*8-base:X}")
        report["module_reference_windows"] = []
        for rva in report["module_references"]:
            location = base + int(rva, 16)
            report["module_reference_windows"].append({"rva": rva,
                "window_start_rva": f"0x{location-base-0x100:X}",
                "body_hex": session.read(location-0x100, 0x208).hex()})
        write_json(args.output_directory / "owner_inspection.json", report)
        # Explicit diagnostic window around the recorded inline pointer tables.
        # This is a type survey, not a promoted collection root or a runtime fallback.
        owners = []
        seen = set()
        failed_headers = 0
        for page in range(base + 0x3C00000, base + 0x3DB0000, 4096):
            raw = session.read(page, 4096)
            for index, (pointer,) in enumerate(struct.iter_unpack("<Q", raw)):
                if pointer in seen or not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
                    continue
                seen.add(pointer)
                try:
                    header = session.read(pointer, 0x40)
                except (OSError, RuntimeError):
                    failed_headers += 1
                    continue
                if (struct.unpack_from("<Q", header)[0] != expected_vtable
                        or struct.unpack_from("<I", header, 0x3C)[0] != expected_marker):
                    continue
                current = session.read(pointer, 0x800)
                item = {"address": f"0x{pointer:X}", "slot_rva": f"0x{page+index*8-base:X}",
                        "body_hex": current.hex(), "position_links": []}
                for offset in (0x708, 0x768):
                    target_address = struct.unpack_from("<Q", current, offset)[0]
                    link = {"offset": f"0x{offset:X}", "address": f"0x{target_address:X}"}
                    try:
                        target = session.read(target_address, 0x100)
                        link.update(vtable=f"0x{struct.unpack_from('<Q', target)[0]:X}",
                                    marker=struct.unpack_from("<I", target, 0x3C)[0],
                                    xyz_d0=struct.unpack_from("<fff", target, 0xD0), body_hex=target.hex())
                    except (OSError, RuntimeError) as error:
                        link["error"] = str(error)
                    item["position_links"].append(link)
                owners.append(item)
        write_json(args.output_directory / "owner_type_survey.json", {
            "identity": session.identity, "status": "unclassified_type_survey",
            "window_start_rva": "0x3C00000", "window_end_rva": "0x3DB0000",
            "unique_pointers": len(seen), "failed_headers": failed_headers, "owners": owners})
        after = session.read(address, 4096)
        report["owner_unchanged"] = after == body
        report["status"] = "unclassified_owner_context"
        write_json(args.output_directory / "owner_after_survey.json", report)
        print(json.dumps({key: report[key] for key in ("address", "header_matches", "fields", "module_references", "owner_unchanged")}), flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Owner inspection failed: {error}")
