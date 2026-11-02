"""Read-only world-object annotations bound to an explicit discovery profile."""

import argparse
import ctypes
import json
import math
import struct
import time
from datetime import datetime, timezone
from pathlib import Path

from apb_candidate_overlay import CandidateOverlay
from apb_cursor_scan import Session, ForegroundLost
from apb_f8_common import Point, Rect, project
from apb_manual_labels import instance_signature


class CandidateExpired(RuntimeError):
    """The captured object instance was replaced; its projection must be removed."""


def instance_key(point):
    return f"{point['address']}:{point['instance_signature']}:{integer(point['position_offset'])}"


def integer(value):
    return int(value, 0) if isinstance(value, str) else int(value)


def load_profile(path, session):
    profile = json.loads(path.read_text(encoding="utf-8-sig"))
    if profile.get("format") != "apb-build-candidates-v1":
        raise RuntimeError("Expected apb-build-candidates-v1 discovery profile")
    if profile.get("status") != "unverified_build_candidates":
        raise RuntimeError("Profile status must be unverified_build_candidates")
    for key in ("pid", "created_filetime", "image_path", "image_sha256"):
        if profile["identity"][key] != session.identity[key]:
            raise RuntimeError(f"Profile process identity differs: {key}; obtain a fresh scan")
    if not profile.get("camera") or not profile.get("candidates"):
        raise RuntimeError("Profile needs an explicit camera and object candidates")
    for spec in [profile["camera"], *profile["candidates"]]:
        for key in ("address", "vtable_rva", "marker", "position_offset", "instance_signature"):
            if key not in spec:
                raise RuntimeError(f"Profile object missing {key}")
        if not 0x40 <= integer(spec["position_offset"]) <= 0x4000:
            raise RuntimeError("Position offset outside bounded discovery extent")
    if not 0x40 <= integer(profile["camera"]["matrix_offset"]) <= 0x4000:
        raise RuntimeError("Camera matrix offset outside bounded discovery extent")
    integer(profile["module_base"])
    return profile


def read_typed(session, base, spec, camera=False):
    address = integer(spec["address"])
    offset = integer(spec["position_offset"])
    size = max(0x100, offset + 12, integer(spec["matrix_offset"]) + 64 if camera else 0)
    body = b"".join(session.read(address + offset, min(4096, size-offset))
                    for offset in range(0, size, 4096))
    if (instance_signature(body) != spec["instance_signature"]
            or struct.unpack_from("<Q", body)[0] != base + integer(spec["vtable_rva"])
            or struct.unpack_from("<I", body, 0x3C)[0] != integer(spec["marker"])
            or instance_signature(session.read(address, 0x40)) != spec["instance_signature"]):
        raise CandidateExpired(f"Object fingerprint/type changed at {hex(address)}; rescan")
    xyz = struct.unpack_from("<3f", body, offset)
    if not all(math.isfinite(v) and abs(v) < 1e9 for v in xyz):
        raise RuntimeError(f"Invalid position at {hex(address)} + {hex(offset)}")
    result = {**spec, "address": hex(address), "position_offset": offset,
              "xyz": xyz, "body_hex": body.hex(), "capture_extent": size,
              "extent_status": "bounded_probe_not_verified_object_size"}
    if camera:
        matrix = struct.unpack_from("<16f", body, integer(spec["matrix_offset"]))
        if (not all(math.isfinite(v) and abs(v) < 1e9 for v in matrix)
                or any(sum(v*v for v in matrix[col::4]) < 1e-12 for col in (0, 1, 3))):
            raise RuntimeError("Camera matrix failed finite/nondegenerate shape check")
        result.update(matrix=matrix, position=xyz)
    return result


def draw(overlay, points, selected, labels, message):
    width, height = overlay.place()
    canvas = overlay.canvas
    canvas.delete("all")
    canvas.create_text(18, 175, anchor="nw", fill="#ffdc60", font=("Consolas", 11, "bold"),
        text=f"OBJECT CANDIDATES | F6 tag | F5 next | F7 undo | F8 start/stop/reload | F11 exit\n{message}")
    canvas.create_line(width/2-6, height/2, width/2+6, height/2, fill="#ff6868", width=2)
    canvas.create_line(width/2, height/2-6, width/2, height/2+6, fill="#ff6868", width=2)
    for point in points:
        x, y = point["projection"]["client_x"], point["projection"]["client_y"]
        tag = labels.get(instance_key(point), "candidate")
        color = "#73ff8c" if tag != "candidate" else "#57e8ff"
        canvas.create_oval(x-4, y-4, x+4, y+4, outline=color, width=2)
        name = point.get("object_name", "")
        canvas.create_text(x+9, y-9, text=f"{tag} {name} {point['address']} +{hex(point['position_offset'])}",
                           anchor="sw", fill=color, font=("Consolas", 9))
        if point is selected:
            canvas.create_oval(x-12, y-12, x+12, y+12, outline="#ff6868", width=3)
    overlay.root.update()


