"""Live readable character/component overlay; EchoDriver is the only memory reader."""

import argparse
import ctypes
import json
import math
import struct
import sys
import time
import traceback
import tkinter as tk
from tkinter import simpledialog
from pathlib import Path

from apb_candidate_overlay import CandidateOverlay
from apb_camera_live import locate_camera, camera_shape, camera_identity
from apb_capture_controls import down
from apb_character_ownership import resolve_local, owned_position, filter_owned, typed_header, COMPONENT_TYPE, PAWN_TYPE
from apb_mission_classification import read_mission, classify
from apb_cursor_scan import ForegroundLost
from apb_district_candidates import survey, read_position, CAMERA_ROOT
from apb_f8_common import Point, Rect, Session, module_base, camera_at, project, client_capture, utc_now, write_json
from apb_manual_labels import instance_signature, annotation_evidence


def readable(point, page=0):
    owner = point["ownership"]
    x, y, z = point["position"]
    summary = [f"Character #{point['id']} | world position: X {x:.1f}, Y {y:.1f}, height Z {z:.1f}",
            f"Ownership: current character/component links checked | {len(owner['components'])} registered link(s)",
            "Vehicle: attached" if owner["vehicle_attachment"] else "Vehicle: no checked attachment",
            f"Screen: {point['projection']['status']} (screen projection does not check obstruction)",
            f"Shooting path: {point.get('operator_observation', 'not annotated')} (your label)",
            "Bones / mesh triangles: mapping not established",
            "C markers: checked position components; P marker: character origin"]
    detail = point.get("decoded_fields", {})
    bounds = detail.get("collision")
    if bounds:
        summary.insert(3, f"Collision size: height {bounds['half_height']*2:.1f}, radius {bounds['radius']:.1f} world units")
    elif detail:
        summary.insert(3, "Collision bounds unavailable — F6 page 3 shows the diagnostic")
    summary.insert(4, "Mission role hypothesis: " + detail.get("mission", {}).get("role", "unknown"))
    if page == 0 or not detail:
        return summary
    if page == 1:
        return ["RELATED FIELD LEADS | values decoded; meanings are hypotheses",
                f"Stance/eye-height lead: {detail['eye_height_lead']:.2f} (not a head bone)",
                f"Related height scalars: {detail['height_related_a']:.2f}, {detail['height_related_b']:.2f}",
                "Motion-vector lead: " + ", ".join(f"{v:.2f}" for v in detail['motion_vector_lead']) + " (velocity unconfirmed)",
                f"Paired scalar leads: {detail['paired_scalars'][0]:.2f}, {detail['paired_scalars'][1]:.2f} (meaning unknown)",
                f"Health/config candidate A: {detail['integer_a']} | B: {detail['integer_b']} (health UNCONFIRMED)",
                f"Character flag bits: {detail['flags']} (decimal; meanings unresolved)",
                f"Driving-associated bit: {'on' if detail['flags'] & 0x400 else 'off'} (not a universal vehicle detector)",
                "These values describe this pawn type; marker-294 lifecycle flags are a different type."]
    return ["CHECKED COLLISION COMPONENT | not skeletal mesh wireframe",
            f"Origin: {bounds['origin']}" if bounds else "Collision bounds unavailable",
            f"Half-size: {bounds['extent']}" if bounds else detail.get("collision_error", "No collision source"),
            f"Half-height: {bounds['half_height']:.2f} | radius: {bounds['radius']:.2f}" if bounds else "No guessed dimensions",
            "Owner backlinks and component type rechecked" if bounds else "No body box emitted",
            "Pink box = collision bounds; it does not identify head, arms or legs.",
            "Player-name text, health, bones and automatic LOS: not verified."]


