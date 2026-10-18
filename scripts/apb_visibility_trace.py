"""F8-confirmed visibility discovery through the selected EchoDriver reader.

Every phase waits indefinitely for the operator. F8 confirms a settled scene,
then a short measured burst is saved before the next movement cue is displayed.
"""

import argparse
import hashlib
import json
import math
import sys
import time
import traceback
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, write_json
from apb_visibility_candidates import review
from apb_visibility_f8 import capture
from apb_visibility_protocol import PHASES, OBSERVER_PHASES
from apb_visibility_scope import VisibilityScope
from apb_visibility_structure import decode_structure, review_structure, STRUCTURE_OFFSET, STRUCTURE_SIZE, PRIOR_SESSION
from apb_visibility_trace_overlay import TraceOverlay
from apb_camera_live import locate_camera
from apb_district_candidates import CAMERA_ROOT
from apb_character_ownership import resolve_local
from apb_visibility_roster import read_roster


class CaptureDiscarded(Exception):
    """Operator explicitly invalidated this capture; retain it and await F8."""


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--protocol", choices=("target-motion", "observer-los"), default="target-motion")
    parser.add_argument("--hz", type=float, default=15)
    parser.add_argument("--seconds", type=float, default=4)
    parser.add_argument("--trim-seconds", type=float, default=0.5)
    parser.add_argument("--camera-position-tolerance", type=float, default=1.0,
                        help="Camera displacement limit: session baseline for target-motion; burst baseline for settled observer-los captures")
    parser.add_argument("--camera-matrix-tolerance", type=float, default=0.001)
    parser.add_argument("--target-position-tolerance", type=float, default=2.0,
                        help="Target displacement limit: settled burst for target-motion; selection reference throughout observer-los")
    args = parser.parse_args()
    numbers = [args.hz, args.seconds, args.trim_seconds, args.camera_position_tolerance,
               args.camera_matrix_tolerance, args.target_position_tolerance]
    if not all(math.isfinite(value) for value in numbers):
        parser.error("Numeric options must be finite")
    if not 1 <= args.hz <= 30 or not 3 <= args.seconds <= 30:
        parser.error("Use 1..30 Hz and 3..30 seconds per confirmed capture")
    if not 0 <= args.trim_seconds < (args.seconds-1)/2:
        parser.error("Trim must leave at least one second of settled samples")
    if min(args.camera_position_tolerance, args.camera_matrix_tolerance, args.target_position_tolerance) < 0:
        parser.error("Movement tolerances cannot be negative")
    args.output_directory = args.output_directory.resolve()
    if args.output_directory.exists() or not args.output_directory.parent.is_dir():
        parser.error("Use a new output directory with an existing parent")
    return args


