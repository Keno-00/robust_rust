"""F8-paced local and two-account witness capture for APB position objects.

The desktop account first jumps in place for a local reference. During the
mission it stays still while the laptop account moves away, returns, and jumps
in place. This tool records evidence;
it does not name a candidate pawn or send mouse movement.
"""

import argparse
import csv
import math
import sys
import time
from pathlib import Path

from apb_f8_common import (
    CONTROLLER_CLASS, OWNER_CLASS, Session, candidate_positions, capture_object_prefixes, capture_table,
    client_capture, module_base, position_at, utc_now, wait_f8, write_json,
)


def capture_phase(session, base, directory, label):
    prefix = directory / label
    print(f"{label}: reading object table. Keep the observed accounts still until CAPTURE COMPLETE.", flush=True)
    started = utc_now()
    objects, table = capture_table(session, base, prefix)
    prefixes = capture_object_prefixes(session, prefix)
    positions, links, rejected, body_errors = candidate_positions(session, objects)
    position_bodies = []
    for address in positions:
        try:
            position_bodies.append({"address": address,
                                    "body_hex": session.read(int(address, 16), 0x800).hex()})
        except (OSError, RuntimeError) as error:
            position_bodies.append({"address": address, "read_error": str(error)})
    relation_bodies = []
    for kind in (CONTROLLER_CLASS, OWNER_CLASS):
        for address in sorted(set(objects[kind])):
            try:
                size = 0x800 if kind == CONTROLLER_CLASS else 0x400
                relation_bodies.append({"address": f"0x{address:X}", "marker": kind,
                                        "body_hex": session.read(address, size).hex()})
            except (OSError, RuntimeError) as error:
                relation_bodies.append({"address": f"0x{address:X}", "marker": kind,
                                        "read_error": str(error)})
    image = client_capture(session, Path(str(prefix) + ".png"))
    report = {
        "format": "apb-players-f8-v1", "label": label, "started_utc": started,
        "completed_utc": utc_now(), "identity": session.identity,
        "module_base": f"0x{base:X}", "table": table, "object_prefixes": prefixes,
        "positions_unclassified": positions, "links_unclassified": links,
        "position_bodies": position_bodies,
        "relation_bodies": relation_bodies, "rejected_positions": rejected,
        "body_read_errors": body_errors, "image": image,
    }
    write_json(Path(str(prefix) + ".json"), report)
    print(f"{label}: CAPTURE COMPLETE. {len(positions)} unclassified positions, "
          f"{table['failed_headers']} failed headers, {prefixes['failed_reads']} failed full-prefix reads.", flush=True)
    if not positions:
        raise RuntimeError(f"No readable candidate positions in {label}; raw capture was saved")
    return report


def movement_ranking(before, moved, returned):
    initial = before["positions_unclassified"]
    away = moved["positions_unclassified"]
    back = returned["positions_unclassified"]
    rows = []
    for address in sorted(initial.keys() & away.keys() & back.keys()):
        first, second, third = initial[address], away[address], back[address]
        rows.append({
            "address": address, "baseline": first, "moved": second, "returned": third,
            "move_xy": math.dist(first[:2], second[:2]),
            "return_xy": math.dist(first[:2], third[:2]),
            "move_z": second[2] - first[2],
        })
    rows.sort(key=lambda item: (-item["move_xy"], item["return_xy"], item["address"]))
    return rows