def decoded_fields(session, base, point, local):
    pawn = int(point["address"], 16)
    body = session.read(pawn, 0xF00)
    if instance_signature(body) != point["instance_signature"]:
        raise RuntimeError("Pawn changed before related-field decoding")
    scalar = lambda offset: struct.unpack_from("<f", body, offset)[0]
    fields = {"flags": struct.unpack_from("<I", body, 0x84)[0],
              "eye_height_lead": scalar(0x328), "height_related_a": scalar(0x374),
              "height_related_b": scalar(0x448), "motion_vector_lead": struct.unpack_from("<fff", body, 0x6B4),
              "integer_a": struct.unpack_from("<I", body, 0x4B4)[0],
              "integer_b": struct.unpack_from("<I", body, 0x514)[0],
              "paired_scalars": [scalar(0xE08), scalar(0xEB8)], "raw_body_hex": body.hex()}
    if not all(math.isfinite(v) for v in [fields['eye_height_lead'], fields['height_related_a'],
                fields['height_related_b'], *fields['motion_vector_lead'], *fields['paired_scalars']]):
        raise RuntimeError("Related scalar decoding produced nonfinite values; inspect raw log")
    try:
        remote = read_mission(session, base, pawn, point["instance_signature"])
        own = read_mission(session, base, int(local["pawn"], 16), local["pawn_signature"])
        fields["mission"] = {**classify(own, remote), "local_fields": own, "remote_fields": remote,
                             "status": "experimental_recorded_hypothesis"}
    except (OSError, RuntimeError) as error:
        fields["mission"] = {"role": "unknown", "reason": str(error)}
    try:
        address = struct.unpack_from("<Q", body, 0x248)[0]
        stamp = typed_header(session, base, address, (0x3197448, 3269))
        raw = session.read(address, 0x240)
        origin = struct.unpack_from("<fff", raw, 0x108)
        extent = struct.unpack_from("<fff", raw, 0x114)
        sphere = struct.unpack_from("<f", raw, 0x120)[0]
        half_height, radius = struct.unpack_from("<ff", raw, 0x234)
        if (instance_signature(raw) != stamp or struct.unpack_from("<Q", raw, 0x44)[0] != pawn
                or struct.unpack_from("<Q", raw, 0x7C)[0] != pawn
                or not all(math.isfinite(v) and abs(v) < 1_000_000 for v in origin)
                or not all(math.isfinite(v) and v > 0 for v in [*extent, sphere, half_height, radius])
                or not 10 <= half_height <= 300 or not 1 <= radius <= 150
                or max(abs(extent[0]-radius), abs(extent[1]-radius), abs(extent[2]-half_height)) > 0.05
                or abs(sphere-math.sqrt(sum(v*v for v in extent))) > 0.05):
            raise RuntimeError("Collision ownership/dimensions do not match checked layout")
        if (session.read(pawn+0x248, 8) != body[0x248:0x250]
                or typed_header(session, base, address, (0x3197448, 3269)) != stamp
                or session.read(address+0x44, 8) != raw[0x44:0x4C]
                or session.read(address+0x7C, 8) != raw[0x7C:0x84]
                or session.read(address+0x234, 8) != raw[0x234:0x23C]):
            raise RuntimeError("Collision component changed during decode")
        fields["collision"] = {"address": hex(address), "origin": origin, "extent": extent,
                               "sphere_radius": sphere, "half_height": half_height, "radius": radius,
                               "raw_body_hex": raw.hex()}
    except (OSError, RuntimeError) as error:
        fields["collision_error"] = str(error)
    if typed_header(session, base, pawn, PAWN_TYPE) != point["instance_signature"]:
        raise RuntimeError("Pawn changed during related-field decoding")
    return fields


def components(session, base, point, camera, width, height):
    result = []
    for index, value in enumerate(point["ownership"]["components"], 1):
        address = int(value, 16)
        header = session.read(address, 0x40)
        kind = (struct.unpack_from("<Q", header)[0]-base, struct.unpack_from("<I", header, 0x3C)[0])
        if kind != COMPONENT_TYPE:
            result.append({"id": index, "address": value, "meaning": "Registered link; spatial layout unverified"})
            continue
        signature = instance_signature(header)
        body = session.read(address, 0x180)
        xyz = struct.unpack_from("<fff", body, 0x88)
        if (instance_signature(body) != signature
                or struct.unpack_from("<Q", body, 0x178)[0] != int(point["address"], 16)
                or typed_header(session, base, address, COMPONENT_TYPE) != signature
                or session.read(address+0x178, 8) != body[0x178:0x180]
                or not all(math.isfinite(v) and abs(v) < 1_000_000 for v in xyz)):
            raise RuntimeError(f"Position component changed during sample: {value}")
        result.append({"id": index, "address": value, "position": xyz,
                       "projection": project(camera, xyz, width, height),
                       "meaning": "Checked position component (not a head/arm/leg bone)", "body_hex": body.hex()})
    return result