def capture_phase(session, ui, scope, phase, folder, args, origin):
    folder.mkdir()
    write_json(folder / "capture-start.json", {"phase": phase, "identity": session.identity,
               "started_utc": utc_now(), "operator_confirmation_s": time.monotonic()-origin})
    session.require_foreground()
    scope.verify()
    scope.begin_capture(phase, args)
    ui.draw(f"CAPTURING: {phase['name']} | {args.seconds:g}s\n"
            "Hold the requested scene. F9 discard | F11 exit.")
    before = client_capture(session, folder / "before.png")
    started = time.monotonic()
    deadline, previous = started, None
    rounds = missed = 0
    max_gap = 0.0
    first_position = None
    maximum_displacement = 0.0
    maximum_camera_displacement = maximum_camera_matrix_delta = 0.0
    raw_path, rows_path = folder / "raw.bin.partial", folder / "samples.jsonl.partial"
    try:
        with raw_path.open("xb") as raw, rows_path.open("x", encoding="utf-8") as rows:
            while time.monotonic()-started < args.seconds:
                edges = ui.keys()
                if edges is None:
                    raise RuntimeError("APB lost focus during capture; partial evidence retained")
                if 0x7A in edges:
                    raise KeyboardInterrupt("F11 stopped capture; partial evidence retained")
                if 0x78 in edges:
                    raise CaptureDiscarded("Operator pressed F9; scene/capture rejected")
                now = time.monotonic()
                if now < deadline:
                    time.sleep(min(deadline-now, 0.005))
                    continue
                if previous is not None:
                    max_gap = max(max_gap, now-previous)
                previous = now
                bodies, timing, context = scope.read_round(origin, before, args, phase["observation"])
                if args.protocol == "observer-los":
                    context["structure"] = decode_structure(bodies["pawn"])
                maximum_camera_displacement = max(maximum_camera_displacement, context["camera_delta"]["position_delta"])
                maximum_camera_matrix_delta = max(maximum_camera_matrix_delta, context["camera_delta"]["matrix_max_delta"])
                if first_position is None:
                    first_position = context["position"]
                displacement = math.dist(first_position, context["position"])
                maximum_displacement = max(maximum_displacement, displacement)
                if not phase["moving_target"] and displacement > args.target_position_tolerance:
                    raise CaptureDiscarded(f"Witness moved {displacement:.6g} world units during a settled capture")
                context["target_displacement_from_first_sample"] = displacement
                for entry in timing:
                    entry["raw_offset"] = raw.tell()
                    raw.write(bodies[entry["name"]])
                rows.write(json.dumps({"round": rounds, "elapsed_s": now-started,
                    "session_s": now-origin, "completed_s": time.monotonic()-origin,
                    "utc": utc_now(), "observation": phase["observation"],
                    "regions": timing, **context}, allow_nan=False)+"\n")
                raw.flush()
                rows.flush()
                rounds += 1
                elapsed = time.monotonic()-started
                ui.recording(scope.point, context["camera"], phase, rounds, args.seconds-elapsed,
                             rounds/max(elapsed, 0.001), missed, max_gap, context.get("structure"))
                deadline += 1/args.hz
                ended = time.monotonic()
                if deadline < ended:
                    skipped = int((ended-deadline)*args.hz)+1
                    missed += skipped
                    deadline += skipped/args.hz
        if phase['moving_target'] and maximum_displacement <= args.target_position_tolerance:
            raise CaptureDiscarded('Movement control did not observe target movement; walk through open view and retry')
        if phase.get('moving_observer') and (maximum_camera_displacement <= args.camera_position_tolerance
                                             and maximum_camera_matrix_delta <= args.camera_matrix_tolerance):
            raise CaptureDiscarded('Observer movement control saw no camera movement; walk in open view and retry')
        after = client_capture(session, folder / "after.png")
        scope.verify()
        scope.check_camera(camera_at(session, scope.camera_address), args.camera_position_tolerance,
                           args.camera_matrix_tolerance)
        if (before["client_width"], before["client_height"]) != (after["client_width"], after["client_height"]):
            raise RuntimeError("APB client dimensions changed during capture")
        if rounds < 3 or previous-started < 2*args.trim_seconds+0.5:
            raise RuntimeError("Capture has insufficient sampling coverage; inspect rate and driver latency")
        raw_path.rename(folder / "raw.bin")
        rows_path.rename(folder / "samples.jsonl")
        result = {"phase": phase, "identity": session.identity, "rounds": rounds,
            "completed_utc": utc_now(), "sampling_span_s": previous-started,
            "effective_start_hz": (rounds-1)/(previous-started), "missed_deadlines": missed,
            "max_gap_s": max_gap, "maximum_target_displacement": maximum_displacement, "before": before, "after": after,
            "maximum_camera_displacement": maximum_camera_displacement,
            "maximum_camera_matrix_delta": maximum_camera_matrix_delta,
            "raw_sha256": hashlib.sha256((folder / "raw.bin").read_bytes()).hexdigest(),
            "samples_sha256": hashlib.sha256((folder / "samples.jsonl").read_bytes()).hexdigest(),
            "status": "completed_operator_confirmed_capture"}
        write_json(folder / "capture.json", result)
        print(f"CAPTURE COMPLETE: {phase['name']}; {rounds} samples, "
              f"{result['effective_start_hz']:.2f} Hz, max gap {max_gap:.3f}s. Safe to change scene.", flush=True)
        return result
    except CaptureDiscarded as error:
        write_json(folder / "capture.json", {"phase": phase, "identity": session.identity,
            "status": "operator_or_motion_discarded", "reason": str(error), "rounds": rounds,
            "completed_utc": utc_now()})
        print(f"CAPTURE DISCARDED: {error}. Evidence retained in {folder}. Prepare this phase again; F8 when ready.", flush=True)
        return None


