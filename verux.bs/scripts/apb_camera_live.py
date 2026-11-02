"""F8-controlled camera inspector; all APB memory reads use EchoDriver."""

import argparse
import ctypes
import json
import math
import struct
import time
import tkinter as tk
from pathlib import Path

from apb_f8_common import Session, camera_at, module_base, utc_now, write_json
from apb_manual_labels import instance_signature


CAMERA_VTABLE_RVA = 0x2B5EEA8
F8 = 0x77
REACQUIRE_SECONDS = 0.5
AUDIT_SECONDS = 2.0


class CameraUnavailable(RuntimeError):
    """A visible camera lifecycle transition, rather than a transport failure."""


def valid_pointer(pointer):
    return 0x10000 <= pointer < 0x800000000000 and pointer % 8 == 0


def camera_shape(camera, base):
    body = bytes.fromhex(camera["body_hex"])
    matrix = camera["matrix"]
    return (struct.unpack_from("<Q", body)[0] == base + CAMERA_VTABLE_RVA
            and abs(matrix[14] + 9.99) <= 0.02 and abs(matrix[15]) <= 0.02)


def camera_identity(candidate):
    return candidate["table"], candidate["index"], candidate["camera"]


def locate_camera(session, base, rva, entry_count, diagnostic=None):
    """Resolve one camera with a stable binding; audit changes in other slots."""
    started = time.monotonic()
    root_slot = base + rva
    header = session.read(root_slot, 16)
    table, adjacent_a, adjacent_b = struct.unpack("<QII", header)
    if not valid_pointer(table):
        raise CameraUnavailable(f"Module camera table is unavailable (0x{table:X})")
    try:
        entries = session.read(table, entry_count * 8)
    except (OSError, RuntimeError) as error:
        raise CameraUnavailable(f"Cannot read module camera table 0x{table:X}: {error}") from error
    found = []
    signatures = {}
    markers = {}
    unreadable = 0
    for index, (pointer,) in enumerate(struct.iter_unpack("<Q", entries)):
        if not valid_pointer(pointer):
            continue
        try:
            (kind,) = struct.unpack("<I", session.read(pointer + 0x3C, 4))
        except (OSError, RuntimeError):
            unreadable += 1
            markers[index] = None
            continue
        markers[index] = kind
        if kind != 16920:
            continue
        try:
            camera = camera_at(session, pointer)
        except (OSError, RuntimeError) as error:
            raise CameraUnavailable(f"Camera at index {index} changed during lookup: {error}") from error
        if not camera_shape(camera, base):
            raise CameraUnavailable(f"Typed camera at table index {index} has an unverified vtable or matrix")
        found.append((index, pointer))
        signatures[index] = instance_signature(bytes.fromhex(camera["body_hex"]))
    root_after = session.read(root_slot, 8)
    table_after = struct.unpack("<Q", root_after)[0]
    changed = []
    audit = {"utc": utc_now(), "started_monotonic_s": started,
             "root_slot": hex(root_slot), "table_before": hex(table), "table_after": hex(table_after),
             "entries_before_hex": entries.hex(), "entries_after_hex": None,
             "changed_slots": changed, "camera_candidates_before": [{"index": i, "address": hex(p),
                 "signature": signatures[i]} for i, p in found]}

    def event(outcome, reason):
        record = {**audit, "completed_monotonic_s": time.monotonic(), "outcome": outcome, "reason": reason}
        if diagnostic is not None:
            diagnostic(record)
        changes = ", ".join(f"{c['index']}: {c['before']} -> {c['after']}" for c in changed) or "none"
        print(f"CAMERA TABLE {outcome.upper()}: {reason}; root {hex(table)} -> {hex(table_after)}; "
              f"changed slots [{changes}]", flush=True)

    if root_after != header[:8]:
        event("rejected", "Module table root changed during lookup")
        raise CameraUnavailable("Camera table root changed during lookup; before/after root logged. Restart selection.")
    try:
        entries_after = session.read(table, entry_count * 8)
    except (OSError, RuntimeError) as error:
        audit["table_read_error"] = str(error)
        event("rejected", "Cannot read table for binding verification")
        raise CameraUnavailable("Cannot reread camera table; inspect camera diagnostics") from error
    audit["entries_after_hex"] = entries_after.hex()
    changed.extend({"index": slot, "before": hex(before), "after": hex(after),
                    "before_marker": markers.get(slot)}
                   for slot, ((before,), (after,)) in enumerate(zip(struct.iter_unpack("<Q", entries),
                                                                  struct.iter_unpack("<Q", entries_after)))
                   if before != after)
    if len(found) != 1:
        event("rejected", f"Expected one typed camera; found {len(found)}")
        raise CameraUnavailable(f"Expected one typed camera in the first {entry_count} table slots; "
                                f"found {len(found)} ({unreadable} unreadable entries)")
    index, pointer = found[0]
    if struct.unpack_from("<Q", entries_after, index*8)[0] != pointer:
        event("rejected", "Selected camera slot changed during lookup")
        raise CameraUnavailable(f"Selected camera table slot {index} changed; exact slot pointers logged. Restart selection.")
    # Changes elsewhere must be classified. A new camera or an unreadable new
    # pointer makes this lookup ambiguous; never accept it by ignoring the slot.
    for change in changed:
        current = int(change["after"], 16)
        change["after_marker"] = None
        if valid_pointer(current):
            try:
                (kind,) = struct.unpack("<I", session.read(current+0x3C, 4))
            except (OSError, RuntimeError) as error:
                change["classification_error"] = str(error)
                event("rejected", "Cannot classify a changed table entry")
                raise CameraUnavailable(f"Cannot classify changed table slot {change['index']}; exact pointers logged") from error
            change["after_marker"] = kind
            if kind == 16920:
                event("rejected", "Another camera-marker entry appeared during lookup")
                raise CameraUnavailable(f"Camera lookup became ambiguous at table slot {change['index']}; restart selection")
        latest = session.read(table+change["index"]*8, 8)
        if latest != entries_after[change["index"]*8:(change["index"]+1)*8]:
            change["latest"] = hex(struct.unpack("<Q", latest)[0])
            event("rejected", "Changed entry changed again during classification")
            raise CameraUnavailable(f"Table slot {change['index']} changed again before its classification was verified; inspect camera diagnostics")
    try:
        camera_after = camera_at(session, pointer)
    except (OSError, RuntimeError) as error:
        audit["camera_read_error"] = str(error)
        event("rejected", "Selected camera became unreadable")
        raise CameraUnavailable("Selected camera became unreadable; inspect camera diagnostics") from error
    audit["camera_signature_after"] = instance_signature(bytes.fromhex(camera_after["body_hex"]))
    if (not camera_shape(camera_after, base)
            or struct.unpack_from("<I", bytes.fromhex(camera_after["body_hex"]), 0x3C)[0] != 16920
            or instance_signature(bytes.fromhex(camera_after["body_hex"])) != signatures[index]):
        event("rejected", "Selected camera type/instance changed during lookup")
        raise CameraUnavailable("Selected camera type or instance changed during lookup; restart selection")
    final_root = session.read(root_slot, 8)
    audit["table_final"] = hex(struct.unpack("<Q", final_root)[0])
    if final_root != header[:8]:
        event("rejected", "Selected camera root changed during final verification")
        raise CameraUnavailable("Selected camera root changed during final verification; restart selection")
    final_pointer = struct.unpack("<Q", session.read(table+index*8, 8))[0]
    audit["camera_slot_final"] = hex(final_pointer)
    if final_pointer != pointer:
        event("rejected", "Selected camera root/slot changed during final verification")
        raise CameraUnavailable("Selected camera root or slot changed during final verification; restart selection")
    if changed:
        event("accepted_non_camera_changes", "Selected camera binding and instance stayed stable; changed entries are non-camera")
    return {"table": table, "index": index, "camera": pointer,
            "adjacent": [adjacent_a, adjacent_b]}


