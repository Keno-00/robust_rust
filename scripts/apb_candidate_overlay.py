"""Click-through diagnostic points in APB client coordinates, never player labels."""

import ctypes
import tkinter as tk

from apb_f8_common import Point, Rect
from apb_cursor_scan import ForegroundLost


class CandidateOverlay:
    def __init__(self, session):
        self.session = session
        self.root = tk.Tk()
        self.root.withdraw()
        self.root.title("APB unclassified position candidates")
        self.root.overrideredirect(True)
        self.root.attributes("-topmost", True)
        self.root.attributes("-transparentcolor", "black")
        self.canvas = tk.Canvas(self.root, bg="black", highlightthickness=0)
        self.canvas.pack(fill="both", expand=True)
        self.root.update_idletasks()
        u = session.user32
        u.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
        u.GetAncestor.restype = ctypes.c_void_p
        u.GetWindowLongPtrW.argtypes = [ctypes.c_void_p, ctypes.c_int]
        u.GetWindowLongPtrW.restype = ctypes.c_ssize_t
        u.SetWindowLongPtrW.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_ssize_t]
        u.SetWindowLongPtrW.restype = ctypes.c_ssize_t
        u.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int,
                                  ctypes.c_int, ctypes.c_int, ctypes.c_uint]
        u.SetWindowPos.restype = ctypes.c_bool
        self.window = u.GetAncestor(self.root.winfo_id(), 2)
        if not self.window:
            raise RuntimeError("Cannot locate overlay window")
        self.styles = 0x80000 | 0x20 | 0x08000000  # layered, click-through, no activation
        style = u.GetWindowLongPtrW(self.window, -20)
        ctypes.set_last_error(0)
        u.SetWindowLongPtrW(self.window, -20, style | self.styles)
        if ctypes.get_last_error():
            raise ctypes.WinError(ctypes.get_last_error())
        self.visible = False

    def place(self):
        u = self.session.user32
        hwnd = self.session.require_foreground()
        rect = Rect()
        origin = Point()
        if not u.GetClientRect(hwnd, ctypes.byref(rect)) or not u.ClientToScreen(hwnd, ctypes.byref(origin)):
            raise ctypes.WinError(ctypes.get_last_error())
        width, height = rect.right-rect.left, rect.bottom-rect.top
        if width < 320 or height < 240:
            raise RuntimeError("Cannot align overlay to invalid APB client dimensions")
        self.root.geometry(f"{width}x{height}")
        if not self.visible:
            self.root.deiconify()
            self.visible = True
        self.root.update_idletasks()
        if self.session.require_foreground() != hwnd:
            raise ForegroundLost("Foreground APB window changed during overlay placement")
        if not u.SetWindowPos(self.window, ctypes.c_void_p(-1), origin.x, origin.y,
                              width, height, 0x10 | 0x40):
            raise ctypes.WinError(ctypes.get_last_error())
        if self.session.user32.GetWindowLongPtrW(self.window, -20) & self.styles != self.styles:
            raise RuntimeError("Overlay lost its click-through/no-activation window styles")
        self.session.require_foreground()
        return width, height

    def clear(self, message):
        self.place()
        self.canvas.delete("all")
        self.canvas.create_text(18, 175, text=message, anchor="nw", fill="#ffdc60",
                                font=("Consolas", 12, "bold"))
        self.root.update()

    def draw(self, points, manual=None, selected=None):
        width, height = self.place()
        self.canvas.delete("all")
        inside = [point for point in points if point["projection"]["status"] == "inside"
                  and point.get("manual_label") != "exclude"
                  and (not getattr(self, "opposition_only", False)
                       or point.get("mission_classification", {}).get("role") == "opposition")]
        controls = "F9 frame | F8 stop/save | F10 pause | F11 exit"
        if manual:
            controls = "F6 hide | F4 player | F5 next | F7 undo | F9 frame | F8 stop/save | F10 pause | F11 exit"
        if getattr(self, "mission_labels", False):
            controls = "F1 outside | F2 teammate | F3 opposition | F9 frame | F8 stop/save | F10 pause | F11 exit"
        if getattr(self, "mission_classify", False):
            controls = "F1 outside | F2 teammate | F3 opposition | F4 filter | F9 frame | F8 stop | F10 pause | F11 exit"
        self.canvas.create_text(18, 175,
            text=f"{'CURRENT CHARACTER PAWNS' if getattr(self, 'ownership_status', None) else 'EXPERIMENTAL STRUCTURE MATCHES' if getattr(self, 'structure_filter', False) else 'UNCLASSIFIED CANDIDATES'}: {len(inside)} on screen | {controls}\n"
                 + (f"Hidden: {sum(v == 'exclude' for v in manual.labels.values())} | "
                    f"Player labels: {sum(v == 'player' for v in manual.labels.values())} | {manual.message}"
                    if manual else "Cyan: +0x88 / 4741   Yellow: +0xD0 / 294 | NPCs, vehicles, stale objects may appear"),
            anchor="nw", fill="#ffdc60", font=("Consolas", 11, "bold"))
        if getattr(self, "ownership_status", None):
            self.canvas.create_text(18, 211, text=self.ownership_status, anchor="nw",
                                    fill="#ffdc60", font=("Consolas", 11, "bold"))
        if getattr(self, "mission_labels", False):
            self.canvas.create_text(18, 229, text=getattr(self, "mission_context_status",
                "YOUR STATE: unknown | Ctrl+F2 in mission | Ctrl+F3 outside mission"), anchor="nw",
                fill="#ffdc60", font=("Consolas", 11, "bold"))
        if getattr(self, "mission_status", None):
            self.canvas.create_text(18, 247, text=self.mission_status, anchor="nw",
                                    fill="#ffdc60", font=("Consolas", 11, "bold"))
        for point in inside:
            p = point["projection"]
            x, y = p["client_x"], p["client_y"]
            color = "#57e8ff" if point["marker"] in (4741, 5778) else "#ffdc60"
            if point.get("manual_label") == "player":
                color = "#73ff8c"
            color = {"teammate": "#70b5ff", "opposition": "#ff7272", "outside_mission": "#dedede"}.get(point.get("manual_label"), color)
            prediction = point.get("mission_classification")
            if prediction:
                color = {"teammate": "#70b5ff", "opposition": "#ff7272", "outside_mission": "#dedede",
                         "unknown": "#ffdc60"}[prediction["role"]]
            self.canvas.create_oval(x-4, y-4, x+4, y+4, outline=color, width=2)
            self.canvas.create_line(x-8, y, x+8, y, fill=color)
            self.canvas.create_line(x, y-8, x, y+8, fill=color)
            label = point["address"]
            if prediction:
                label = prediction["role"] + " " + label
                ground_truth = point.get("manual_label")
                if ground_truth in ("teammate", "opposition", "outside_mission") and ground_truth != prediction["role"]:
                    label += f" | YOUR LABEL: {ground_truth}"
                    self.canvas.create_oval(x-11, y-11, x+11, y+11, outline="#ff70ef", width=3)
            self.canvas.create_text(x+9, y-9, text=label, anchor="sw", fill=color,
                                    font=("Consolas", 8))
        if manual:
            self.canvas.create_line(width/2-6, height/2, width/2+6, height/2, fill="#ff6868", width=2)
            self.canvas.create_line(width/2, height/2-6, width/2, height/2+6, fill="#ff6868", width=2)
            if selected:
                x, y = selected["projection"]["client_x"], selected["projection"]["client_y"]
                self.canvas.create_oval(x-12, y-12, x+12, y+12, outline="#ff6868", width=3)
                self.canvas.create_text(width/2, height/2+28,
                    text=f"SELECTED {selected['address']} ({manual.selected_index+1}/{len(manual.nearby)})",
                    fill="#ff6868", font=("Consolas", 11, "bold"))
        self.root.update()

    def hide(self):
        self.canvas.delete("all")
        self.root.withdraw()
        self.root.update()
        self.visible = False

    def close(self):
        self.root.destroy()
