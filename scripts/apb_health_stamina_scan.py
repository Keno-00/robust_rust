"""F8-paced, read-only health/stamina discovery for an operator-selected laptop pawn.

Uses EchoDriver exclusively for APB memory. Pins one instance, preserves raw
bodies and control samples, and never substitutes another pawn after respawn.
"""

import argparse
import ctypes
import hashlib
import json
import math
import struct
import time
import traceback
from pathlib import Path

from apb_camera_live import locate_camera, valid_pointer, camera_shape
from apb_candidate_overlay import CandidateOverlay
from apb_character_ownership import (resolve_local, typed_header, pointer_field,
                                     WORLD_TYPE, PAWN_TYPE, VEHICLE_TYPE)
from apb_cursor_scan import ForegroundLost
from apb_f8_common import Session, module_base, camera_at, project, client_capture, utc_now, write_json, Rect, Point
from apb_health_stamina_review import PHASES, review
from apb_manual_labels import instance_signature, ManualLabels

CAMERA_ROOT = 0x3DBF110
PAWN_EXTENT = 0x3000
LINK_EXTENT = 0x1000
MAX_NODES = 512
MAX_LINKS = 64
F5, F8, F11 = 0x74, 0x77, 0x7A
ECHO_MAX_READ = 4096


class BodyReadError(RuntimeError):
    """A failed bounded EchoDriver body read, with request diagnostics."""


def kind(body, base):
    return struct.unpack_from("<Q", body)[0] - base, struct.unpack_from("<I", body, 0x3C)[0]


def down(session, key):
    return bool(session.user32.GetAsyncKeyState(key) & 0x8000)


def read_body(session, address, extent):
    """Honor EchoDriver's 1–4096-byte request contract for every body read."""
    chunks = []
    for offset in range(0, extent, ECHO_MAX_READ):
        size = min(ECHO_MAX_READ, extent - offset)
        try:
            chunks.append(session.read(address + offset, size))
        except (OSError, RuntimeError) as error:
            raise BodyReadError(f"EchoDriver body read failed at 0x{address + offset:X}, "
                               f"request={size} bytes, body={extent} bytes: {error}") from error
    return b"".join(chunks)


def root_context(session, base):
    camera = locate_camera(session, base, CAMERA_ROOT, 128)
    local = resolve_local(session, base, camera["camera"])
    if not local["pawn"]:
        raise RuntimeError("Local character is unavailable. Load into the laptop's district, on foot, then start a NEW scan.")
    local["world_signature"] = typed_header(session, base, int(local["world"], 16), WORLD_TYPE)
    return camera, local


def roster(session, base, world, stamp):
    if typed_header(session, base, world, WORLD_TYPE) != stamp:
        raise RuntimeError("World changed before target selection. Restart this scan in the current district.")
    head = pointer_field(session, world, 0x48C)
    node = head
    seen = set()
    edges = []
    pawns = []
    while node:
        if len(seen) >= MAX_NODES or not valid_pointer(node) or node in seen:
            raise RuntimeError(f"Invalid/cyclic or oversized rooted pawn list at 0x{node:X}. Capture the list layout; partial selection withheld.")
        seen.add(node)
        body = session.read(node, 0x358)
        node_kind = kind(body, base)
        if node_kind not in (PAWN_TYPE, VEHICLE_TYPE):
            raise RuntimeError(f"Unsupported rooted pawn type {node_kind} at 0x{node:X}. Inspect this type before using the scan.")
        if struct.unpack_from("<Q", body, 0x2E0)[0] != world:
            raise RuntimeError("Pawn-list World changed during traversal. Restart the scan after the district settles.")
        next_node = struct.unpack_from("<Q", body, 0x350)[0]
        fingerprint = instance_signature(body)
        edges.append((node, fingerprint, node_kind, next_node))
        if node_kind == PAWN_TYPE:
            xyz = struct.unpack_from("<fff", body, 0x88)
            if not all(math.isfinite(v) and abs(v) < 1e6 for v in xyz):
                raise RuntimeError(f"Invalid character coordinates at 0x{node:X}")
            pawns.append({"address": hex(node), "signature": fingerprint, "position": xyz})
        node = next_node
    for address, fingerprint, node_kind, next_node in edges:
        body = session.read(address, 0x358)
        if (kind(body, base) != node_kind or instance_signature(body) != fingerprint
                or struct.unpack_from("<Q", body, 0x2E0)[0] != world
                or struct.unpack_from("<Q", body, 0x350)[0] != next_node):
            raise RuntimeError("Rooted pawn list changed during target selection. Restart after the district settles.")
    if typed_header(session, base, world, WORLD_TYPE) != stamp or pointer_field(session, world, 0x48C) != head:
        raise RuntimeError("World/head changed during target selection")
    return pawns


