"""UI for the raw visibility recorder, using the existing layered overlay."""

import ctypes
import math
import struct
import time
import tkinter as tk

from apb_candidate_overlay import CandidateOverlay
from apb_cursor_scan import ForegroundLost
from apb_f8_common import Point, Rect, camera_at, project
from apb_manual_labels import instance_signature


class TraceOverlay:
    def __init__(self, session):
        self.session = session
        session.user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
        session.user32.GetAsyncKeyState.restype = ctypes.c_short
        session.user32.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
        session.user32.GetClientRect.restype = ctypes.c_bool
        session.user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(Point)]
        session.user32.ClientToScreen.restype = ctypes.c_bool
        self.overlay = CandidateOverlay(session)
        self.held = set()
        self.closed = False
        self.panel = tk.Toplevel(self.overlay.root)
        self.panel.title("APB visibility discovery")
        self.panel.geometry("720x230")
        self.message = tk.StringVar(value="Focus APB. F8 starts witness selection.\n"
            "F5 cycles targets; F6 confirms. F8 confirms each requested scene.\n"
            "Hold until CAPTURE COMPLETE. F9 discards a capture; F11 exits.\n"
            f"Administrator: {'YES' if ctypes.windll.shell32.IsUserAnAdmin() else 'NO'}")
        tk.Label(self.panel, textvariable=self.message, font=("Segoe UI", 12),
                 justify="left", wraplength=690).pack(padx=12, pady=12)
        self.panel.protocol("WM_DELETE_WINDOW", lambda: setattr(self, "closed", True))
        self.overlay.root.update()

    def keys(self):
        self.overlay.root.update()
        if self.session.kernel32.WaitForSingleObject(self.session.process, 0) != 258:
            raise RuntimeError("APB exited")
        if self.closed:
            raise KeyboardInterrupt("Trace UI closed")
        try:
            self.session.require_foreground()
        except ForegroundLost:
            self.held = {key for key in range(0x70, 0x7B)
                         if self.session.user32.GetAsyncKeyState(key) & 0x8000}
            if self.overlay.visible:
                self.overlay.hide()
            return None
        down = {key for key in range(0x70, 0x7B)
                if self.session.user32.GetAsyncKeyState(key) & 0x8000}
        edges = down-self.held
        self.held = down
        return edges

    def draw(self, message, camera=None, points=(), selected=None):
        width, height = self.overlay.place()
        canvas = self.overlay.canvas
        canvas.delete("all")
        canvas.create_rectangle(10, 12, min(width-10, 790), 150, fill="#18232c", outline="#62def0")
        canvas.create_text(22, 22, text=message, anchor="nw", fill="#ffffff",
                           font=("Consolas", 12, "bold"), width=min(width-44, 745))
        canvas.create_line(width/2-7, height/2, width/2+7, height/2, fill="#ff7777", width=2)
        canvas.create_line(width/2, height/2-7, width/2, height/2+7, fill="#ff7777", width=2)
        if camera:
            for point in points:
                projection = project(camera, point["position"], width, height)
                if projection["status"] != "inside":
                    continue
                x, y = projection["client_x"], projection["client_y"]
                color = "#77ff99" if point["id"] == selected else "#ffdc60"
                canvas.create_oval(x-9, y-9, x+9, y+9, outline=color, width=3)
                canvas.create_text(x+12, y, text=f"Pawn #{point['id']} origin", anchor="w",
                                   fill=color, font=("Consolas", 11, "bold"))
        self.overlay.root.update()

    def wait(self, message):
        self.message.set(message)
        while True:
            edges = self.keys()
            if edges is not None:
                if 0x7A in edges:
                    raise KeyboardInterrupt("F11 ended session")
                self.panel.withdraw()
                self.draw(message)
                if 0x77 in edges:
                    return
            time.sleep(0.02)

    def select(self, source):
        points = source["points"]
        if not points:
            raise RuntimeError("Selection survey found no eligible pawns")
        index = 0
        camera_address = int(source["camera"]["address"], 16)
        camera_signature = instance_signature(self.session.read(camera_address, 0x40))
        while True:
            edges = self.keys()
            if edges is None:
                time.sleep(0.03)
                continue
            if 0x7A in edges:
                raise KeyboardInterrupt("F11 ended selection")
            if 0x74 in edges:
                index = (index+1) % len(points)
            camera = camera_at(self.session, camera_address)
            if instance_signature(self.session.read(camera_address, 0x40)) != camera_signature:
                raise RuntimeError("Camera instance changed during selection")
            for point in points:
                body = self.session.read(int(point["address"], 16), 0x100)
                if instance_signature(body) != point["instance_signature"]:
                    raise RuntimeError("Selection candidate changed instance; restart selection")
                xyz = struct.unpack_from("<fff", body, 0x88)
                if not all(math.isfinite(v) and abs(v) < 1_000_000 for v in xyz):
                    raise RuntimeError("Invalid target position during selection")
                point["position"] = xyz
            chosen = points[index]
            self.draw(f"SELECT WITNESS: Pawn #{chosen['id']} (green)\n"
                      "F5 next pawn | F6 confirm witness | F11 exit\n"
                      "Markers are pawn origins; verify the person independently.\n"
                      "No automatic LOS or bones.", camera, points, chosen["id"])
            if 0x75 in edges:
                return chosen
            time.sleep(0.07)

    def recording(self, point, camera, phase, rounds, remaining, hz, missed, gap, structure=None):
        raw_line = (f"\nRAW +D1C {structure['pointer']} | count {structure['count']} | cap {structure['capacity']}"
                    + (" | invalid layout" if not structure['layout_plausible'] else "") if structure is not None else "")
        self.draw(f"CAPTURING: {phase['name']} | Pawn #{point['id']}\n"
                  f"F8 LABEL: {phase['observation'].upper()} | {max(0, remaining):.1f}s left\n"
                  f"{rounds} samples | {hz:.1f} Hz | gaps {gap:.3f}s | missed {missed}\n"
                  "Hold scene. F9 discard | F11 exit" + raw_line,
                  camera, [point], point['id'])

    def close(self):
        self.overlay.close()