def trace_jump(session, directory, addresses, name, subject):
    if not addresses or len(addresses) > 4096:
        raise RuntimeError(f"Cannot trace {len(addresses)} position candidates; inspect saved captures")
    wait_f8(session, f"{subject} ready to jump in place; F8 starts {name} live trace")
    print(f"{name.upper()} LIVE TRACE STARTED. Jump twice with the {subject}. "
          "After landing, press F8 on the desktop to end. No time limit.", flush=True)
    path = directory / f"{name}_jump.tsv"
    temporary = directory / f"{name}_jump.tsv.partial"
    user32 = session.user32
    samples = 0
    max_sample_gap_s = 0.0
    previous_at = None
    read_errors = {}
    observations = {}
    last_status = time.monotonic()
    with temporary.open("x", newline="", encoding="utf-8") as stream:
        writer = csv.writer(stream, delimiter="\t")
        writer.writerow(("elapsed_s", "address", "x", "y", "z"))
        started = time.monotonic()
        while True:
            if user32.GetAsyncKeyState(0x77) & 0x8000:
                session.require_foreground()
                while user32.GetAsyncKeyState(0x77) & 0x8000:
                    time.sleep(0.05)
                break
            session.require_foreground()
            at = time.monotonic() - started
            if previous_at is not None:
                max_sample_gap_s = max(max_sample_gap_s, at - previous_at)
            previous_at = at
            for address in addresses:
                try:
                    xyz = position_at(session, int(address, 16))
                except (OSError, RuntimeError) as error:
                    read_errors[address] = str(error)
                    continue
                writer.writerow((f"{at:.4f}", address, *xyz))
                item = observations.get(address)
                if item is None:
                    observations[address] = {"minimum": list(xyz), "maximum": list(xyz),
                                             "first": xyz, "last": xyz, "samples": 1}
                else:
                    item["samples"] += 1
                    item["last"] = xyz
                    for axis in range(3):
                        item["minimum"][axis] = min(item["minimum"][axis], xyz[axis])
                        item["maximum"][axis] = max(item["maximum"][axis], xyz[axis])
            samples += 1
            if time.monotonic() - last_status >= 2:
                print(f"  Trace active: {samples} sample rounds, {len(read_errors)} addresses with read errors.", flush=True)
                last_status = time.monotonic()
            time.sleep(0.10)
    if samples < 5:
        raise RuntimeError(f"Jump trace had only {samples} rounds; partial trace retained at {temporary}")
    temporary.replace(path)
    summary = []
    for address, item in observations.items():
        minimum, maximum = item["minimum"], item["maximum"]
        summary.append({"address": address, "samples": item["samples"],
                        "x_range": maximum[0] - minimum[0],
                        "y_range": maximum[1] - minimum[1],
                        "z_range": maximum[2] - minimum[2],
                        "z_first": item["first"][2], "z_last": item["last"][2],
                        "z_peak": maximum[2]})
    summary.sort(key=lambda item: (-item["z_range"], item["x_range"] + item["y_range"]))
    jump_observed = any(item["z_range"] > 0 for item in summary)
    write_json(directory / f"{name}_jump.json", {
        "format": "apb-player-jump-v1", "subject": subject, "identity": session.identity,
        "status": "unclassified_jump_leads" if jump_observed else "jump_not_observed",
        "trace_path": str(path), "sample_rounds": samples,
        "max_sample_gap_s": max_sample_gap_s,
        "read_errors": read_errors, "candidate_summary": summary,
    })
    print(f"{name.upper()} LIVE TRACE COMPLETE. {samples} rounds saved; largest Z ranges:", flush=True)
    for item in summary[:12]:
        print(f"  {item['address']} Z range={item['z_range']:.1f} "
              f"XY range=({item['x_range']:.1f},{item['y_range']:.1f})", flush=True)
    if not jump_observed:
        print(f"{name.upper()}: no candidate changed Z; this trace does not identify "
              f"the {subject}'s position. Check jump timing and candidate coverage.", flush=True)
    if max_sample_gap_s > 0.30:
        raise RuntimeError(f"{name} jump trace had a {max_sample_gap_s:.3f}s sample gap; "
                           "inspect saved trace before using its Z ranking")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--witness-only", action="store_true",
                        help="Repeat witness captures without repeating the desktop local trace")
    args = parser.parse_args()
    if args.output_directory.exists() or not 0 < args.pid < 2**32:
        parser.error("Use a valid PID and a new output directory")
    session = Session(args.pid)
    try:
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {
            "format": "apb-players-session-v1", "created_utc": utc_now(),
            "identity": session.identity, "module_base": f"0x{base:X}",
            "role": "desktop observer; laptop witness; no candidate is preclassified",
            "capture_stages": ["witness"] if args.witness_only else ["local", "witness"],
        })
        if not args.witness_only:
            print("First identify the desktop's local-position lead while standing in the "
                  "district where the witness mission will run. Keep APB foreground.", flush=True)
            wait_f8(session, "Desktop account standing still; local baseline")
            local = capture_phase(session, base, args.output_directory, "local_baseline")
            trace_jump(session, args.output_directory,
                       sorted(local["positions_unclassified"]), "local", "desktop account")
            print("Local capture complete. Start the five-minute witness mission when both "
                  "accounts are ready. Keep the desktop in one place.", flush=True)
        else:
            print("WITNESS-ONLY CAPTURE: reconnect the laptop witness and prepare both accounts "
                  "in the same district and mission. Keep the desktop in one place. "
                  "A fresh address must be established by movement and jumps.", flush=True)
        wait_f8(session, "Both accounts still and witness visible; baseline")
        baseline = capture_phase(session, base, args.output_directory, "witness_baseline")
        wait_f8(session, "Laptop witness moved to a distinct nearby position and stopped")
        moved = capture_phase(session, base, args.output_directory, "witness_moved")
        wait_f8(session, "Laptop witness returned to its first spot and stopped")
        returned = capture_phase(session, base, args.output_directory, "witness_returned")
        ranked = movement_ranking(baseline, moved, returned)
        movement_observed = any(item["move_xy"] > 0 for item in ranked)
        return_observed = any(item["move_xy"] > 0 and item["return_xy"] < item["move_xy"]
                              for item in ranked)
        write_json(args.output_directory / "movement_leads.json", {
            "format": "apb-witness-movement-leads-v1", "identity": session.identity,
            "status": ("movement_not_observed" if not movement_observed else
                       "return_not_observed" if not return_observed else "unclassified_leads"),
            "shared_candidates": len(ranked), "candidates": ranked,
        })
        print("Movement leads (not verified player identities):", flush=True)
        if not ranked:
            print("No position address survived all three witness still captures; "
                  "the jump trace will still be saved, but no witness identity is established.", flush=True)
        elif not movement_observed:
            print("No shared candidate changed XY between baseline and moved captures. "
                  "The jump trace will still be saved; repeat the still captures after "
                  "checking the witness has moved before pressing F8.", flush=True)
        elif not return_observed:
            print("No moving candidate returned closer to its baseline. The jump trace "
                  "will still be saved; repeat using a visible landmark for the starting spot.", flush=True)
        for item in ranked[:12]:
            print(f"  {item['address']} moved XY={item['move_xy']:.1f}, "
                  f"returned XY={item['return_xy']:.1f}", flush=True)
        addresses = sorted(set(baseline["positions_unclassified"]) |
                           set(moved["positions_unclassified"]) |
                           set(returned["positions_unclassified"]))
        trace_jump(session, args.output_directory, addresses, "witness", "laptop account")
        if not ranked:
            raise RuntimeError("No stable witness position address across movement captures; inspect raw object prefixes")
        if not movement_observed:
            raise RuntimeError("Witness movement was not observed between baseline and moved captures; "
                               "inspect screenshots and repeat the still captures")
        if not return_observed:
            raise RuntimeError("Witness return was not observed; repeat the still captures "
                               "and return to the baseline landmark before pressing F8")
        print("Witness capture complete. Compare movement and jump leads, screenshots, "
              "then verify controller/pawn links before naming the witness pawn.", flush=True)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Witness capture interrupted; completed files remain for inspection.", file=sys.stderr)
        raise SystemExit(1)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(f"Witness capture failed: {error}")