def draw(overlay, points, selected, expanded, message, camera):
    width, height = overlay.place()
    canvas = overlay.canvas
    canvas.delete("all")
    canvas.create_text(16, 165, text="F8 start/stop | F4 current | F5 cycle | F6 detail pages | F9 comment | F10 pause | F11 exit\n"
                       "F1 visible | F2 shot blocked | F3 partial/cursor clear | F7 partial/cursor blocked\n" + message,
                       anchor="nw", fill="#ffdc60", font=("Consolas", 10, "bold"))
    for point in points:
        p = point["projection"]
        if p["status"] != "inside":
            continue
        x, y = p["client_x"], p["client_y"]
        color = "#73ff8c" if selected is point else "#57e8ff"
        canvas.create_oval(x-5, y-5, x+5, y+5, outline=color, width=2)
        canvas.create_text(x+10, y-10, text=f"P{point['id']} character origin", anchor="sw", fill=color,
                           font=("Consolas", 9, "bold"))
    canvas.create_line(width/2-8, height/2, width/2+8, height/2, fill="#ff6868", width=2)
    canvas.create_line(width/2, height/2-8, width/2, height/2+8, fill="#ff6868", width=2)
    if selected:
        p = selected["projection"]
        if p["status"] == "inside":
            x, y = p["client_x"], p["client_y"]
            canvas.create_oval(x-14, y-14, x+14, y+14, outline="#73ff8c", width=3)
        for component in selected.get("visual_components", []):
            cp = component.get("projection", {})
            if cp.get("status") == "inside":
                x, y = cp["client_x"], cp["client_y"]
                canvas.create_rectangle(x-8, y-8, x+8, y+8, outline="#ff70ef", width=2)
                canvas.create_text(x+10, y+12, text=f"C{component['id']} position component", anchor="nw",
                                   fill="#ff70ef", font=("Consolas", 9))
        bounds = selected.get("decoded_fields", {}).get("collision")
        if bounds:
            corners = [project(camera, [bounds['origin'][axis] + sign[axis]*bounds['extent'][axis]
                       for axis in range(3)], width, height)
                       for sign in ((-1,-1,-1), (-1,-1,1), (-1,1,-1), (-1,1,1),
                                    (1,-1,-1), (1,-1,1), (1,1,-1), (1,1,1))]
            for a in range(8):
                for bit in (1, 2, 4):
                    b = a ^ bit
                    if b > a and all(corners[i]['status'] in ('inside', 'outside') for i in (a,b)):
                        canvas.create_line(corners[a]['client_x'], corners[a]['client_y'],
                                           corners[b]['client_x'], corners[b]['client_y'], fill="#ff70ef", width=2)
        if expanded is not None:
            lines = [f"DETAIL PAGE {expanded+1}/3 | F6 next"] + readable(selected, expanded)
            lines += [f"C{c['id']}: {c['meaning']}" for c in selected.get("visual_components", [])]
            canvas.create_rectangle(10, height-22-len(lines)*17, min(width-10, 850), height-10,
                                    fill="#171c25", outline="#73ff8c")
            canvas.create_text(20, height-18-len(lines)*17, text="\n".join(lines), anchor="nw",
                               fill="#e7edf8", font=("Consolas", 10))
    overlay.root.update()