class ScanOverlay(CandidateOverlay):
    def __init__(self, session):
        session.user32.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
        session.user32.GetClientRect.restype = ctypes.c_bool
        session.user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(Point)]
        session.user32.ClientToScreen.restype = ctypes.c_bool
        super().__init__(session)

    def draw_scan(self, points, selected, message):
        width, height = self.place()
        self.canvas.delete("all")
        self.canvas.create_text(18, 175, text=message, anchor="nw", fill="#ffdc60",
                                font=("Consolas", 11, "bold"), width=max(300, width - 36))
        self.canvas.create_oval(width / 2 - 24, height / 2 - 24, width / 2 + 24,
                                height / 2 + 24, outline="#ff7272")
        for point in points:
            p = point["projection"]
            if p["status"] != "inside":
                continue
            x, y = p["client_x"], p["client_y"]
            chosen = selected and point["address"] == selected["address"]
            color = "#73ff8c" if chosen else "#57e8ff"
            radius = 10 if chosen else 4
            self.canvas.create_oval(x - radius, y - radius, x + radius, y + radius,
                                    outline=color, width=2)
            self.canvas.create_text(x + 12, y - 10, text=point["address"], anchor="sw", fill=color,
                                    font=("Consolas", 9, "bold"))
        self.root.update()


