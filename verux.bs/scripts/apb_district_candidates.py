"""Read-only pawn projection recorder with explicit experimental mission classification."""

import argparse
import ctypes
import json
import math
import struct
import time
import copy
from collections import Counter
from pathlib import Path

from apb_camera_live import camera_identity, camera_shape, locate_camera
from apb_cursor_scan import ForegroundLost
from apb_capture_controls import down as key_held, wait_start, F10, F11
from apb_f8_common import Rect, Session, camera_at, client_capture, module_base, project, utc_now, wait_f8, write_json

# Explicit diagnostic window surveyed previously. Its collection boundaries and
# player membership remain unverified; this is not a product resolver.
WINDOW_START = 0x3C00000
WINDOW_END = 0x3DB0000
CAMERA_ROOT = 0x3DBF110
POSITION_TYPES = {(0x3142FB0, 4741): 0x88, (0x2AE68D8, 294): 0xD0}
# Exact linked type observed in the controlled alive/dead captures; actor
# identity and ownership semantics remain hypotheses, never position sources.
LINKED_TYPES = {(0x2AFF240, 1062), (0x2AAEFA0, 5778)}


def survey(session, base, directory, sequence, include_pawns=False):
    started = utc_now()
    raw = b"".join(session.read(base + page, 4096)
                   for page in range(WINDOW_START, WINDOW_END, 4096))
    (directory / f"survey-{sequence:04d}.table.bin").write_bytes(raw)
    slots = {}
    for index, (pointer,) in enumerate(struct.iter_unpack("<Q", raw)):
        if 0x10000 <= pointer < 0x800000000000 and pointer % 8 == 0:
            slots.setdefault(pointer, []).append(WINDOW_START + index * 8)
    candidates = {}
    types = Counter()
    errors = 0
    body_errors = []
    candidate_bodies = (directory / f"survey-{sequence:04d}.candidate-bodies.bin").open("xb")
    try:
        return survey_headers(session, base, directory, sequence, started, slots,
                              candidates, types, errors, body_errors, candidate_bodies, include_pawns)
    finally:
        candidate_bodies.close()


def survey_headers(session, base, directory, sequence, started, slots,
                   candidates, types, errors, body_errors, candidate_bodies, include_pawns=False):
    position_types = dict(POSITION_TYPES)
    if include_pawns:
        position_types[(0x2AAEFA0, 5778)] = 0x88
    with ((directory / f"survey-{sequence:04d}.headers.bin").open("xb") as saved,
          (directory / f"survey-{sequence:04d}.linked-bodies.bin").open("xb") as linked):
        for address, references in slots.items():
            try:
                header = session.read(address, 0x40)
            except (OSError, RuntimeError):
                errors += 1
                continue
            saved.write(struct.pack("<Q", address) + header)
            key = (struct.unpack_from("<Q", header)[0] - base,
                   struct.unpack_from("<I", header, 0x3C)[0])
            types[key] += 1
            if key in LINKED_TYPES:
                try:
                    body = session.read(address, 0x800)
                    if body[:0x40] != header:
                        raise RuntimeError("Linked object header changed during body capture")
                    linked.write(struct.pack("<Q", address) + body)
                except (OSError, RuntimeError) as error:
                    body_errors.append({"address": f"0x{address:X}", "error": str(error)})
            if key in position_types:
                try:
                    body = session.read(address, 0x800)
                except (OSError, RuntimeError) as error:
                    body_errors.append({"address": f"0x{address:X}", "error": str(error)})
                    continue
                if body[:0x40] != header:
                    body_errors.append({"address": f"0x{address:X}", "error": "Header changed during body capture"})
                    continue
                candidate_bodies.write(struct.pack("<Q", address) + body)
                candidates[address] = {"address": f"0x{address:X}", "vtable_rva": f"0x{key[0]:X}",
                    "marker": key[1], "position_offset": position_types[key],
                    "slot_rvas": [f"0x{rva:X}" for rva in references]}
    report = {"started_utc": started, "completed_utc": utc_now(), "sequence": sequence,
        "unique_pointers": len(slots), "failed_headers": errors,
        "candidate_body_errors": body_errors,
        "candidate_body_format": "records: little-endian u64 address followed by 0x800 object bytes",
        "type_counts": [{"vtable_rva": f"0x{key[0]:X}", "marker": key[1], "count": count}
                        for key, count in types.most_common()], "candidates": list(candidates.values()),
        "status": "unclassified_position_candidates"}
    write_json(directory / f"survey-{sequence:04d}.json", report)
    return candidates, report


