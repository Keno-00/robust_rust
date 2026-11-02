"""Overlay-only current-build validation and timestamped object annotation."""

import argparse
import ctypes
import json
import math
import struct
import time
from datetime import datetime, timezone
from pathlib import Path

from apb_candidate_overlay import CandidateOverlay
from apb_cursor_candidate_probe import CursorCandidateProbe
from apb_cursor_scan import Session, ForegroundLost
from apb_f8_common import Point, Rect, project
from apb_live_objects import LiveObjectSurvey, SurveyChanged
from apb_object_annotation_overlay import CandidateExpired, instance_key, load_profile, read_typed
from apb_validation_runtime import ValidationRuntime, RuntimeChanged


def draw(overlay, mode, points, selected, labels, message, sample, tag):
    width, height = overlay.place()
    c = overlay.canvas
    c.delete("all")
    controls = {"players": "F5 next | F6 lock target | F1 exposed | F2 blocked | F3 unknown",
                "objects": f"F5 next | F6 tag {tag} | F3 switch pumpkin/grenade | F7 undo",
                "cursor": "F1 WHITE | F2 GREEN (car) | F3 RED (if available)"}[mode]
    pose = sample["local"]["pose"] if sample else None
    local = (f"{sample['local']['state']} | half-height {pose['half_height']:.1f} radius {pose['radius']:.1f}"
             if sample else "Waiting for live sample")
    c.create_text(18, 175, anchor="nw", fill="#ffdc60", font=("Consolas", 10, "bold"),
                  text=f"{mode.upper()} | F4 mode | F8 start/stop | F11 exit\n{controls}\n{local}\n{message}")
    c.create_line(width/2-6, height/2, width/2+6, height/2, fill="#ff6868", width=2)
    c.create_line(width/2, height/2-6, width/2, height/2+6, fill="#ff6868", width=2)
    for point in points:
        p = point["projection"]
        x, y = p["client_x"], p["client_y"]
        if mode == "players":
            color = "#73ff8c" if point["recently_rendered"] else "#ffdc60"
            text = f"{'RECENT' if point['recently_rendered'] else 'STALE'} {point['age']:.2f}s {point['address']}"
        else:
            label = labels.get(instance_key(point), "candidate")
            color = "#73ff8c" if label != "candidate" else "#57e8ff"
            age = (f" seen {time.monotonic()-point['first_seen_monotonic']:.2f}s"
                   if "first_seen_monotonic" in point else "")
            text = f"{label} {point.get('object_name', '')} {point['address']}{age}"
        c.create_oval(x-4, y-4, x+4, y+4, outline=color, width=2)
        c.create_text(x+9, y-9, text=text, anchor="sw", fill=color, font=("Consolas", 9))
        if selected and instance_key(point) == instance_key(selected):
            c.create_oval(x-12, y-12, x+12, y+12, outline="#ff6868", width=3)
    overlay.root.update()