class Scanner:
    def __init__(self, session, directory):
        self.session = session
        self.directory = directory
        self.base = module_base(session)
        pe = struct.unpack("<I", session.read(self.base + 0x3C, 4))[0]
        if pe > 0x1000 or session.read(self.base + pe, 4) != b"PE\0\0":
            raise RuntimeError("Cannot validate the APB PE image bounds")
        self.image_size = struct.unpack("<I", session.read(self.base + pe + 0x50, 4))[0]
        if not 0x1000 <= self.image_size <= 0x80000000:
            raise RuntimeError("APB image size is invalid")
        self.camera = None
        self.local = None
        self.overlay = ScanOverlay(session)
        self.target = None
        self.previous = {key: down(session, key) for key in (F5, F8, F11)}

    def event(self, name, **fields):
        row = {"utc": utc_now(), "event": name, **fields}
        with (self.directory / "events.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(row) + "\n")
        print(f"{name.upper()}: {fields}", flush=True)

    def keys(self):
        edges = set()
        for key in self.previous:
            pressed = down(self.session, key)
            if pressed and not self.previous[key]:
                edges.add(key)
            self.previous[key] = pressed
        if F11 in edges:
            raise RuntimeError("Operator stopped with F11; completed phases preserved. Use a new output directory to restart.")
        return edges

    def context(self):
        camera, local = root_context(self.session, self.base)
        if any(local[field] != self.local[field] for field in ("world", "world_signature", "pawn", "pawn_signature", "controller")):
            raise RuntimeError("Local pawn/World changed. Do not mix respawns or districts; start a NEW scan.")
        self.camera = camera
        return local

    def arm_f8(self, phase):
        """Observe key release after the prior action; never queue busy presses."""
        if down(self.session, F8):
            self.event("waiting_f8_release", phase=phase,
                       instruction="Release F8 to arm this phase; presses during capture are discarded.")
        while down(self.session, F8):
            self.keys()
            time.sleep(0.05)
        self.previous[F8] = False

    def projection(self, pawn):
        frame = camera_at(self.session, self.camera["camera"])
        if not camera_shape(frame, self.base):
            raise RuntimeError("Camera projection layout changed")
        width, height = self.overlay.place()
        return {**pawn, "instance_signature": pawn["signature"],
                "projection": project(frame, pawn["position"], width, height)}

    def select(self):
        manual = ManualLabels()
        self.event("start_ready", instruction="Load desktop and laptop characters into the SAME DISTRICT, on foot. Focus desktop APB and press F8 to BEGIN target selection. F11 exits.")
        while True:
            edges = self.keys()
            if F8 in edges:
                try:
                    self.session.require_foreground()
                except ForegroundLost:
                    self.event("start_ignored", reason="Desktop APB must be foreground before F8")
                else:
                    self.camera, self.local = root_context(self.session, self.base)
                    break
            time.sleep(0.05)
        self.event("selection_ready", instruction="Aim desktop reticle at the laptop's CYAN body-center marker. F5 cycles overlapping markers. F8 pins it. F11 exits.")
        while True:
            edges = self.keys()
            try:
                self.session.require_foreground()
            except ForegroundLost:
                self.overlay.hide()
                time.sleep(0.1)
                continue
            self.context()
            pawns = roster(self.session, self.base, int(self.local["world"], 16), self.local["world_signature"])
            points = [self.projection(pawn) for pawn in pawns if pawn["address"] != self.local["pawn"]]
            width, height = self.overlay.place()
            selected = manual.select(points, width, height)
            if F5 in edges:
                manual.cycle()
                selected = manual.select(points, width, height)
            self.overlay.draw_scan(points, selected,
                "SELECT LAPTOP PLAYER: align reticle with its cyan body-center marker.\n"
                "F5 cycles nearby markers | F8 pins the GREEN marker | F11 exits\n"
                "Markers are character instances; YOU verify which is your laptop group member.")
            if F8 in edges:
                if selected is None:
                    self.event("selection_rejected", reason="No marker within 24 pixels of the reticle; aim at the laptop marker and press F8 again.")
                else:
                    self.target = {"address": selected["address"], "signature": selected["signature"],
                                   "operator_identity": "laptop-loaded player in operator's group",
                                   "identity_status": "operator_selected_instance_not_account_identity"}
                    self.target_body()
                    write_json(self.directory / "selection.json", {"target": self.target,
                        "identity": self.session.identity, "local": self.local,
                        "image": client_capture(self.session, self.directory / "selection.png", True)})
                    self.event("target_pinned", target=self.target)
                    return
            time.sleep(0.1)

    def target_body(self, extent=0x300):
        address = int(self.target["address"], 16)
        if typed_header(self.session, self.base, address, PAWN_TYPE) != self.target["signature"]:
            raise RuntimeError("Laptop pawn instance changed/respawned. Start a NEW scan and reselect; no replacement was used.")
        body = read_body(self.session, address, extent)
        if instance_signature(body) != self.target["signature"] or struct.unpack_from("<Q", body, 0x2E0)[0] != int(self.local["world"], 16):
            raise RuntimeError("Pinned laptop pawn or its World changed during read")
        component = struct.unpack_from("<Q", body, 0x248)[0]
        stamp = typed_header(self.session, self.base, component, (0x3197448, 3269))
        if pointer_field(self.session, component, 0x44) != address or pointer_field(self.session, component, 0x7C) != address:
            raise RuntimeError("Laptop collision component does not point back to the selected pawn")
        if (pointer_field(self.session, address, 0x248) != component
                or typed_header(self.session, self.base, component, (0x3197448, 3269)) != stamp
                or typed_header(self.session, self.base, address, PAWN_TYPE) != self.target["signature"]
                or pointer_field(self.session, address, 0x2E0) != int(self.local["world"], 16)):
            raise RuntimeError("Laptop pawn/component instance changed during sample")
        return body

    def wait_phase(self, index, label, instruction):
        self.arm_f8(label)
        self.event("phase_ready", phase=label, instruction=instruction)
        focused = True
        while True:
            edges = self.keys()
            try:
                self.session.require_foreground()
            except ForegroundLost:
                self.overlay.hide()
                if focused:
                    self.event("focus_lost", phase=label, action="F8 ignored until desktop APB is foreground")
                focused = False
                time.sleep(0.1)
                continue
            if not focused:
                self.event("focus_restored", phase=label)
            focused = True
            self.context()
            body = self.target_body()
            point = self.projection({**self.target, "position": struct.unpack_from("<fff", body, 0x88)})
            self.overlay.draw_scan([point], point,
                f"LAPTOP HEALTH/STAMINA {index + 1}/{len(PHASES)}: {label}\n{instruction}\n"
                "F8 CAPTURES NOW | F11 exits | Check the LAPTOP HUD, not desktop HUD.\n"
                "Baseline F8 also confirms the green marker belongs to your laptop player.")
            if F8 in edges:
                return
            time.sleep(0.1)

    def save_body(self, directory, label, address, body):
        filename = label.replace("/", "-").replace("+", "_") + ".bin"
        path = directory / filename
        with path.open("xb") as stream:
            stream.write(body)
        node_kind = kind(body, self.base)
        return {"path": str(path.relative_to(self.directory)), "address": hex(address),
                "signature": instance_signature(body), "vtable_rva": hex(node_kind[0]),
                "marker": node_kind[1], "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest(),
                "extent_status": "bounded_probe_not_verified_object_size", "read_complete_utc": utc_now()}

    def related(self, subject, pawn, body, directory, objects, failures):
        fields = [(f"field+{offset:X}", offset, struct.unpack_from("<Q", body, offset)[0])
                  for offset in range(0x40, 0x1000 - 7, 4)]
        array, count, capacity = struct.unpack_from("<Qii", body, 0x140)
        if not 0 <= count <= capacity <= 128 or (count and not valid_pointer(array)):
            raise RuntimeError("Pawn registered-component array has invalid bounds")
        entries = self.session.read(array, count * 8) if count else b""
        fields += [(f"component-{i}", None, pointer) for i, (pointer,) in enumerate(struct.iter_unpack("<Q", entries))]
        accepted = {}
        rejected = {}
        for name, offset, address in fields:
            if not valid_pointer(address) or address == pawn:
                continue
            if address in rejected:
                continue
            if address not in accepted:
                try:
                    header = self.session.read(address, 0x40)
                    if not self.base <= struct.unpack_from("<Q", header)[0] < self.base + self.image_size:
                        rejected[address] = "not a module-vtable object"
                        continue
                    if len(accepted) >= MAX_LINKS:
                        raise RuntimeError(f"Related-object bound {MAX_LINKS} exceeded; narrow the capture explicitly")
                    linked = read_body(self.session, address, LINK_EXTENT)
                    if instance_signature(linked) != instance_signature(header) or self.session.read(address, 0x40)[0x24:0x40] != linked[0x24:0x40]:
                        raise RuntimeError("Speculative linked object changed during read")
                    accepted[address] = linked
                except (OSError, BodyReadError) as error:
                    rejected[address] = str(error)
                    failures.append({"subject": subject, "field": name, "address": hex(address), "error": str(error)})
                    continue
            if offset is not None and pointer_field(self.session, pawn, offset) != address:
                failures.append({"subject": subject, "field": name, "address": hex(address), "error": "pointer changed during capture; object excluded"})
                continue
            label = f"{subject}/{name}"
            objects[label] = self.save_body(directory, label, address, accepted[address])
        if self.session.read(pawn + 0x140, 16) != body[0x140:0x150] or (count and self.session.read(array, count * 8) != entries):
            raise RuntimeError("Registered component array changed during capture; repeat in a new scan")

    def sample(self, directory, sequence):
        directory.mkdir()
        self.session.require_foreground()
        local = self.context()
        started = utc_now()
        # The target raw body is read first, before exploratory linked reads.
        target = self.target_body(PAWN_EXTENT)
        local_address = int(local["pawn"], 16)
        control = read_body(self.session, local_address, PAWN_EXTENT)
        if (kind(control, self.base) != PAWN_TYPE or instance_signature(control) != self.local["pawn_signature"]
                or struct.unpack_from("<Q", control, 0x2E0)[0] != int(self.local["world"], 16)):
            raise RuntimeError("Local control pawn changed during capture")
        objects = {"laptop/pawn": self.save_body(directory, "laptop/pawn", int(self.target["address"], 16), target),
                   "local/pawn": self.save_body(directory, "local/pawn", local_address, control)}
        failures = []
        self.related("laptop", int(self.target["address"], 16), target, directory, objects, failures)
        self.related("local", local_address, control, directory, objects, failures)
        self.target_body()
        self.context()
        self.session.require_foreground()
        return {"sequence": sequence, "started_utc": started, "completed_utc": utc_now(),
                "objects": objects, "probe_failures": failures,
                "scope": "two pawn bodies and bounded direct typed-object links; speculative link failures explicit"}

    def run(self):
        self.select()
        write_json(self.directory / "session.json", {"identity": self.session.identity,
            "module_base": hex(self.base), "target": self.target, "local_control": self.local,
            "phases": [{"name": name, "instruction": text} for name, text in PHASES],
            "memory_reader": "EchoDriver Session.read", "burst_samples": 3,
            "max_memory_request_bytes": ECHO_MAX_READ,
            "status": "operator_paced_remote_health_stamina_discovery"})
        completed = []
        for index, (label, instruction) in enumerate(PHASES):
            self.wait_phase(index, label, instruction)
            self.overlay.clear(f"CAPTURING {label}\nF8 presses during capture are ignored.\n"
                               "Wait for CAPTURE_COMPLETE and the next phase instruction.")
            self.event("capture_started", phase=label, action="F8 input discarded while busy")
            directory = self.directory / label
            directory.mkdir()
            report = {"identity": self.session.identity, "phase": label,
                      "operator_instruction": instruction, "operator_state_evidence": "F8 confirms laptop HUD state",
                      "capture_start_utc": utc_now(), "samples": [], "complete": False}
            for sequence in range(3):
                self.keys()  # Observe/discard busy F8 edges; honor F11 between samples.
                report["samples"].append(self.sample(directory / f"sample-{sequence:02d}", sequence))
                if sequence < 2:
                    time.sleep(0.04)
            report["image"] = client_capture(self.session, directory / "desktop-frame.png", True)
            report["capture_end_utc"] = utc_now()
            report["complete"] = True
            write_json(directory / "stage.json", report)
            completed.append(label)
            failures = sum(len(s["probe_failures"]) for s in report["samples"])
            self.event("capture_complete", phase=label, speculative_link_read_failures=failures,
                       instruction="Completed files retained. Prepare the next laptop state; no countdown.")
        result = review(self.directory)
        write_json(self.directory / "candidates.json", result)
        write_json(self.directory / "summary.json", {"identity": self.session.identity,
            "target": self.target, "phases_completed": completed,
            "candidates": len(result["candidates"]), "status": "capture_complete_candidates_unverified"})
        self.event("scan_complete", candidates=len(result["candidates"]), output=str(self.directory))
        if not result["candidates"]:
            self.event("no_scalar_match", reason="Inconclusive: check laptop HUD timing, replication and indirect/encoded fields. Raw captures remain saved; no health/stamina offset was established.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--verify-read-from", type=Path,
                        help="Read-only transport check of both pinned pawns from a same-process session.json; no F8 phases")
    args = parser.parse_args()
    args.output_directory.mkdir(parents=False, exist_ok=False)
    scanner = None
    session = None
    try:
        session = Session(args.pid)
        session.user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
        session.user32.GetAsyncKeyState.restype = ctypes.c_short
        if args.verify_read_from:
            source = json.loads(args.verify_read_from.read_text(encoding="utf-8"))
            if source["identity"] != session.identity or module_base(session) != int(source["module_base"], 16):
                raise RuntimeError("Read verification requires the exact saved APB process and image base")
            base = int(source["module_base"], 16)
            results = []
            for name, address, stamp in (
                    ("laptop", int(source["target"]["address"], 16), source["target"]["signature"]),
                    ("local", int(source["local_control"]["pawn"], 16), source["local_control"]["pawn_signature"])):
                if typed_header(session, base, address, PAWN_TYPE) != stamp:
                    raise RuntimeError(f"{name} pawn changed before read verification")
                body = read_body(session, address, PAWN_EXTENT)
                if (instance_signature(body) != stamp or typed_header(session, base, address, PAWN_TYPE) != stamp
                        or struct.unpack_from("<Q", body, 0x2E0)[0] != int(source["local_control"]["world"], 16)):
                    raise RuntimeError(f"{name} pawn/World changed during read verification")
                results.append({"subject": name, "address": hex(address), "bytes": len(body),
                                "requests": 3, "max_request_bytes": ECHO_MAX_READ,
                                "sha256": hashlib.sha256(body).hexdigest()})
            write_json(args.output_directory / "read-verification.json", {"identity": session.identity,
                "source": str(args.verify_read_from), "results": results, "status": "bounded_reads_succeeded"})
            print("READ_VERIFICATION_COMPLETE: both pinned 12288-byte pawn bodies read through three 4096-byte EchoDriver requests each.", flush=True)
            return
        scanner = Scanner(session, args.output_directory)
        scanner.run()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error, KeyboardInterrupt) as error:
        message = f"Health/stamina scan failed: {error}"
        write_json(args.output_directory / "failure.json", {"utc": utc_now(), "error": message,
            "traceback": traceback.format_exc(),
            "action": "Inspect the terminal log and completed phase files. Resolve the cause; restart with a NEW output directory."})
        print(message, flush=True)
        raise SystemExit(1)
    finally:
        if scanner:
            scanner.overlay.close()
        if session:
            session.close()


if __name__ == "__main__":
    main()