def read_position(session, base, lead, structure_filter=False, direct_pawn=False):
    address = int(lead["address"], 16)
    body = session.read(address, 0x180 if structure_filter else 0x100)
    if (struct.unpack_from("<Q", body)[0] != base + int(lead["vtable_rva"], 16)
            or struct.unpack_from("<I", body, 0x3C)[0] != lead["marker"]):
        raise RuntimeError("Candidate type changed; old position discarded")
    xyz = struct.unpack_from("<fff", body, lead["position_offset"])
    if not all(math.isfinite(value) and abs(value) < 1_000_000 for value in xyz):
        raise RuntimeError("Candidate XYZ is invalid; position discarded")
    from apb_manual_labels import instance_signature
    relation = None
    if direct_pawn:
        if (int(lead["vtable_rva"], 16), lead["marker"]) != (0x2AAEFA0, 5778):
            raise RuntimeError("Ownership discovery requires the exact character pawn type")
        relation = {"address": hex(address), "vtable_rva": "0x2AAEFA0", "marker": 5778,
                    "instance_signature": instance_signature(body), "status": "direct_typed_character_pawn"}
    elif structure_filter:
        if lead["marker"] != 4741 or int(lead["vtable_rva"], 16) != 0x3142FB0:
            raise RuntimeError("Position type does not match the labelled structure hypothesis")
        linked = struct.unpack_from("<Q", body, 0x178)[0]
        if not 0x10000 <= linked < 0x800000000000 or linked % 8:
            raise RuntimeError("No plausible +0x178 typed link")
        header = session.read(linked, 0x40)
        if (struct.unpack_from("<Q", header)[0] != base + 0x2AAEFA0
                or struct.unpack_from("<I", header, 0x3C)[0] != 5778):
            raise RuntimeError("+0x178 target does not match vtable 0x2AAEFA0 / marker 5778")
        if session.read(address + 0x178, 8) != body[0x178:0x180]:
            raise RuntimeError("+0x178 link changed during sample")
        relation = {"offset": "0x178", "address": hex(linked), "vtable_rva": "0x2AAEFA0",
                    "marker": 5778, "instance_signature": instance_signature(header),
                    "flags_raw_10": hex(struct.unpack_from("<Q", header, 0x10)[0]),
                    "status": "unverified_ownership_structure"}
    return xyz, instance_signature(body), relation


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--inspect-only", action="store_true",
                        help="Immediately preserve one table/header/full-body survey; no F8 or screenshot")
    parser.add_argument("--lifecycle", action="store_true",
                        help="F8 marks alive, after movement, dead, and respawned capture stages")
    parser.add_argument("--overlay", action="store_true",
                        help="Draw click-through unclassified points; F9 saves a comparison frame")
    parser.add_argument("--manual-filter", action="store_true",
                        help="Overlay with explicit F6 exclusion/F4 player labels and F7 undo")
    parser.add_argument("--structure-filter", action="store_true",
                        help="Explicit experimental +0x178 typed-link filter derived from manual labels")
    parser.add_argument("--ownership-filter", action="store_true",
                        help="Require current pawn component membership and exclude the resolved local pawn")
    parser.add_argument("--mission-labels", action="store_true",
                        help="F1 outside mission, F2 teammate, F3 opposition; preserve local/labelled object contexts")
    parser.add_argument("--mission-classify", action="store_true",
                        help="Explicit experimental pawn +0x990 token/+0x994 side classifier; F4 opposition-only toggle")
    args = parser.parse_args()
    if not 1 <= args.seconds <= 1800 or args.output_directory.exists():
        parser.error("Use a new output directory and a duration of 1..1800 seconds")
    if args.overlay and (args.inspect_only or args.lifecycle):
        parser.error("Overlay is a separate visual recording mode")
    if args.manual_filter and not args.overlay:
        parser.error("Manual filtering requires --overlay")
    if args.structure_filter and not args.overlay:
        parser.error("Experimental structure filtering requires --overlay")
    if args.ownership_filter and not args.structure_filter:
        parser.error("Ownership filtering requires --structure-filter --overlay")
    if args.mission_labels and not (args.manual_filter and args.ownership_filter):
        parser.error("Mission labels require manual and ownership filtering")
    if args.mission_classify and not args.mission_labels:
        parser.error("Experimental classification requires --mission-labels for live validation")
    return args