def orientation(matrix):
    forward = [matrix[3], matrix[7], matrix[11]]
    norm = math.sqrt(sum(value * value for value in forward))
    right_horizontal = math.hypot(matrix[0], matrix[4])
    if not 0.98 <= norm <= 1.02 or abs(matrix[11]) > 1.00001:
        raise RuntimeError(f"Invalid forward vector: {forward}, norm={norm:.6f}")
    if right_horizontal < 0.0001 or abs(matrix[8]) > right_horizontal * 0.02:
        raise RuntimeError("Yaw derivation requires a horizontal right vector; inspect saved raw matrix")
    # With the observed no-roll matrix, right = scale * (-sin(yaw), cos(yaw), 0).
    # Unlike forward XY, these components remain usable at vertical pitch.
    yaw = math.degrees(math.atan2(-matrix[0], matrix[4])) % 360
    horizontal = math.hypot(forward[0], forward[1])
    return {
        "pitch_deg": math.degrees(math.asin(max(-1.0, min(1.0, matrix[11])))),
        "yaw_provisional_deg": yaw,
        "yaw_forward_deg": math.degrees(math.atan2(forward[1], forward[0])) % 360
        if horizontal >= 0.05 else None,
        "forward": forward, "forward_norm": norm,
        "right": [matrix[0], matrix[4], matrix[8]],
        "yaw_convention": "0=world +X; 90=world +Y; look-back response observed, absolute heading unverified",
    }