def run(args, session):
    for name, types in (("GetClientRect", [ctypes.c_void_p, ctypes.POINTER(Rect)]),
                        ("ClientToScreen", [ctypes.c_void_p, ctypes.POINTER(Point)])):
        f = getattr(session.user32, name)
        f.argtypes, f.restype = types, ctypes.c_bool
    session.user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
    session.user32.GetAsyncKeyState.restype = ctypes.c_short
    profile = load_profile(args.profile, session)
    overlay = CandidateOverlay(session)
    active, mode, tag = False, "players", "pumpkin"
    previous, labels, undo = {}, {}, []
    selected_key, locked = None, None
    message, sample = "F8 starts | preliminary render recency, not geometry LOS", None
    specs, next_survey, expired, next_log = [], 0, set(), 0
    args.output_directory.mkdir(parents=True, exist_ok=False)
    with (args.output_directory / "events.jsonl").open("x", encoding="utf-8", buffering=1) as stream:
        def log(event, **fields):
            stream.write(json.dumps({"utc": datetime.now(timezone.utc).isoformat(),
                "monotonic_s": time.monotonic(), "identity": session.identity,
                "event": event, **fields}, allow_nan=False)+"\n")

        runtime = ValidationRuntime(session, profile, diagnostic=log)
        survey = LiveObjectSurvey(session, profile, diagnostic=lambda event: log(event.pop("event"), **event))
        cursor = CursorCandidateProbe(session, runtime.base)
        log("ready", profile=str(args.profile.resolve()), modes=["players", "objects", "cursor"])
        try:
            while True:
                overlay.root.update()
                keys = {}
                for vk in (0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x7A):
                    held = bool(session.user32.GetAsyncKeyState(vk) & 0x8000)
                    keys[vk], previous[vk] = held and not previous.get(vk, False), held
                if keys[0x7A]:
                    log("closed")
                    return
                if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
                    raise RuntimeError("APB exited. Close this overlay and obtain a fresh process-bound profile.")
                try:
                    session.require_foreground()
                    if keys[0x73]:
                        modes = ["players", "objects", "cursor"]
                        mode = modes[(modes.index(mode)+1) % len(modes)]
                        selected_key = locked = None
                        next_survey = 0
                        message = "Render recency candidate, not geometry LOS" if mode == "players" else "Ready"
                        log("mode", mode=mode)
                    if keys[0x77]:
                        active = not active
                        next_survey = 0
                        message = "Live" if active else "Stopped; F8 resumes"
                        log("started" if active else "stopped", mode=mode)
                    if not active:
                        draw(overlay, mode, [], None, labels, message, sample, tag)
                        time.sleep(.04)
                        continue
                    sample = runtime.sample()
                    width, height = overlay.place()
                    if mode == "cursor":
                        for vk, label in ((0x70, "white"), (0x71, "green"), (0x72, "red")):
                            if keys[vk]:
                                overlay.clear(f"Capturing {label.upper()}; hold cursor state steady")
                                result = cursor.capture(label)
                                log("cursor_capture", world_time=sample["world_clock"], **result)
                                message = f"{label.upper()} saved | {result['candidate_count']} candidates | repeat WHITE/GREEN"
                        draw(overlay, mode, [], None, labels, message, sample, tag)
                        time.sleep(.05)
                        continue
                    if mode == "players":
                        raw_points = sample["points"]
                    else:
                        if keys[0x72]:
                            tag = "grenade" if tag == "pumpkin" else "pumpkin"
                            message = f"Tag: {tag}"
                        if time.monotonic() >= next_survey:
                            specs = []
                            try:
                                specs = survey.refresh(sample["local"]["world"], sample["camera"],
                                                       world_time=sample["world_clock"])
                            except SurveyChanged as error:
                                message = f"Actor membership changing: {error}"
                                log("survey_changed", message=str(error))
                            next_survey = time.monotonic()+1.0
                        raw_points = []
                        for spec in specs:
                            key = instance_key(spec)
                            if key in expired:
                                continue
                            try:
                                point = read_typed(session, runtime.base, spec)
                                if spec.get("candidate_kind") == "grenade_or_projectile_name_candidate":
                                    point["survey_evidence"] = {"body_hex": spec["body_hex"],
                                        "capture_extent": spec["capture_extent"],
                                        "observed_utc": spec["last_seen_utc"],
                                        "observed_world_time": spec["last_seen_world_time"],
                                        "extent_status": spec["extent_status"]}
                                raw_points.append(point)
                            except CandidateExpired as error:
                                expired.add(key)
                                log("candidate_expired", instance_key=key, message=str(error), removed_tag=labels.pop(key, None))
                        # Project with the view sampled after the actor survey.
                        sample = runtime.sample()
                    points = []
                    for point in raw_points:
                        point["projection"] = project(sample["camera"], point["xyz"], width, height)
                        if point["projection"]["status"] == "inside":
                            points.append(point)
                    points.sort(key=lambda p: math.hypot(p["projection"]["client_x"]-width/2,
                                                          p["projection"]["client_y"]-height/2))
                    nearby = [p for p in points if math.hypot(p["projection"]["client_x"]-width/2,
                                                              p["projection"]["client_y"]-height/2) <= 36]
                    nearby_keys = [instance_key(p) for p in nearby]
                    if keys[0x74] and nearby_keys:
                        selected_key = nearby_keys[(nearby_keys.index(selected_key)+1) % len(nearby_keys)] if selected_key in nearby_keys else nearby_keys[1 % len(nearby_keys)]
                        locked = None
                    selected = next((p for p in nearby if instance_key(p) == selected_key), nearby[0] if nearby else None)
                    if mode == "players":
                        if keys[0x75]:
                            locked = instance_key(selected) if selected else None
                            log("target_lock", instance_key=locked)
                        if locked:
                            selected = next((p for p in raw_points if instance_key(p) == locked), None)
                            if selected is None:
                                message = "Selected instance expired; choose another target"
                                log("target_expired", instance_key=locked)
                                locked = None
                        for vk, label in ((0x70, "exposed"), (0x71, "blocked"), (0x72, "unknown")):
                            if keys[vk]:
                                if selected:
                                    log("operator_visibility", label=label, point=selected, camera=sample["camera"],
                                        local=sample["local"], world_time=sample["world_clock"])
                                    message = f"Your label: {label} | candidate age {selected['age']:.2f}s"
                                else:
                                    message = "Aim at a pawn marker and F6 lock it first"
                    else:
                        if keys[0x75]:
                            if selected:
                                evidence = read_typed(session, runtime.base, selected)
                                key = instance_key(selected)
                                log("annotation", tag=tag, previous_tag=labels.get(key), point=evidence,
                                    camera=sample["camera"], world_time=sample["world_clock"],
                                    timestamp_semantics="observed/annotated time, not verified spawn or fuse")
                                undo.append((key, labels.get(key)))
                                labels[key] = tag
                                message = f"Tagged {tag}: {selected['address']}"
                            else:
                                message = "No candidate at the centre reticle"
                        if keys[0x76] and undo:
                            key, old = undo.pop()
                            log("undo", instance_key=key, restored_tag=old)
                            if old is None:
                                labels.pop(key, None)
                            else:
                                labels[key] = old
                            message = "Last annotation undone"
                    if time.monotonic() >= next_log:
                        log("sample", mode=mode, local=sample["local"], world_time=sample["world_clock"],
                            points=sample["points"] if mode == "players" else raw_points,
                            visibility_scope="render recency only; geometry LOS unverified")
                        next_log = time.monotonic()+.5
                    draw(overlay, mode, points, selected, labels,
                         f"{message} | on screen {len(points)} | expired {len(expired)}", sample, tag)
                except ForegroundLost:
                    overlay.hide()
                except RuntimeChanged as error:
                    log("runtime_changed", message=str(error), mode=mode)
                    try:
                        overlay.clear(f"Waiting for a stable snapshot: {error}\nF8 stops | F11 exits")
                    except ForegroundLost:
                        overlay.hide()
                except (OSError, RuntimeError, KeyError, ValueError, struct.error) as error:
                    active = False
                    overlay.hide()
                    message = f"Unavailable: {error}. F8 retries current reader."
                    log("error", message=str(error), mode=mode)
                    print(message, flush=True)
                time.sleep(.04)
        finally:
            overlay.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("profile", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    session = Session(args.pid)
    try:
        run(args, session)
    finally:
        session.close()


if __name__ == "__main__":
    main()