def run(args, session):
    for name, types in (("GetClientRect", [ctypes.c_void_p, ctypes.POINTER(Rect)]),
                        ("ClientToScreen", [ctypes.c_void_p, ctypes.POINTER(Point)])):
        function = getattr(session.user32, name)
        function.argtypes, function.restype = types, ctypes.c_bool
    session.user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
    session.user32.GetAsyncKeyState.restype = ctypes.c_short
    profile = load_profile(args.profile, session)
    overlay = CandidateOverlay(session)
    labels, undo, previous = {}, [], {}
    active, selected_key, message = False, None, "F8 starts explicit profile candidates"
    lost_focus = False
    expired_count = 0
    with args.output.open("x", encoding="utf-8", buffering=1) as stream:
        def log(event, **data):
            stream.write(json.dumps({"utc": datetime.now(timezone.utc).isoformat(),
                "event": event, "identity": session.identity, "profile": str(args.profile.resolve()), **data}) + "\n")
        log("ready", message=message)
        try:
            while True:
                overlay.root.update()
                pressed = {}
                for vk in (0x74, 0x75, 0x76, 0x77, 0x7A):
                    held = bool(session.user32.GetAsyncKeyState(vk) & 0x8000)
                    pressed[vk], previous[vk] = held and not previous.get(vk, False), held
                if pressed[0x7A]:
                    log("closed")
                    return
                try:
                    session.require_foreground()
                    if lost_focus:
                        log("focus_restored")
                        lost_focus = False
                    if pressed[0x77]:
                        active = not active
                        if active:
                            profile = load_profile(args.profile, session)
                            expired_count = 0
                        selected_key = None
                        message = "Live unverified candidates" if active else "Stopped; F8 reloads profile"
                        log("started" if active else "stopped")
                    if not active:
                        draw(overlay, [], None, labels, message)
                        time.sleep(0.04)
                        continue
                    base = integer(profile["module_base"])
                    camera = read_typed(session, base, profile["camera"], camera=True)
                    width, height = overlay.place()
                    sampled = []
                    expired = set()
                    for spec in profile["candidates"]:
                        try:
                            point = read_typed(session, base, spec)
                        except CandidateExpired as error:
                            key = instance_key(spec)
                            log("candidate_expired", instance_key=key, message=str(error),
                                removed_tag=labels.get(key))
                            labels.pop(key, None)
                            expired.add(key)
                            continue
                        sampled.append(point)
                    if expired:
                        expired_count += len(expired)
                        profile["candidates"] = [spec for spec in profile["candidates"]
                                                 if instance_key(spec) not in expired]
                    camera = read_typed(session, base, profile["camera"], camera=True)
                    points = []
                    for point in sampled:
                        selection = profile.get("selection", {})
                        if (selection.get("kind") == "typed emitter-to-prefab associations"
                                and math.dist(point["xyz"], camera["position"]) > selection["radius_world_units"]):
                            continue
                        point["projection"] = project(camera, point["xyz"], width, height)
                        if point["projection"]["status"] == "inside":
                            points.append(point)
                    points.sort(key=lambda p: math.hypot(p["projection"]["client_x"]-width/2,
                                                          p["projection"]["client_y"]-height/2))
                    nearby = [p for p in points if math.hypot(p["projection"]["client_x"]-width/2,
                                                              p["projection"]["client_y"]-height/2) <= 36]
                    keys = [instance_key(point) for point in nearby]
                    if pressed[0x74] and keys:
                        selected_key = keys[(keys.index(selected_key)+1) % len(keys)] if selected_key in keys else keys[1 % len(keys)]
                    selected = next((p for p in nearby if instance_key(p) == selected_key), nearby[0] if nearby else None)
                    if pressed[0x75] and selected:
                        session.require_foreground()
                        evidence = read_typed(session, base, selected)
                        key = instance_key(selected)
                        old = labels.get(key)
                        log("annotation", tag=args.tag, previous_tag=old, point=evidence,
                            camera={**profile["camera"], "body_hex": camera["body_hex"]},
                            module_base=profile["module_base"], projection=selected["projection"])
                        undo.append((key, old))
                        labels[key] = args.tag
                        message = f"Tagged {selected['address']}: {args.tag}"
                    elif pressed[0x75]:
                        message = "No candidate at the centre reticle"
                    if pressed[0x76] and undo:
                        key, old = undo[-1]
                        log("undo", instance_key=key, restored_tag=old)
                        undo.pop()
                        if old is None:
                            labels.pop(key, None)
                        else:
                            labels[key] = old
                        message = "Last annotation undone"
                    session.require_foreground()
                    draw(overlay, points, selected, labels,
                         f"{message} | On screen: {len(points)} | Expired instances removed: {expired_count}")
                except ForegroundLost as error:
                    overlay.hide()
                    selected_key = None
                    if not lost_focus:
                        log("focus_lost", message=str(error))
                        lost_focus = True
                except (OSError, RuntimeError, KeyError, ValueError, struct.error) as error:
                    active, selected_key = False, None
                    overlay.hide()
                    message = f"Stopped: {error}. F8 retries explicit profile."
                    log("error", message=message)
                    print(message, flush=True)
                time.sleep(0.04)
        finally:
            overlay.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--profile", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--tag", default="pumpkin")
    args = parser.parse_args()
    if not args.tag.strip() or len(args.tag) > 80 or any(ord(c) < 32 for c in args.tag):
        parser.error("Tag must be 1–80 printable characters")
    session = Session(args.pid)
    try:
        run(args, session)
    finally:
        session.close()


if __name__ == "__main__":
    main()
