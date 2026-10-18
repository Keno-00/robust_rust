"""Show the observed APB cursor-state field live through EchoDriver.

The raw value is always displayed so the person in game can check each label.
"""

import argparse
import json
import struct
import time
from datetime import datetime
from pathlib import Path

from apb_cursor_modules import module_base
from apb_cursor_scan import Session


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("leads", type=Path)
    parser.add_argument("--pointer-rva", type=lambda value: int(value, 0))
    parser.add_argument("--field-offset", type=lambda value: int(value, 0))
    parser.add_argument("--allow-new-session", action="store_true", help="Read pointer after restart; raw address from report is no longer trusted")
    args = parser.parse_args()
    if (args.pointer_rva is None) != (args.field_offset is None):
        parser.error("--pointer-rva and --field-offset must be supplied together")
    if args.allow_new_session and args.pointer_rva is None:
        parser.error("--allow-new-session requires a module pointer path")
    report = json.loads(args.leads.read_text(encoding="utf-8"))
    if report.get("format") == "apb-cursor-object-leads-v1":
        matches = [
            item for item in report["leads"]
            if item["relationship"] == "three_values"
            and (item["white"], item["green"], item["red"]) == (0, 1, 2)
        ]
    elif report.get("format") == "apb-cursor-recheck-v1":
        matches = [
            item for item in report["candidates"]
            if len({item["white"], item["green"], item["red"]}) == 3
            and max(item["white"], item["green"], item["red"]) <= 255
        ]
    else:
        raise ValueError("Unsupported cursor-lead report")
    if len(matches) != 1:
        raise ValueError(f"Expected one three-state cursor lead, found {len(matches)}")
    lead = matches[0]
    session = Session(args.pid)
    try:
        same_session = session.identity == report["identity"]
        if not same_session and not args.allow_new_session:
            raise ValueError("Cursor lead belongs to another APB process launch")
        base = module_base(session) if args.pointer_rva is not None else None
        if base is not None:
            print(f"APB PID {args.pid} | EchoDriver read-only | APB.exe 0x{base:X} + 0x{args.pointer_rva:X} -> +0x{args.field_offset:X}", flush=True)
            print(f"Original field {lead['address']} is {'available for cross-check' if same_session else 'from a prior launch and will not be read'}", flush=True)
        else:
            print(f"APB PID {args.pid} | EchoDriver read-only | address {lead['address']}", flush=True)
        labels = {lead["white"]: "WHITE", lead["green"]: "GREEN", lead["red"]: "RED"}
        print(f"Observed labels: {lead['white']}=WHITE, {lead['green']}=GREEN, {lead['red']}=RED, other=UNKNOWN", flush=True)
        print("Refocus APB and move the cursor freely. This terminal prints each change; press Ctrl+C here to stop.", flush=True)
        previous = None
        last_print = 0.0
        while True:
            try:
                session.require_foreground()
            except RuntimeError:
                current = ("PAUSED", None)
            else:
                try:
                    if base is not None:
                        pointer = struct.unpack("<Q", session.read(base + args.pointer_rva, 8))[0]
                        if not 0x10000 <= pointer < 0x800000000000:
                            current = ("INVALID POINTER", f"0x{pointer:X}")
                        else:
                            field = pointer + args.field_offset
                            (value,) = struct.unpack("<I", session.read(field, 4))
                            if same_session:
                                (direct,) = struct.unpack("<I", session.read(int(lead["address"], 16), 4))
                                if field != int(lead["address"], 16):
                                    current = ("PATH MISMATCH", f"ptr=0x{pointer:X} field=0x{field:X} path={value} direct={direct}")
                                elif value != direct:
                                    current = ("VALUE CHANGED BETWEEN READS", f"first={value} second={direct} ptr=0x{pointer:X}")
                                else:
                                    current = (labels.get(value, "UNKNOWN"), f"{value} ptr=0x{pointer:X}")
                            else:
                                current = (labels.get(value, "UNKNOWN"), f"{value} ptr=0x{pointer:X}")
                    elif "object" in lead:
                        (class_id,) = struct.unpack("<I", session.read(int(lead["object"], 16) + 0x3C, 4))
                        if class_id != lead["class_id"]:
                            current = ("OBJECT STALE", class_id)
                        else:
                            (value,) = struct.unpack("<I", session.read(int(lead["address"], 16), 4))
                            current = (labels.get(value, "UNKNOWN"), value)
                    else:
                        (value,) = struct.unpack("<I", session.read(int(lead["address"], 16), 4))
                        current = (labels.get(value, "UNKNOWN"), value)
                except (OSError, RuntimeError) as error:
                    current = ("READ ERROR", str(error))
            now = time.monotonic()
            if current != previous or now - last_print >= 2:
                print(f"{datetime.now().strftime('%H:%M:%S')}  {current[0]}  raw={current[1]}", flush=True)
                previous = current
                last_print = now
            time.sleep(0.2)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Live cursor read stopped.")
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"Live cursor read failed: {error}")