def run(session, directory):
    session.user32.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
    session.user32.GetClientRect.restype = ctypes.c_bool
    session.user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(Point)]
    session.user32.ClientToScreen.restype = ctypes.c_bool
    overlay = CandidateOverlay(session)
    dialog_owner = tk.Tk()
    dialog_owner.title("APB component overlay — waiting for F8")
    dialog_owner.geometry("680x180")
    elevated = bool(ctypes.windll.shell32.IsUserAnAdmin())
    tk.Label(dialog_owner, text="Component overlay is ready.\nFocus APB and press F8 to show it.\n"
             "F4 selects a character. F6 cycles readable details. F9 adds a word comment.\n"
             f"Administrator: {'YES' if elevated else 'NO'} | EchoDevice opened\n"
             "F8 stops. F11 exits.", font=("Segoe UI", 12), wraplength=650).pack(pady=12)
    closed = [False]
    dialog_owner.protocol("WM_DELETE_WINDOW", lambda: closed.__setitem__(0, True))
    sequence, annotations, rounds = 0, 0, 0
    current, pressed_before, labels = {}, {}, {}
    pinned, selected = None, None
    active, paused, expanded = False, False, 0
    next_survey = 0
    cycle = 0
    message = "Aim near a character origin and press F4 to select it."
    with (directory / "events.jsonl").open("x", encoding="utf-8", buffering=1) as events:
        def log(state, **record):
            events.write(json.dumps({"utc": utc_now(), "state": state, **record}) + "\n")
            if state != "sample":
                print(f"{state.upper()}: {record.get('message', '')}", flush=True)
        try:
            log("waiting", message="Focus APB. F8 starts live component overlay; F11 exits.")
            while True:
                overlay.root.update()
                keys = {}
                for vk in (0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A):
                    held = down(session, vk)
                    keys[vk] = held and not pressed_before.get(vk, False)
                    pressed_before[vk] = held
                if keys[0x7A] or closed[0]:
                    log("closed", message="F11 ended overlay; evidence retained")
                    return
                if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
                    raise RuntimeError("APB exited during overlay session")
                try:
                    session.require_foreground()
                except ForegroundLost:
                    if overlay.visible:
                        overlay.hide()
                    time.sleep(0.08)
                    continue
                if keys[0x77]:
                    active = not active
                    paused = False
                    if active:
                        next_survey = 0
                        dialog_owner.withdraw()
                    else:
                        dialog_owner.deiconify()
                    log("started" if active else "stopped", message="F8 live overlay " + ("started" if active else "stopped"))
                if not active:
                    if overlay.visible:
                        overlay.hide()
                    time.sleep(0.08)
                    continue
                if keys[0x79]:
                    paused = not paused
                    log("paused" if paused else "resumed", message="F10 toggled recording")
                    next_survey = 0
                if paused:
                    overlay.clear("PAUSED | F10 resumes | F8 stops | F11 exits")
                    time.sleep(0.08)
                    continue
                if keys[0x75]:
                    expanded = (expanded+1) % 3
                try:
                    base = module_base(session)
                    camera_link = locate_camera(session, base, CAMERA_ROOT, 128)
                    camera = camera_at(session, camera_link["camera"])
                    if not camera_shape(camera, base):
                        raise RuntimeError("Camera shape changed")
                    local = resolve_local(session, base, camera_link["camera"])
                    if not local["pawn"]:
                        raise RuntimeError("No local pawn; enter a district")
                    width, height = overlay.place()
                    if time.monotonic() >= next_survey:
                        overlay.clear("REFRESHING pawn candidates through EchoDriver. Keep APB foreground.")
                        current, report = survey(session, base, directory, sequence, include_pawns=True)
                        sequence += 1
                        next_survey = time.monotonic()+10
                        log("survey_complete", message=f"Bounded survey: {report['failed_headers']} failed headers; roster incomplete")
                        # Full surveys are sequential and may take seconds. Refresh
                        # the actual camera before projecting newly read positions.
                        refreshed = locate_camera(session, base, CAMERA_ROOT, 128)
                        if camera_identity(refreshed) != camera_identity(camera_link):
                            raise RuntimeError("Camera changed during candidate survey")
                        camera = camera_at(session, camera_link["camera"])
                        if not camera_shape(camera, base):
                            raise RuntimeError("Camera shape changed after survey")
                        local = resolve_local(session, base, camera_link["camera"])
                    points, rejected = [], []
                    for lead in current.values():
                        if (int(lead["vtable_rva"], 16), lead["marker"]) != (0x2AAEFA0, 5778):
                            continue
                        session.require_foreground()
                        try:
                            xyz, signature, relation = read_position(session, base, lead, direct_pawn=True)
                            point = {"address": lead["address"], "instance_signature": signature,
                                     "structure_relation": relation, "position": xyz}
                            point["ownership"] = owned_position(session, base, point)
                            point["position"] = point["ownership"]["position"]
                            point["projection"] = project(camera, point["position"], width, height)
                            points.append(point)
                        except (OSError, RuntimeError) as error:
                            rejected.append({"address": lead["address"], "error": str(error)})
                    points, excluded, ambiguous = filter_owned(points, local)
                    points.sort(key=lambda p: int(p["address"], 16))
                    for index, point in enumerate(points, 1):
                        point["id"] = index
                        point["operator_observation"] = labels.get((point["address"], point["instance_signature"]), "unknown")
                    on_screen = [p for p in points if p["projection"]["status"] == "inside"]
                    on_screen.sort(key=lambda p: math.hypot(p["projection"]["client_x"]-width/2,
                                                           p["projection"]["client_y"]-height/2))
                    if keys[0x74]:
                        cycle += 1
                    if keys[0x73]:
                        cycle = 0
                    if keys[0x73] or keys[0x74]:
                        candidate = on_screen[cycle % len(on_screen)] if on_screen else None
                        pinned = (candidate["address"], candidate["instance_signature"]) if candidate else None
                        message = f"CURRENT CHARACTER #{candidate['id']} | F9 adds a cursor comment" if candidate else "No on-screen candidate"
                        log("selection", message=message, instance=pinned)
                    selected = next((p for p in points if (p["address"], p["instance_signature"]) == pinned), None)
                    if pinned and selected is None:
                        pinned = None
                        message = "Selected instance unavailable; select again with F4."
                        log("selection_expired", message=message)
                    if selected:
                        selected["visual_components"] = components(session, base, selected, camera, width, height)
                        selected["decoded_fields"] = decoded_fields(session, base, selected, local)
                        for key, label in ((0x70, "visible"), (0x71, "shooting_blocked"),
                                           (0x72, "partial_cursor_visible"), (0x76, "partial_cursor_blocked")):
                            if keys[key]:
                                labels[pinned] = label
                                selected["operator_observation"] = label
                                log("operator_label", message=label, instance=pinned)
                    if (resolve_local(session, base, camera_link["camera"]) != local
                            or camera_identity(locate_camera(session, base, CAMERA_ROOT, 128)) != camera_identity(camera_link)):
                        raise RuntimeError("Camera/local ownership changed; projections withheld")
                    draw(overlay, points, selected, expanded, message, camera)
                    if keys[0x78]:
                        if selected is None:
                            message = "F9 needs a current character: aim near its P marker and press F4."
                        else:
                            frame = directory / f"annotation-{annotations:04d}"
                            annotations += 1
                            frame.mkdir()
                            sample_utc = utc_now()
                            evidence = annotation_evidence(session, base, current[int(selected["address"], 16)], selected, mission=True)
                            image = client_capture(session, frame / "overlay.png", include_layered_windows=True)
                            origin = client_capture(session, frame / "scene.png")
                            overlay.hide()
                            comment = simpledialog.askstring("Comment on highlighted cursor part",
                                "Describe the part at the red centre cursor (head, arm, foot, glass, etc.).\n"
                                "The comment attaches to this selected character and captured screen point.", parent=dialog_owner)
                            write_json(frame / "annotation.json", {"format": "apb-readable-component-overlay-v1",
                                "identity": session.identity, "sample_utc": sample_utc, "annotated_utc": utc_now(),
                                "point": selected, "camera": camera, "local": local, "evidence": evidence,
                                "readable_details": readable(selected), "image": image, "scene_image": origin,
                                "cursor_comment": comment, "cursor_client": [width/2, height/2],
                                "comment_status": "saved" if comment and comment.strip() else "cancelled_or_empty",
                                "source": "operator_annotation_not_verified_bone_or_LOS_mapping"})
                            log("annotation_saved", message=str(frame), comment=comment)
                            message = "Comment saved. Refocus APB; F9 adds another."
                            next_survey = 0
                    rounds += 1
                    log("sample", round=rounds, selected=selected, points=points, rejected=rejected,
                        excluded_local=[p["address"] for p in excluded], ambiguous=ambiguous)
                except ForegroundLost:
                    overlay.hide()
                except (OSError, RuntimeError, ValueError, struct.error) as error:
                    log("capture_unavailable", message=str(error))
                    overlay.clear(f"UNAVAILABLE: {error}\nCorrect the issue; F8 stop/start retries. No alternate source.")
                    paused = True
                time.sleep(0.2)
        finally:
            overlay.close()
            dialog_owner.destroy()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    print(f"Administrator: {'YES' if ctypes.windll.shell32.IsUserAnAdmin() else 'NO'}", flush=True)
    session = Session(args.pid)
    try:
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {"identity": session.identity,
                   "transport": "EchoDriver", "mode": "live_click_through_readable_component_overlay"})
        run(session, args.output_directory)
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except Exception:
        traceback.print_exc(file=sys.stdout)
        raise SystemExit(1)