class Inspector:
    def __init__(self, session, source, directory, interval_ms):
        self.session = session
        self.directory = directory
        self.interval_ms = interval_ms
        self.base = module_base(session)
        self.rva = int(source["module_pointer_rva"], 16)
        self.entry_count = source["entry_count"]
        if self.entry_count != 128:
            raise RuntimeError(f"Unverified saved camera table bound: {self.entry_count}")
        self.current = None
        self.pending = None
        self.next_lookup = 0.0
        self.next_audit = 0.0
        self.unavailable_reason = None
        self.stream = (directory / "samples.jsonl").open("x", encoding="utf-8", buffering=1)
        self.root = tk.Tk()
        self.root.title("APB camera | EchoDriver | diagnostic")
        self.root.geometry("700x550")
        self.root.configure(bg="#171b22")
        self.root.attributes("-topmost", True)
        self.status = tk.StringVar(value="WAITING: refocus APB, level the view, then press F8")
        self.values = tk.StringVar(value="No live sample yet")
        self.matrix = tk.StringVar(value="")
        self.details = tk.StringVar(value=f"PID {session.pid} | root APB.exe + 0x{self.rva:X}\n"
                                    f"Scanning first {self.entry_count} table slots on transition")
        for variable, font, color in (
            (self.status, ("Segoe UI", 12, "bold"), "#f3d178"),
            (self.values, ("Consolas", 16), "#e2eaff"),
            (self.details, ("Consolas", 10), "#a9bfdc"),
            (self.matrix, ("Consolas", 11), "#becbdc"),
        ):
            tk.Label(self.root, textvariable=variable, font=font, fg=color,
                     bg="#171b22", anchor="w", justify="left", wraplength=675).pack(
                         fill="x", padx=12, pady=8)
        tk.Label(self.root, text="F8 in APB: start / stop. Alt-tab pauses reads. Close this panel to exit.\n"
                 "Yaw is derived from matrix +0x680; absolute heading remains unverified.\n"
                 "Camera transitions clear values until the module table identifies one stable camera.",
                 fg="#abb4c0", bg="#171b22", justify="left", anchor="w").pack(fill="x", padx=12)
        self.root.protocol("WM_DELETE_WINDOW", self.close)
        session.user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
        session.user32.GetAsyncKeyState.restype = ctypes.c_short
        self.last_key = bool(session.user32.GetAsyncKeyState(F8) & 0x8000)
        self.started = False
        self.finished = False
        self.failure = None
        self.paused = False
        self.first_yaw = None
        self.samples = 0
        self.last_console = 0.0
        self.event("waiting", "Refocus APB and press F8 to start")
        self.root.after(50, self.tick)

    def event(self, state, message, **data):
        record = {"utc": utc_now(), "state": state, "message": message, **data}
        self.stream.write(json.dumps(record, allow_nan=False) + "\n")
        print(f"{record['utc']} {state.upper()}: {message}", flush=True)

    def unavailable(self, reason):
        self.current = None
        self.pending = None
        self.next_lookup = 0.0
        self.first_yaw = None
        if reason != self.unavailable_reason:
            self.event("unavailable", reason, samples=self.samples)
        self.unavailable_reason = reason
        self.status.set(f"UNAVAILABLE: {reason}")
        self.values.set("No current camera sample. Waiting for one stable typed camera.")
        self.details.set(f"PID {self.session.pid} | root APB.exe + 0x{self.rva:X}\n"
                         f"Scanning first {self.entry_count} slots; previous address is invalid")
        self.matrix.set("")

    def reacquire(self):
        now = time.monotonic()
        if now < self.next_lookup:
            return
        self.next_lookup = now + REACQUIRE_SECONDS
        try:
            candidate = locate_camera(self.session, self.base, self.rva, self.entry_count)
        except CameraUnavailable as error:
            reason = str(error)
            if reason != self.unavailable_reason:
                self.event("unavailable", reason, samples=self.samples)
            self.unavailable_reason = reason
            self.pending = None
            self.status.set(f"UNAVAILABLE: {reason}")
            return
        if self.pending is None or camera_identity(candidate) != camera_identity(self.pending):
            self.pending = candidate
            self.unavailable_reason = None
            self.status.set("VERIFYING: one camera found; checking it again before live reads")
            return
        self.current = candidate
        self.pending = None
        self.unavailable_reason = None
        self.next_audit = now + AUDIT_SECONDS
        self.event("acquired", "Stable module-table camera", **candidate)

    def sample(self):
        start = time.perf_counter()
        if self.current is None:
            self.reacquire()
            return
        selected = self.current
        if time.monotonic() >= self.next_audit:
            audited = locate_camera(self.session, self.base, self.rva, self.entry_count)
            if camera_identity(audited) != camera_identity(selected):
                raise CameraUnavailable("Module table selected a different camera during audit")
            self.next_audit = time.monotonic() + AUDIT_SECONDS
        root_slot = self.base + self.rva
        header = self.session.read(root_slot, 16)
        table, adjacent_a, adjacent_b = struct.unpack("<QII", header)
        if table != selected["table"]:
            raise CameraUnavailable(f"Module table changed 0x{selected['table']:X} -> 0x{table:X}")
        slot = table + selected["index"] * 8
        (pointer,) = struct.unpack("<Q", self.session.read(slot, 8))
        if pointer != selected["camera"]:
            raise CameraUnavailable(f"Camera slot changed 0x{selected['camera']:X} -> 0x{pointer:X}")
        try:
            camera = camera_at(self.session, pointer)
        except RuntimeError as error:
            if str(error).startswith("Camera candidate"):
                raise CameraUnavailable(str(error)) from error
            raise
        matrix = camera["matrix"]
        if not camera_shape(camera, self.base):
            raise CameraUnavailable("Camera vtable or matrix no longer has the observed shape")
        # Moving matrices are not atomic, but ownership must survive the sample.
        if self.session.read(root_slot, 8) != header[:8] or self.session.read(slot, 8) != struct.pack("<Q", pointer):
            raise CameraUnavailable("Root or camera slot changed during sample")
        try:
            angles = orientation(matrix)
        except RuntimeError as error:
            self.event("invalid_sample", str(error), matrix=matrix, position=camera["position"])
            raise
        if self.first_yaw is None:
            self.first_yaw = angles["yaw_provisional_deg"]
        angles["yaw_delta_from_start_deg"] = (angles["yaw_provisional_deg"] - self.first_yaw + 180) % 360 - 180
        self.samples += 1
        record = {"utc": utc_now(), "state": "sample", "sequence": self.samples,
                  "table": f"0x{table:X}", "camera": f"0x{pointer:X}", "index": selected["index"],
                  "adjacent_u32_unclassified": [adjacent_a, adjacent_b],
                  "position": camera["position"], "matrix": matrix, "orientation": angles,
                  "read_ms": (time.perf_counter() - start) * 1000}
        self.stream.write(json.dumps(record, allow_nan=False) + "\n")
        yaw_forward = angles["yaw_forward_deg"]
        forward_text = "undefined near vertical" if yaw_forward is None else f"{yaw_forward:8.3f} deg"
        self.values.set(f"Pitch      {angles['pitch_deg']:+9.3f} deg   raw {matrix[11]:+.7f}\n"
                        f"Yaw*       {angles['yaw_provisional_deg']:9.3f} deg   delta {angles['yaw_delta_from_start_deg']:+.3f}\n"
                        f"Yaw (fwd)  {forward_text}\n"
                        f"Position   X {camera['position'][0]:.3f}\n"
                        f"           Y {camera['position'][1]:.3f}   Z {camera['position'][2]:.3f}")
        self.details.set(f"PID {self.session.pid} | root APB.exe + 0x{self.rva:X}\n"
                         f"table 0x{table:X} | index {selected['index']} | camera 0x{pointer:X}\n"
                         f"sample {self.samples} | {record['read_ms']:.2f} ms | {record['utc']}\n"
                         f"neighbor u32 (unclassified): {adjacent_a}, {adjacent_b}")
        self.matrix.set("Matrix +0x680 (raw):\n" + "\n".join(
            " ".join(f"{value:+11.6f}" for value in matrix[row * 4:row * 4 + 4]) for row in range(4)))
        self.status.set("LIVE | Pitch checked; yaw* provisional | F8 stops")
        if time.monotonic() - self.last_console >= 1:
            print(f"LIVE #{self.samples} pitch={angles['pitch_deg']:+.3f} yaw*={angles['yaw_provisional_deg']:.3f} "
                  f"delta_yaw={angles['yaw_delta_from_start_deg']:+.3f} camera=0x{pointer:X}", flush=True)
            self.last_console = time.monotonic()

    def tick(self):
        if self.finished:
            return
        try:
            if self.session.kernel32.WaitForSingleObject(self.session.process, 0) != 258:
                raise RuntimeError("APB exited. Relaunch the inspector for the new process.")
            down = bool(self.session.user32.GetAsyncKeyState(F8) & 0x8000)
            pressed = down and not self.last_key
            self.last_key = down
            try:
                self.session.require_foreground()
            except RuntimeError as error:
                if not self.paused:
                    self.event("paused", str(error))
                    self.paused = True
                self.status.set("PAUSED: APB is not foreground; refocus the game")
                self.values.set("No current sample while paused")
                self.matrix.set("")
                self.root.after(100, self.tick)
                return
            if self.paused:
                self.event("resumed", "APB foreground")
                self.paused = False
            if pressed:
                if self.started:
                    self.finished = True
                    self.event("stopped", "F8 ended live recording", samples=self.samples)
                    self.status.set("STOPPED: F8 ended recording; last sample remains below")
                    return
                self.started = True
                self.event("started", "F8 started live recording")
            if self.started:
                try:
                    self.sample()
                except CameraUnavailable as error:
                    self.unavailable(str(error))
            else:
                self.status.set("WAITING: level the view in APB, then press F8")
            self.root.after(self.interval_ms if self.started else 50, self.tick)
        except (OSError, RuntimeError, ValueError, struct.error) as error:
            self.finished = True
            self.failure = str(error)
            self.event("error", str(error), samples=self.samples)
            self.status.set(f"STOPPED / ERROR: {error}")
            self.values.set("Live values unavailable. See samples.jsonl and terminal log.")
            self.matrix.set("")

    def close(self):
        self.event("closed", "Inspector window closed", samples=self.samples)
        self.root.destroy()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--interval-ms", type=int, default=200)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists() or not 100 <= args.interval_ms <= 2000:
        parser.error("Use a valid PID, a new output directory, and an interval of 100..2000 ms")
    source = json.loads(args.capture.read_text(encoding="utf-8"))
    if source.get("format") != "apb-camera-table-root-f8-v1":
        raise RuntimeError("Source must be a completed F8 module table capture")
    if int(source["module_pointer_rva"], 16) != 0x3DBF110:
        raise RuntimeError("Saved capture does not use the validated APB camera module root")
    before, after = source["camera_entries_before"], source["camera_entries_after"]
    if len(before) != 1 or len(after) != 1 or (
            before[0]["index"], before[0]["pointer"]) != (after[0]["index"], after[0]["pointer"]):
        raise RuntimeError("Saved F8 capture did not hold one stable camera; take a new table check")
    session = Session(args.pid)
    inspector = None
    try:
        if source["identity"]["image_sha256"] != session.identity["image_sha256"]:
            raise RuntimeError("Camera capture belongs to another APB build. Take a new F8 table check.")
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {
            "format": "apb-camera-live-v1", "started_utc": utc_now(), "identity": session.identity,
            "source_capture": str(args.capture), "interval_ms": args.interval_ms,
            "memory_reader": "EchoDriver v3 / EchoDevice / AES-256-GCM / IOCTL 0x226002",
            "yaw_validation": "look-back response observed; absolute heading unverified",
            "scope": "module-rooted unique typed camera; visible transition reacquisition within first 128 slots",
        })
        print("Live camera inspector: EchoDriver read-only. Refocus APB, level view, F8 starts; F8 stops.", flush=True)
        print(f"Raw samples and events: {args.output_directory / 'samples.jsonl'}", flush=True)
        inspector = Inspector(session, source, args.output_directory, args.interval_ms)
        inspector.root.mainloop()
        if inspector.failure is not None:
            raise RuntimeError(inspector.failure)
    finally:
        if inspector is not None:
            inspector.stream.close()
        session.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("Live camera inspector interrupted.", flush=True)
    except (OSError, RuntimeError, ValueError, KeyError, struct.error, tk.TclError) as error:
        raise SystemExit(f"Live camera inspector failed: {error}")