def run():
    args = arguments()
    args.output_directory.mkdir()
    session = ui = None
    origin = time.monotonic()
    phases = OBSERVER_PHASES if args.protocol == "observer-los" else PHASES
    status = {"format": "apb-visibility-discovery-v2", "started_utc": utc_now(),
        "requested_hz": args.hz, "capture_seconds": args.seconds, "trim_seconds": args.trim_seconds,
        "movement_tolerances": {"camera_position": args.camera_position_tolerance,
            "camera_matrix": args.camera_matrix_tolerance, "target_position": args.target_position_tolerance},
        "protocol": phases, "protocol_name": args.protocol,
        "scene_scope": "stationary_target_observer_los" if args.protocol == "observer-los" else "in_view_clear_vs_opaque_wall",
        "status": "initializing", "completed_phases": []}
    exit_code = 0
    try:
        session = Session(args.pid)
        status["identity"] = session.identity
        write_json(args.output_directory / "session-start.json", status)
        camera_events = args.output_directory / "camera-table-events.jsonl"
        camera_events.touch(exist_ok=False)

        def camera_diagnostic(event):
            with camera_events.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(event, allow_nan=False)+"\n")

        ui = TraceOverlay(session)
        base = module_base(session)
        status["module_base"] = hex(base)
        status["camera_binding_checks"] = {"event_log": str(camera_events),
            "policy": "Unique typed camera, stable root/selected slot/instance; other changed slots classified and logged"}
        current_camera = locate_camera(session, base, CAMERA_ROOT, 128, camera_diagnostic)
        print(f"CAMERA BINDING VERIFIED: table {hex(current_camera['table'])}, "
              f"slot {current_camera['index']}, camera {hex(current_camera['camera'])}. "
              "Non-camera slot changes are classified and logged.", flush=True)
        current_local = resolve_local(session, base, current_camera['camera'])
        current_roster = read_roster(session, base, int(current_local['world'], 16))
        status['pawn_selection_source'] = 'world-roster'
        print(f"WORLD PAWN LIST VERIFIED: {len(current_roster['actors'])} characters. "
              "Selection uses World +0x48C / pawn +0x350 with ownership checks.", flush=True)
        selection = args.output_directory / "selection"
        selection.mkdir()
        instruction = ("Keep laptop character stationary for the entire session. Move the PC between captures, then stop."
                       if args.protocol == "observer-los" else "Keep PC camera/player stationary. Operate the laptop.")
        print(instruction + " Take as long as needed between F8 captures.\n"
              "F8 confirms each scene; F9 discards an active capture; F11 exits. No inferred LOS labels.", flush=True)
        ui.wait("WITNESS SELECTION\nLaptop: stand exposed and still. F8 survey | F11 exit.")
        ui.draw("CAPTURING selection survey. Keep witness and PC still. Please wait.")
        source = capture(session, selection, 1, camera_diagnostic, pawn_source='world-roster')
        print("SELECTION SURVEY COMPLETE. F5 cycles pawn origins; F6 confirms your laptop character.", flush=True)
        point = ui.select(source)
        scope = VisibilityScope(session, base, source, point, selection, camera_diagnostic)
        if args.protocol == "observer-los":
            if scope.owner['pawn'] == scope.local['pawn']:
                raise RuntimeError('Selected witness is the PC observer. Restart and select the laptop character.')
            decode_structure(session.read(scope.pawn, STRUCTURE_OFFSET+STRUCTURE_SIZE))
            status.update(fixed_target_reference=list(scope.owner['position']),
                          focused_structure={"offset": hex(STRUCTURE_OFFSET), "size": STRUCTURE_SIZE,
                              "interpretation": "unverified_pointer_count_capacity", "prior_lead_session": PRIOR_SESSION,
                              "hypothesis": "Nonzero pointer correlates with operator-labeled clear", "pointer_dereference": False},
                          camera_policy="Reset baseline each burst; motion permitted between bursts and during observer control only")
        status.update({"selected_pawn": point, "regions": scope.regions,
            "scope_context": {"local": scope.local, "camera_address": scope.camera_address,
                "camera_signature": scope.camera_signature, "camera_owner": scope.camera_owner},
            "scope": "Target pawn, registered component prefixes, typed world prefix; clipped at known census neighbors",
            "limits": "Sequential reads. Probe sizes and mesh ownership unverified. F8 labels are human observations. "
                      "Before/after images cannot independently prove all intermediate scene states."})
        write_json(args.output_directory / "session-scope.json", status)
        print(f"Fixed scope: {len(scope.regions)} prefixes / {sum(r['size'] for r in scope.regions)} bytes per round. "
              "World prefix included for clock discovery; no clock field has been verified.", flush=True)
        sequence = 0
        discovery_end = max(i for i, p in enumerate(phases, 1) if p["partition"] == "discovery")
        control_end = max(i for i, p in enumerate(phases, 1) if p["partition"] == "control")
        for index, phase in enumerate(phases, 1):
            while True:
                cue = ({"clear": "PC: CLEAR VIEW", "blocked": "PC: BLOCKED VIEW"} if args.protocol == "observer-los"
                       else {"clear": "VISIBLE NOW", "blocked": "HIDE BEHIND WALL"})[phase["observation"]]
                message = f"{cue}: {index}/{len(phases)} {phase['name']}\n{phase['prompt']}\nF8 when ready | F11 exit"
                capture_instruction = ('Keep walking in open view' if phase['moving_target'] or phase.get('moving_observer')
                                       else 'Hold it steady')
                print(f"\n{cue} - {phase['name']}: {phase['prompt']}\n"
                      f"Press F8 when the scene is ready. {capture_instruction} "
                      f"until CAPTURE COMPLETE ({args.seconds:g}s sampling).", flush=True)
                ui.wait(message)
                sequence += 1
                folder = args.output_directory / f"capture-{sequence:04d}"
                result = capture_phase(session, ui, scope, phase, folder, args, origin)
                if result is not None:
                    status["completed_phases"].append({"name": phase["name"], "capture": folder.name})
                    if args.protocol == "observer-los":
                        structure_report = review_structure(args.output_directory, folder/"structure-review.json")
                        last = structure_report['phases'][-1]
                        print(f"STRUCTURE RAW: {last['raw']['nonzero_pointer_fraction']:.1%} nonzero, "
                              f"count {last['raw']['counts']}, cap {last['raw']['capacities']}; "
                              f"{len(last['exceptions'])} label/layout exceptions. Unverified meaning.", flush=True)
                    break
            # Keep fits restricted to discovery even during later comparisons.
            if index in (2, 4, discovery_end, control_end, len(phases)):
                ui.draw(f"CAPTURE COMPLETE: {phase['name']}\nReviewing saved bytes. Please wait.")
                print("Filtering saved candidate fields; no target process reads during analysis.", flush=True)
                candidates = review(args.output_directory)
                write_json(folder / "candidate-progress.json", {"counts": candidates["counts"],
                    "comparison_pass_counts": candidates.get("comparison_pass_counts", {}),
                    "status": candidates["status"]})
                print(f"CANDIDATE COUNTS: {candidates['counts']} (hypotheses, not visibility results).", flush=True)
                if index == discovery_end:
                    write_json(args.output_directory / "discovery-candidates.json", candidates)
        write_json(args.output_directory / "candidates.json", candidates)
        if args.protocol == "observer-los":
            review_structure(args.output_directory)
        status.update(status="completed_raw_evidence", candidate_counts=candidates["counts"],
                      comparison_pass_counts=candidates.get("comparison_pass_counts", {}))
        print(f"DISCOVERY COMPLETE: {args.output_directory}\n"
              f"Recheck survivors: {status['comparison_pass_counts']}. Inspect candidates.json and screenshots; "
              "no field is automatically accepted as LOS.", flush=True)
        if not sum(status['comparison_pass_counts'].values()):
            print('NO CANDIDATE PASSED controls/rechecks in this scope. Visibility remains unavailable; raw evidence retained.', flush=True)
        ui.message.set(f"Discovery complete.\n{args.output_directory}\nRaw evidence and candidate report saved.")
        ui.overlay.hide()
        ui.panel.deiconify()
    except KeyboardInterrupt as error:
        exit_code = 130
        status.update(status="operator_stopped", reason=str(error))
        print(f"OPERATOR STOPPED: {error}. Completed and partial evidence retained.", flush=True)
    except Exception as error:
        exit_code = 1
        status.update(status="failed", error=f"{type(error).__name__}: {error}")
        (args.output_directory / "failure.txt").write_text(traceback.format_exc(), encoding="utf-8")
        status['completed_utc'] = utc_now()
        write_json(args.output_directory / 'session.json', status)
        traceback.print_exc()
        if ui is not None:
            from tkinter import messagebox
            ui.message.set(f"DISCOVERY FAILED: {error}\nEvidence: {args.output_directory}")
            ui.panel.deiconify()
            messagebox.showerror("APB visibility discovery failed", f"{error}\n\nEvidence retained in {args.output_directory}", parent=ui.panel)
    finally:
        status["completed_utc"] = utc_now()
        if not (args.output_directory / 'session.json').exists():
            write_json(args.output_directory / "session.json", status)
        if session is not None:
            session.close()
        if ui is not None:
            ui.close()
    return exit_code


if __name__ == "__main__":
    sys.stderr = sys.stdout  # Preserve the full traceback in PowerShell's log.
    raise SystemExit(run())