def record(args):
    session = Session(args.pid)
    overlay = None
    manual = None
    try:
        base = module_base(session)
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {"identity": session.identity,
            "module_base": f"0x{base:X}", "window_start_rva": f"0x{WINDOW_START:X}",
            "window_end_rva": f"0x{WINDOW_END:X}", "camera_root_rva": f"0x{CAMERA_ROOT:X}",
            "position_types": [{"vtable_rva": f"0x{vt:X}", "marker": marker,
                               "offset": f"0x{offset:X}"} for (vt, marker), offset in POSITION_TYPES.items()],
            "mission_filter": args.mission_classify, "overlay": args.overlay,
            "manual_filter": args.manual_filter,
            "mission_labels": args.mission_labels,
            "experimental_structure_filter": args.structure_filter,
            "experimental_ownership_filter": args.ownership_filter,
            "ownership_discovery": "direct_vtable_0x2AAEFA0_marker_5778" if args.ownership_filter else None,
            "status": "diagnostic_candidates_not_verified_players"})
        if args.inspect_only:
            current, report = survey(session, base, args.output_directory, 0)
            print(f"INSPECTION COMPLETE: {len(current)} candidate bodies; "
                  f"{report['failed_headers']} failed headers; {len(report['candidate_body_errors'])} body failures. "
                  f"Saved {args.output_directory}", flush=True)
            return
        if args.overlay:
            if not wait_start(session, "Focus APB and face nearby characters"):
                return "exit"
        else:
            wait_f8(session, "both characters alive and stationary; desktop stays still" if args.lifecycle
                    else "district-wide candidate recording; move/jump/respawn the laptop after RECORDING")
        first = locate_camera(session, base, CAMERA_ROOT, 128)
        time.sleep(0.5)
        second = locate_camera(session, base, CAMERA_ROOT, 128)
        if camera_identity(first) != camera_identity(second):
            raise RuntimeError("Camera changed during initial stability check; restart capture")
        image = client_capture(session, args.output_directory / "start.png")
        width, height = image["client_width"], image["client_height"]
        if args.overlay:
            from apb_candidate_overlay import CandidateOverlay
            overlay = CandidateOverlay(session)
            print("OVERLAY: F9 saves a labeled frame; F8 stops/saves; F10 pauses/resumes; F11 fully exits. "
                  "Use windowed/borderless APB if Windows cannot display the overlay over exclusive fullscreen.", flush=True)
        if args.manual_filter:
            from apb_manual_labels import ManualLabels, annotation_evidence, instance_key
            manual = ManualLabels()
            print("MANUAL LABELS: aim red centre reticle near a point; highlighted ring is the exact selection. "
                  "F6 excludes ONE selected instance. F4 labels it as player; F5 cycles overlaps; F7 undoes. "
                  "Labels are session evidence, not automatic player identification.", flush=True)
        if args.structure_filter:
            print("EXPERIMENTAL STRUCTURE FILTER ACTIVE: 4741/+0x88 -> +0x178 -> "
                  "vtable 0x2AAEFA0 / marker 5778. Identity, NPC exclusion and duplicate handling "
                  "remain unverified. No alternate position source is used.", flush=True)
            overlay.structure_filter = True
        if args.ownership_filter:
            from apb_character_ownership import resolve_local, owned_position, filter_owned
            print("OWNERSHIP FILTER ACTIVE: direct typed character pawns; camera/world/controller local chain; "
                  "on-foot component membership or verified vehicle attachment; canonical pawn XYZ +0x88. "
                  "Ambiguous pawn groups are withheld. Persistent account identity is unverified.", flush=True)
            overlay.ownership_status = "OWNERSHIP: waiting for a validated local chain"
        if args.mission_labels:
            overlay.mission_labels = True
            print("MISSION GROUND TRUTH: F1 outside mission, F2 teammate, F3 opposition. "
                  "YOUR STATE: Ctrl+F2 in mission; Ctrl+F3 outside mission. "
                  "These are your ground-truth labels.", flush=True)
        if args.mission_classify:
            from apb_mission_classification import read_mission, classify
            overlay.mission_classify = True
            overlay.opposition_only = False
            print("EXPERIMENTAL MISSION CLASSIFIER ACTIVE: full pawn +0x990 token and +0x994 side. "
                  "Unknown states are displayed and logged; no faction-based substitution. "
                  "F4 toggles opposition-only display. All points are preserved in the recording.", flush=True)
        user32 = session.user32
        user32.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
        user32.GetClientRect.restype = ctypes.c_bool
        current = {}
        observations = {}
        sequence = samples = 0
        next_survey = 0
        started = time.monotonic()
        previous = None
        max_gap = 0
        stage = 0
        stages = ("alive_baseline", "alive_after_motion", "dead", "respawned_after_motion")
        prompts = ("MOVE AND JUMP only the laptop, then stop it and press F8 on desktop.",
                   "KILL the laptop character. Leave it dead; press F8 on desktop BEFORE respawning.",
                   "RESPAWN the laptop, move and jump again, then stop it and press F8 on desktop.")
        marker_pending = args.lifecycle
        frame_sequence = 0
        f9_was_down = False
        paused = False
        manual_paused = False
        f10_was_down = False
        stop_reason = "time_limit"
        annotation_sequence = 0
        mission_state_sequence = 0
        mission_context = {"operator_state": "unknown", "context_epoch": 0,
                           "status": "operator_annotation_not_memory_mission_id"}
        state_keys_down = {}
        mission_state_pending = False
        keys_down = {}
        print("RECORDING: unclassified candidates, no mission filter. "
              + ("Follow the stage prompts. " if args.lifecycle else "F8 stops. ")
              + f"Time limit {args.seconds}s.", flush=True)
        with (args.output_directory / "samples.jsonl").open("x", encoding="utf-8", buffering=1) as stream:
            def log(record):
                stream.write(json.dumps({"utc": utc_now(), **record}) + "\n")
            while time.monotonic() - started < args.seconds:
                try:
                    if args.overlay and key_held(session, F11):
                        stop_reason = "full_exit"
                        log({"state": "full_exit_requested"})
                        break
                    session.require_foreground()
                    if args.mission_labels:
                        control_down = bool(user32.GetAsyncKeyState(0x11) & 0x8000)
                        for state_name, vk in (("in_mission", 0x71), ("outside_mission", 0x72)):
                            state_down = control_down and bool(user32.GetAsyncKeyState(vk) & 0x8000)
                            if state_down and not state_keys_down.get(state_name, False):
                                mission_context = {"operator_state": state_name,
                                    "context_epoch": mission_context["context_epoch"]+1,
                                    "annotated_utc": utc_now(),
                                    "status": "operator_annotation_not_memory_mission_id"}
                                mission_state_pending = True
                                expired = {key: value for key, value in manual.labels.items()
                                           if value in ("teammate", "opposition", "outside_mission")}
                                for key in expired:
                                    del manual.labels[key]
                                manual.undo_stack.clear()
                                overlay.mission_context_status = f"YOUR STATE: {state_name} | Ctrl+F2 in mission | Ctrl+F3 outside mission"
                                log({"state": "mission_context_annotated", "mission_context": mission_context,
                                     "expired_role_labels": expired})
                                print(f"YOUR MISSION STATE: {state_name}; previous mission-role labels cleared.", flush=True)
                            state_keys_down[state_name] = state_down
                    if args.overlay:
                        f10_down = key_held(session, F10)
                        if f10_down and not f10_was_down:
                            manual_paused = not manual_paused
                            log({"state": "manual_pause" if manual_paused else "manual_resume"})
                            print("PAUSED BY F10: F10 resumes, F8 stops/saves, F11 exits." if manual_paused
                                  else "RESUMED BY F10: reacquiring current objects.", flush=True)
                            if manual_paused:
                                overlay.clear("PAUSED BY F10 | F10 resume | F8 stop/save | F11 full exit")
                            else:
                                next_survey = 0
                        f10_was_down = f10_down
                    if paused:
                        log({"state": "resumed"})
                        next_survey = 0
                        paused = False
                    if user32.GetAsyncKeyState(0x77) & 0x8000:
                        while user32.GetAsyncKeyState(0x77) & 0x8000:
                            time.sleep(0.05)
                        if not args.lifecycle:
                            stop_reason = "f8_stop"
                            break
                        stage += 1
                        marker_pending = True
                        next_survey = 0
                    if manual_paused:
                        time.sleep(0.1)
                        continue
                    now = time.monotonic()
                    if now >= next_survey:
                        if overlay:
                            overlay.clear("RESCANNING: previous candidate points cleared")
                        refreshed, report = survey(session, base, args.output_directory, sequence, args.ownership_filter)
                        if args.structure_filter:
                            gated = {}
                            rejection_counts = Counter()
                            linked_headers = []
                            for address, lead in refreshed.items():
                                try:
                                    _, _, relation = read_position(session, base, lead, True, args.ownership_filter)
                                    gated[address] = lead
                                    linked_headers.append({"position": lead["address"], "relation": relation})
                                except (OSError, RuntimeError) as error:
                                    rejection_counts[str(error)] += 1
                            write_json(args.output_directory / f"structure-{sequence:04d}.json", {
                                "status": "direct_typed_pawn_filter" if args.ownership_filter else "experimental_structure_filter", "input_count": len(refreshed),
                                "match_count": len(gated), "rejections": dict(rejection_counts),
                                "matching_links": linked_headers})
                            refreshed = gated
                        log({"state": "survey", "sequence": sequence,
                             "added": [f"0x{x:X}" for x in sorted(refreshed.keys()-current.keys())],
                             "removed": [f"0x{x:X}" for x in sorted(current.keys()-refreshed.keys())],
                             "failed_headers": report["failed_headers"], "count": len(refreshed)})
                        current = refreshed
                        sequence += 1
                        next_survey = time.monotonic() + 5
                        print(f"Survey {sequence}: {len(current)} candidates; {report['failed_headers']} failed headers.", flush=True)
                        if marker_pending:
                            label = stages[stage]
                            phase_image = client_capture(session, args.output_directory / f"{label}.png")
                            phase = {"stage": label, "survey_sequence": sequence-1,
                                     "elapsed_s": time.monotonic()-started, "image": phase_image}
                            write_json(args.output_directory / f"{label}.json", phase)
                            log({"state": "lifecycle_stage", **phase})
                            marker_pending = False
                            print(f"STAGE CAPTURE COMPLETE: {label}", flush=True)
                            if stage == len(stages)-1:
                                break
                            print(prompts[stage], flush=True)
                    try:
                        selected = locate_camera(session, base, CAMERA_ROOT, 128)
                        camera = camera_at(session, selected["camera"])
                        if not camera_shape(camera, base):
                            raise RuntimeError("Camera shape changed")
                        after = locate_camera(session, base, CAMERA_ROOT, 128)
                        if camera_identity(after) != camera_identity(selected):
                            raise RuntimeError("Camera changed during sample")
                        rect = Rect()
                        foreground_window = session.require_foreground()
                        if not user32.GetClientRect(foreground_window, ctypes.byref(rect)):
                            raise ctypes.WinError(ctypes.get_last_error())
                        width, height = rect.right-rect.left, rect.bottom-rect.top
                        if width < 320 or height < 240:
                            raise RuntimeError("APB client dimensions invalid")
                    except RuntimeError as error:
                        log({"state": "camera_unavailable", "error": str(error)})
                        if overlay:
                            overlay.clear(f"CAMERA UNAVAILABLE: {error}")
                        print(f"CAMERA UNAVAILABLE: {error}; no projection emitted.", flush=True)
                        time.sleep(0.5)
                        continue
                    points = []
                    local = None
                    if args.ownership_filter:
                        try:
                            local = resolve_local(session, base, selected["camera"])
                        except (OSError, RuntimeError) as error:
                            log({"state": "ownership_unavailable", "error": str(error)})
                            overlay.clear(f"OWNERSHIP UNAVAILABLE: {error}; no projections")
                            print(f"OWNERSHIP UNAVAILABLE: {error}; no projection emitted.", flush=True)
                            time.sleep(0.5)
                            continue
                    for address, lead in list(current.items()):
                        try:
                            xyz, fingerprint, relation = read_position(session, base, lead, args.structure_filter, args.ownership_filter)
                            point = {"address": lead["address"], "marker": lead["marker"], "position": xyz,
                                     "instance_signature": fingerprint, "structure_relation": relation}
                            if args.ownership_filter:
                                owner = owned_position(session, base, point)
                                point["sampled_pawn_position"] = xyz
                                xyz = owner["position"]
                                point["position"] = xyz
                                point["ownership"] = owner
                            point["projection"] = project(camera, xyz, width, height)
                        except (OSError, RuntimeError) as error:
                            log({"state": "candidate_invalidated", "address": lead["address"], "error": str(error)})
                            del current[address]
                            continue
                        points.append(point)
                        item = observations.setdefault(lead["address"], {"samples": 0, "first": xyz,
                            "last": xyz, "minimum": list(xyz), "maximum": list(xyz)})
                        item["samples"] += 1
                        item["last"] = xyz
                        for axis in range(3):
                            item["minimum"][axis] = min(item["minimum"][axis], xyz[axis])
                            item["maximum"][axis] = max(item["maximum"][axis], xyz[axis])
                    excluded_local = []
                    ambiguities = []
                    if args.ownership_filter:
                        try:
                            if resolve_local(session, base, selected["camera"]) != local:
                                raise RuntimeError("Local ownership changed while sampling characters")
                        except (OSError, RuntimeError) as error:
                            log({"state": "ownership_unavailable", "error": str(error)})
                            overlay.clear(f"OWNERSHIP UNAVAILABLE: {error}; no projections")
                            time.sleep(0.5)
                            continue
                        points, excluded_local, ambiguities = filter_owned(points, local)
                        overlay.ownership_status = (f"OWNERSHIP: self hidden {len(excluded_local)} | "
                            f"ambiguous groups withheld {len(ambiguities)} | {local['status']}")
                        if ambiguities:
                            log({"state": "ambiguous_ownership", "groups": ambiguities})
                    if args.mission_labels and mission_state_pending and local and local["pawn"]:
                        from apb_manual_labels import annotation_evidence
                        context_address = int(local["pawn"], 16)
                        context_point = {"address": local["pawn"], "instance_signature": local["pawn_signature"]}
                        context = annotation_evidence(session, base,
                            current.get(context_address, {"address": local["pawn"], "slot_rvas": [],
                                                         "status": "not_in_current_survey"}), context_point, True)
                        primary_bodies = {}
                        for name in ("world", "controller"):
                            primary_address = int(local[name], 16)
                            primary_bodies[name] = {"address": local[name], "extent": "0x3000",
                                "extent_status": "bounded_probe_not_verified_object_size",
                                "body_hex": b"".join(session.read(primary_address+offset, 4096)
                                                     for offset in (0, 4096, 8192)).hex()}
                        if resolve_local(session, base, selected["camera"]) != local:
                            raise RuntimeError("Local ownership changed during mission-state annotation; retry the state key")
                        write_json(args.output_directory / f"mission-state-{mission_state_sequence:04d}.json", {
                            "identity": session.identity, "utc": utc_now(), "mission_context": mission_context,
                            "local_ownership": local, "local_character_context": context,
                            "primary_contexts": primary_bodies, "survey_sequence": sequence-1})
                        log({"state": "mission_context_saved", "sequence": mission_state_sequence,
                             "mission_context": mission_context})
                        mission_state_sequence += 1
                        mission_state_pending = False
                    local_mission = None
                    if args.mission_classify:
                        mission_error = None
                        try:
                            if not local or not local["pawn"]:
                                raise RuntimeError("No current local character; classification waits for reacquisition")
                            local_mission = read_mission(session, base, int(local["pawn"], 16), local["pawn_signature"])
                        except (OSError, RuntimeError) as error:
                            mission_error = str(error)
                            log({"state": "mission_fields_unavailable", "scope": "local", "error": mission_error})
                        for point in points:
                            owner = point["ownership"]
                            try:
                                remote_mission = read_mission(session, base, int(owner["pawn"], 16), owner["pawn_signature"])
                                prediction = classify(local_mission, remote_mission) if local_mission else {
                                    "role": "unknown", "reason": mission_error}
                                point["mission_classification"] = {**prediction, "local_fields": local_mission,
                                                                   "remote_fields": remote_mission}
                            except (OSError, RuntimeError) as error:
                                point["mission_classification"] = {"role": "unknown", "reason": str(error)}
                                log({"state": "mission_fields_unavailable", "scope": point["address"], "error": str(error)})
                        if local_mission:
                            try:
                                if read_mission(session, base, int(local["pawn"], 16), local["pawn_signature"]) != local_mission:
                                    raise RuntimeError("Local mission changed while sampling players")
                            except (OSError, RuntimeError) as error:
                                log({"state": "mission_fields_unavailable", "scope": "local_after_sample", "error": str(error)})
                                for point in points:
                                    point["mission_classification"] = {"role": "unknown", "reason": str(error)}
                        role_counts = Counter(p["mission_classification"]["role"] for p in points)
                        overlay.mission_status = (f"EXPERIMENTAL ROLES: {dict(role_counts)} | "
                            f"display: {'opposition only' if overlay.opposition_only else 'all'} | F4 toggle")
                        if role_counts.get("unknown"):
                            log({"state": "unknown_mission_classification", "count": role_counts["unknown"],
                                 "action": "no automatic role assigned; inspect raw field values"})
                    at = time.monotonic()
                    if previous is not None:
                        max_gap = max(max_gap, at-previous)
                    previous = at
                    samples += 1
                    sample_utc = utc_now()
                    selected_point = None
                    if manual:
                        selectable_points = [point for point in points
                            if not getattr(overlay, "opposition_only", False)
                            or point.get("mission_classification", {}).get("role") == "opposition"]
                        selected_point = manual.select(selectable_points, width, height)
                        pressed = {}
                        label_keys = (("outside_mission", 0x70), ("teammate", 0x71), ("opposition", 0x72)) if args.mission_labels else ()
                        f4_action = "role_filter" if args.mission_classify else "player"
                        for name, vk in label_keys + ((f4_action, 0x73), ("cycle", 0x74), ("exclude", 0x75), ("undo", 0x76)):
                            down = bool(user32.GetAsyncKeyState(vk) & 0x8000)
                            pressed[name] = down and not keys_down.get(name, False)
                            if args.mission_labels and name in ("outside_mission", "teammate", "opposition"):
                                pressed[name] = pressed[name] and not bool(user32.GetAsyncKeyState(0x11) & 0x8000)
                            keys_down[name] = down
                        if pressed.get("role_filter"):
                            overlay.opposition_only = not overlay.opposition_only
                            log({"state": "opposition_display_toggle", "enabled": overlay.opposition_only})
                            selectable_points = [point for point in points
                                if not overlay.opposition_only
                                or point.get("mission_classification", {}).get("role") == "opposition"]
                            selected_point = manual.select(selectable_points, width, height)
                        simultaneous_roles = [name for name, _ in label_keys if pressed.get(name)]
                        if len(simultaneous_roles) > 1:
                            manual.message = "ROLE LABEL REJECTED: press only one of F1/F2/F3"
                            log({"state": "manual_label_rejected", "reason": manual.message, "keys": simultaneous_roles})
                            print(manual.message, flush=True)
                            for name in simultaneous_roles:
                                pressed[name] = False
                        if pressed["cycle"]:
                            manual.cycle()
                            selected_point = manual.nearby[manual.selected_index] if manual.nearby else None
                        if pressed["undo"]:
                            undone = manual.undo()
                            log({"state": "manual_undo", "change": undone})
                        for action in ("exclude", "player") + tuple(name for name, _ in label_keys):
                            if not pressed.get(action):
                                continue
                            if selected_point is None:
                                manual.message = "No candidate within 24 pixels of red reticle; no label applied"
                                log({"state": "manual_label_rejected", "reason": manual.message})
                                continue
                            address = int(selected_point["address"], 16)
                            try:
                                evidence = annotation_evidence(session, base, current[address], selected_point, args.mission_labels)
                                if args.mission_labels and local and local["pawn"]:
                                    local_address = int(local["pawn"], 16)
                                    local_point = {"address": local["pawn"], "instance_signature": local["pawn_signature"]}
                                    evidence["local_character_context"] = annotation_evidence(session, base,
                                        current.get(local_address, {"address": local["pawn"], "slot_rvas": [],
                                                                   "status": "not_in_current_survey"}), local_point, True)
                                    evidence["local_ownership"] = local
                            except (OSError, RuntimeError) as error:
                                manual.message = str(error)
                                log({"state": "manual_label_rejected", "reason": str(error)})
                                print(f"LABEL REJECTED: {error}", flush=True)
                                continue
                            record = {"identity": session.identity, "utc": utc_now(), "sample_utc": sample_utc,
                                "label": action, "point": selected_point, "camera": camera,
                                "mission_context": dict(mission_context),
                                "local_mission_fields": local_mission,
                                "opposition_only_display": getattr(overlay, "opposition_only", False),
                                "survey_sequence": sequence-1, **evidence,
                                "status": "operator_label_not_verified_player_identity"}
                            write_json(args.output_directory / f"annotation-{annotation_sequence:04d}.json", record)
                            change = manual.assign(selected_point, action)
                            log({"state": "manual_label", "sequence": annotation_sequence, **change})
                            print(f"{action.upper()}: {selected_point['address']} saved annotation-{annotation_sequence:04d}.json", flush=True)
                            annotation_sequence += 1
                        for point in points:
                            point["manual_label"] = manual.labels.get(instance_key(point), "unlabelled")
                    log({"state": "sample", "elapsed_s": at-started,
                         "sample_utc": sample_utc,
                         "camera": f"0x{selected['camera']:X}", "camera_position": camera["position"],
                         "matrix": camera["matrix"], "client_size": [width, height], "points": points,
                         "local_ownership": local, "excluded_local": excluded_local,
                         "mission_context": dict(mission_context),
                         "local_mission_fields": local_mission,
                         "opposition_only_display": getattr(overlay, "opposition_only", False),
                         "ambiguous_ownership": ambiguities})
                    if overlay:
                        overlay.draw(points, manual, selected_point)
                        f9_down = bool(user32.GetAsyncKeyState(0x78) & 0x8000)
                        if f9_down and not f9_was_down:
                            image = client_capture(session, args.output_directory / f"visual-{frame_sequence:04d}.png",
                                                   include_layered_windows=True)
                            write_json(args.output_directory / f"visual-{frame_sequence:04d}.json", {
                                "identity": session.identity, "sample_utc": sample_utc, "elapsed_s": at-started,
                                "camera": camera, "image": image, "points": points,
                                "local_ownership": local, "excluded_local": excluded_local,
                                "mission_context": dict(mission_context),
                                "local_mission_fields": local_mission,
                                "opposition_only_display": getattr(overlay, "opposition_only", False),
                                "ambiguous_ownership": ambiguities,
                                "manual_labels": dict(manual.labels) if manual else None,
                                "status": "experimental_mission_roles_for_visual_comparison" if args.mission_classify
                                          else "unclassified_projected_candidates_for_visual_comparison"})
                            print(f"VISUAL FRAME SAVED: visual-{frame_sequence:04d}.png", flush=True)
                            frame_sequence += 1
                        f9_was_down = f9_down
                    time.sleep(0.2)
                except ForegroundLost as error:
                    if not args.overlay:
                        raise
                    overlay.hide()
                    if not paused:
                        log({"state": "paused", "error": str(error)})
                        print("PAUSED: refocus APB; overlay hidden, recording retained.", flush=True)
                    paused = True
                    keys_down.clear()
                    f9_was_down = False
                    time.sleep(0.1)
        write_json(args.output_directory / "summary.json", {"identity": session.identity,
            "samples": samples, "surveys": sequence, "max_sample_gap_s": max_gap,
            "lifecycle_requested": args.lifecycle,
            "lifecycle_completed": args.lifecycle and stage == len(stages)-1 and not marker_pending,
            "stop_reason": stop_reason,
            "observations": observations, "status": "unclassified_candidate_lifecycle_recording"})
        if manual:
            write_json(args.output_directory / "manual_labels.json", {"identity": session.identity,
                "labels": manual.labels, "annotation_count": annotation_sequence,
                "status": "operator_labels_not_verified_player_identity"})
        print(f"CAPTURE COMPLETE: {samples} rounds, {sequence} surveys. Saved {args.output_directory}", flush=True)
        if (not samples or not observations) and stop_reason not in ("f8_stop", "full_exit"):
            raise RuntimeError("No usable candidate samples; inspect saved coverage and logs")
        if not samples or not observations:
            print("STOPPED WITHOUT USABLE SAMPLES: intentional stop saved; start another recording with F8.", flush=True)
        if args.lifecycle and stage < len(stages)-1:
            raise RuntimeError("Lifecycle time limit reached before all stages; completed evidence retained")
        return "exit" if stop_reason == "full_exit" else "stop"
    finally:
        if overlay:
            overlay.close()
        session.close()


def main():
    args = parse_args()
    root = args.output_directory
    sequence = 1
    while True:
        capture_args = copy.copy(args)
        if sequence > 1:
            capture_args.output_directory = root / f"recording-{sequence:04d}"
        result = record(capture_args)
        if not args.overlay or result == "exit":
            return
        sequence += 1


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        raise SystemExit(f"District candidate recording failed: {error}; completed files retained")
