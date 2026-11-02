"""F8 snapshots with explicit per-frame player/body-part LOS ground truth.

Every target-memory read uses the existing EchoDriver Session. No inferred LOS
or skeleton indices are supplied by this diagnostic tool.
"""

import argparse
import ctypes
import json
import sys
import traceback
import struct
import math
import tkinter as tk
from tkinter import ttk, messagebox, simpledialog
from pathlib import Path

from PIL import Image, ImageDraw, ImageTk

from apb_camera_live import locate_camera, camera_identity, camera_shape
from apb_character_ownership import resolve_local, owned_position, filter_owned
from apb_cursor_scan import ForegroundLost
from apb_district_candidates import CAMERA_ROOT, survey, read_position
from apb_f8_common import Point, Session, module_base, camera_at, client_capture, project, utc_now, write_json
from apb_manual_labels import annotation_evidence
from apb_visibility_roster import capture_roster


STATES = ("unknown", "visible", "obstructed", "partial")
PARTS = ("head", "left_arm", "right_arm", "left_leg", "right_leg", "torso")
ROLES = ("unknown", "opposition", "teammate", "outside_mission", "not_player")
OBSERVATIONS = ("unknown", "visible", "invisible", "partial_cursor_visible", "partial_cursor_invisible")


def capture(session, directory, sequence, camera_diagnostic=None, pawn_source="module-window"):
    session.require_foreground()
    started = utc_now()
    base = module_base(session)
    selected = locate_camera(session, base, CAMERA_ROOT, 128, camera_diagnostic)
    camera = camera_at(session, selected["camera"])
    if not camera_shape(camera, base):
        raise RuntimeError("Camera shape changed; no annotations accepted")
    local = resolve_local(session, base, selected["camera"])
    if not local["pawn"]:
        raise RuntimeError("No current local pawn; enter a district before capturing")
    before = client_capture(session, directory / "before.png")
    cursor = Point()
    session.user32.GetCursorPos.argtypes = [ctypes.POINTER(Point)]
    session.user32.GetCursorPos.restype = ctypes.c_bool
    if not session.user32.GetCursorPos(ctypes.byref(cursor)):
        raise ctypes.WinError(ctypes.get_last_error())
    cursor_context = {"sampled_utc": utc_now(), "screen": [cursor.x, cursor.y],
                      "client": [cursor.x-before["screen_left"], cursor.y-before["screen_top"]],
                      "client_center": [before["client_width"]/2, before["client_height"]/2],
                      "meaning": "OS cursor and client centre recorded separately; actual aim reticle unverified"}
    leads, report = survey(session, base, directory, sequence, include_pawns=True)
    pawn_roster = None
    if pawn_source == "world-roster":
        rooted, pawn_roster = capture_roster(session, base, int(local['world'], 16), directory, sequence)
        leads = {int(lead['address'], 16): lead for lead in rooted}
        print(f"WORLD PAWN LIST: {pawn_roster['characters']} characters / {pawn_roster['nodes']} nodes. "
              "Module survey retained for scope bounds; selection uses the rooted list.", flush=True)
    elif pawn_source != "module-window":
        raise RuntimeError(f"Unknown explicit pawn selection source: {pawn_source}")
    points, rejected = [], []
    for lead in leads.values():
        if (int(lead["vtable_rva"], 16), lead["marker"]) != (0x2AAEFA0, 5778):
            continue
        session.require_foreground()
        try:
            xyz, signature, relation = read_position(session, base, lead, direct_pawn=True)
            point = {"address": lead["address"], "position": xyz,
                     "instance_signature": signature, "structure_relation": relation,
                     "discovery_source": pawn_source}
            if pawn_source == 'world-roster' and signature != lead['instance_signature']:
                raise RuntimeError('Rooted pawn instance changed before ownership validation')
            point["ownership"] = owned_position(session, base, point)
            point["position"] = point["ownership"]["position"]
            point["projection"] = project(camera, point["position"],
                                          before["client_width"], before["client_height"])
            point["evidence"] = annotation_evidence(session, base, lead, point, mission=True)
            points.append(point)
        except (OSError, RuntimeError) as error:
            rejected.append({"address": lead["address"], "error": str(error)})
    points, excluded, ambiguous = filter_owned(points, local)
    if (camera_identity(locate_camera(session, base, CAMERA_ROOT, 128, camera_diagnostic)) != camera_identity(selected)
            or resolve_local(session, base, selected["camera"]) != local):
        raise RuntimeError("Camera/local ownership changed; retry a stationary capture")
    camera_after = camera_at(session, selected["camera"])
    after = client_capture(session, directory / "after.png")
    if (before["client_width"], before["client_height"]) != (after["client_width"], after["client_height"]):
        raise RuntimeError("Client dimensions changed; retry capture")
    for index, point in enumerate(points, 1):
        point["id"] = index
    result = {"format": "apb-visibility-annotation-f8-v1", "started_utc": started,
              "completed_utc": utc_now(), "identity": session.identity,
              "module_base": hex(base), "camera": camera, "camera_after": camera_after,
              "local": local, "points": points, "rejected": rejected,
              "excluded_local": [p["address"] for p in excluded], "ambiguous": ambiguous,
              "survey": report, "images": {"before": before, "after": after},
              "pawn_source": pawn_source, "pawn_roster": pawn_roster,
              "cursor_context": cursor_context,
              "status": "awaiting_operator_annotations",
              "limits": "Sequential reads/images, not atomic. Keep camera and witness still. "
                        "Projection is not LOS. Typed pawns are not verified human identities. "
                        "Body-part labels are visual observations; bone names/indices/transforms "
                        "and obstruction semantics remain unverified."}
    write_json(directory / "capture.json", result)
    return result


class Panel:
    def __init__(self, session, directory):
        self.session, self.directory = session, directory
        self.sequence = 0
        self.pending = None
        self.labels = {}
        self.current_id = None
        self.witness_key = None
        self.cursor_comments = []
        self.raw_comments = []
        self.raw_window = None
        self.raw_region = None
        self.raw_subject = None
        self.root = tk.Tk()
        self.root.title("APB LOS annotations | F8 snapshot | EchoDriver only")
        self.status = tk.StringVar(value="Focus APB, keep camera/witness still, press F8 for ONE snapshot.")
        tk.Label(self.root, textvariable=self.status, wraplength=1080).pack(fill="x")
        tk.Label(self.root, text="After CAPTURE COMPLETE, Alt-tab here. Select a numbered pawn and label it. "
                 "Save frame before next F8. Unknown stays unknown. Close panel to end.").pack()
        image_frame = tk.Frame(self.root)
        image_frame.pack(fill="both", expand=True)
        self.canvas = tk.Canvas(image_frame, width=1000, height=420, background="#181818", cursor="crosshair")
        self.canvas.grid(row=0, column=0, sticky="nsew")
        vertical = ttk.Scrollbar(image_frame, orient="vertical", command=self.canvas.yview)
        vertical.grid(row=0, column=1, sticky="ns")
        horizontal = ttk.Scrollbar(image_frame, orient="horizontal", command=self.canvas.xview)
        horizontal.grid(row=1, column=0, sticky="ew")
        self.canvas.configure(xscrollcommand=horizontal.set, yscrollcommand=vertical.set)
        image_frame.columnconfigure(0, weight=1)
        image_frame.rowconfigure(0, weight=1)
        self.canvas.bind("<Button-1>", self.add_cursor_comment)
        tools = tk.Frame(self.root)
        tools.pack()
        self.zoom = tk.BooleanVar(value=False)
        ttk.Checkbutton(tools, text="2x image zoom (scroll to inspect parts)", variable=self.zoom,
                        command=self.render_image).pack(side="left")
        ttk.Button(tools, text="Undo last cursor comment", command=self.undo_comment).pack(side="left")
        self.component_status = tk.StringVar(value="Select a pawn to inspect recorded component links. Click its image to comment.")
        tk.Label(self.root, textvariable=self.component_status, wraplength=1000).pack()
        row = tk.Frame(self.root)
        row.pack(fill="both", expand=True)
        self.listbox = tk.Listbox(row, width=42, height=12, exportselection=False)
        self.listbox.pack(side="left", fill="both")
        self.listbox.bind("<<ListboxSelect>>", self.select)
        fields = tk.Frame(row)
        fields.pack(side="left", fill="both", expand=True)
        self.variables = {}
        for index, (name, values) in enumerate((("role", ROLES), ("los", STATES), ("observation", OBSERVATIONS),
                                              *((part, STATES) for part in PARTS))):
            tk.Label(fields, text=name).grid(row=index // 4 * 2, column=index % 4)
            variable = tk.StringVar(value="unknown")
            self.variables[name] = variable
            ttk.Combobox(fields, textvariable=variable, values=values, state="readonly", width=24).grid(
                row=index // 4 * 2 + 1, column=index % 4, padx=4, pady=4)
        self.note = tk.StringVar()
        tk.Label(fields, text="Witness name / obstruction / uncertainty").grid(row=6, column=0, columnspan=4)
        tk.Entry(fields, textvariable=self.note, width=76).grid(row=7, column=0, columnspan=4)
        self.witness_status = tk.StringVar(value="Current Player: none selected (annotation witness, not local pawn)")
        tk.Label(self.root, textvariable=self.witness_status).pack()
        ttk.Button(self.root, text="Set selected pawn as Current Player", command=self.set_witness).pack()
        ttk.Button(self.root, text="Current / Show raws", command=self.show_raws).pack()
        ttk.Button(self.root, text="Save frame annotations (unknown entries preserved)", command=self.save).pack(pady=6)
        self.root.protocol("WM_DELETE_WINDOW", self.close)
        self.was_down = bool(session.user32.GetAsyncKeyState(0x77) & 0x8000)
        self.events = (directory / "events.jsonl").open("x", encoding="utf-8", buffering=1)
        self.event("waiting", message=self.status.get())
        self.root.after(75, self.poll)

    def event(self, state, **data):
        self.events.write(json.dumps({"utc": utc_now(), "state": state, **data}) + "\n")
        print(f"{state.upper()}: {data.get('message', '')}", flush=True)

    def stash(self):
        if self.current_id is not None:
            self.labels[self.current_id] = {name: variable.get() for name, variable in self.variables.items()}
            self.labels[self.current_id]["note"] = self.note.get()

    def set_witness(self):
        if self.pending is None or self.current_id is None:
            return
        point = next(p for p in self.pending["points"] if p["id"] == self.current_id)
        self.witness_key = (point["address"], point["instance_signature"])
        self.witness_status.set(f"Current Player: #{point['id']} {point['address']} (exact pawn instance)")
        self.event("current_player_selected", message=self.witness_status.get())

    def select(self, _event=None):
        selection = self.listbox.curselection()
        if not selection or self.pending is None:
            return
        self.stash()
        self.current_id = self.pending["points"][selection[0]]["id"]
        if self.raw_subject is not None and self.raw_subject != self.current_id:
            self.close_raws()
        saved = self.labels.get(self.current_id, {})
        for name, variable in self.variables.items():
            variable.set(saved.get(name, "unknown"))
        self.note.set(saved.get("note", ""))
        point = self.pending["points"][selection[0]]
        owner = point["ownership"]
        self.component_status.set(f"Pawn #{point['id']} {point['address']} | registered components: "
                                  f"{', '.join(owner['components']) or 'none'} | source: {owner['position_source']}\n"
                                  "Yellow P = recorded pawn origin. Bone/mesh geometry is unverified. "
                                  "Click a body part to add a red manual marker and word comment.")

    def display(self):
        self.close_raws()
        self.labels, self.current_id, self.cursor_comments, self.raw_comments = {}, None, [], []
        self.listbox.delete(0, "end")
        for point in self.pending["points"]:
            self.listbox.insert("end", f"#{point['id']} {point['address']} {point['projection']['status']}")
        self.render_image()
        if self.pending["points"]:
            match = next((i for i, p in enumerate(self.pending["points"])
                          if (p["address"], p["instance_signature"]) == self.witness_key), None)
            self.listbox.selection_set(match if match is not None else 0)
            self.select()
            if match is not None:
                self.set_witness()
            else:
                self.witness_key = None
                self.witness_status.set("Current Player: select again; previous pawn is absent or changed")
        else:
            self.witness_key = None
            self.witness_status.set("Current Player: no captured pawn candidates")

    def render_image(self):
        if self.pending is None:
            return
        with Image.open(self.pending["images"]["before"]["path"]) as source:
            image = source.convert("RGB")
        draw = ImageDraw.Draw(image)
        cx, cy = self.pending["cursor_context"]["client_center"]
        draw.line((cx-10, cy, cx+10, cy), fill="cyan", width=2)
        draw.line((cx, cy-10, cx, cy+10), fill="cyan", width=2)
        for point in self.pending["points"]:
            projection = point["projection"]
            if projection["status"] == "inside":
                x, y = projection["client_x"], projection["client_y"]
                draw.ellipse((x-6, y-6, x+6, y+6), outline="yellow", width=2)
                draw.text((x+8, y), f"P{point['id']}", fill="yellow", stroke_width=1, stroke_fill="black")
        for comment in self.cursor_comments:
            x, y = comment["client_x"], comment["client_y"]
            draw.ellipse((x-5, y-5, x+5, y+5), outline="red", width=2)
            draw.text((x+7, y-12), f"C{comment['id']}: {comment['text']}", fill="red",
                      stroke_width=1, stroke_fill="black")
        self.image_scale = 2.0 if self.zoom.get() else 1.0
        if self.image_scale != 1.0:
            image = image.resize((image.width*2, image.height*2))
        self.photo = ImageTk.PhotoImage(image)
        self.canvas.delete("all")
        self.canvas.create_image(0, 0, image=self.photo, anchor="nw")
        self.canvas.configure(scrollregion=(0, 0, image.width, image.height))

    def add_cursor_comment(self, event):
        if self.pending is None or self.current_id is None:
            return
        x = self.canvas.canvasx(event.x) / self.image_scale
        y = self.canvas.canvasy(event.y) / self.image_scale
        image = self.pending["images"]["before"]
        if not (0 <= x < image["client_width"] and 0 <= y < image["client_height"]):
            return
        radius = 7
        sx, sy = x*self.image_scale, y*self.image_scale
        highlight = self.canvas.create_oval(sx-radius, sy-radius, sx+radius, sy+radius,
                                             outline="red", width=3)
        self.root.update_idletasks()
        point = next(p for p in self.pending["points"] if p["id"] == self.current_id)
        comment = simpledialog.askstring("Comment on highlighted part",
                                         f"Pawn #{point['id']}: describe the marked part, e.g. "
                                         "head, left elbow, hand, foot, glass blocks torso.", parent=self.root)
        self.canvas.delete(highlight)
        if comment is None or not comment.strip():
            return
        record = {"id": len(self.cursor_comments)+1, "pawn_id": point["id"],
                  "address": point["address"], "instance_signature": point["instance_signature"],
                  "client_x": x, "client_y": y, "text": comment.strip(),
                  "annotated_utc": utc_now(), "source": "operator_clicked_screen_region",
                  "bone_mapping_status": "unverified"}
        if self.raw_region is not None and self.raw_subject == point["id"] and self.raw_offset.get().strip():
            try:
                record["raw_association"] = self.raw_anchor()
            except ValueError as error:
                messagebox.showerror("Raw offset invalid", str(error))
                return
        self.cursor_comments.append(record)
        self.event("cursor_comment_added", message=record["text"], annotation=record)
        self.render_image()

    def undo_comment(self):
        if self.pending is not None and self.cursor_comments:
            comment = self.cursor_comments.pop()
            self.event("cursor_comment_removed", message=comment["text"], annotation=comment)
            self.render_image()

    def close_raws(self):
        if self.raw_window is not None:
            self.raw_window.destroy()
        self.raw_window, self.raw_region, self.raw_subject = None, None, None

    def show_raws(self):
        if self.pending is None or self.current_id is None:
            return
        self.set_witness()
        self.close_raws()
        point = next(p for p in self.pending["points"] if p["id"] == self.current_id)
        self.raw_subject = point["id"]
        self.raw_regions = [{"name": "Pawn bounded probe", "address": point["address"],
                             "body_hex": point["evidence"]["body_hex"],
                             "source": "selected_pawn", "extent_status": point["evidence"]["extent_status"]}]
        for context in point["evidence"]["pointer_context_unclassified"]:
            self.raw_regions.append({**context, "name": f"Pawn+{context['offset']} -> {context['pointer']}",
                                     "address": context["pointer"], "source": "pointer_context_unclassified",
                                     "extent_status": "bounded_probe_not_verified_object_size"})
        # Display registered component survey bytes separately from later pawn
        # pointer probes. Keep the timing/provenance visible.
        directory = self.directory / f"frame-{self.sequence:04d}"
        raw_path = directory / f"survey-{self.pending['survey']['sequence']:04d}.candidate-bodies.bin"
        try:
            raw = raw_path.read_bytes()
            stride = 8 + 0x800
            if len(raw) % stride:
                raise RuntimeError("Saved component survey record length invalid")
            components = {int(value, 16) for value in point["ownership"]["components"]}
            for offset in range(0, len(raw), stride):
                address = struct.unpack_from("<Q", raw, offset)[0]
                if address in components:
                    self.raw_regions.append({"name": f"Registered component survey {hex(address)}",
                                             "address": hex(address), "body_hex": raw[offset+8:offset+stride].hex(),
                                             "source": str(raw_path),
                                             "extent_status": "earlier_sequential_survey_probe"})
        except (OSError, RuntimeError) as error:
            self.event("raw_component_load_failed", message=str(error))
            self.raw_regions.append({"name": "Component survey unavailable", "address": point["address"],
                                     "source": str(raw_path), "read_error": str(error)})
        window = tk.Toplevel(self.root)
        self.raw_window = window
        window.title(f"Current pawn #{point['id']} {point['address']} | saved raws, read-only")
        window.geometry("1120x680")
        window.protocol("WM_DELETE_WINDOW", self.close_raws)
        tk.Label(window, text="Captured bytes only. Pointer contexts may be shared or unrelated. "
                 "A comment tags evidence; it does not change memory or prove a bone mapping.", wraplength=1080).pack()
        content = tk.Frame(window)
        content.pack(fill="both", expand=True)
        self.raw_list = tk.Listbox(content, width=49, exportselection=False)
        self.raw_list.pack(side="left", fill="y")
        text_frame = tk.Frame(content)
        text_frame.pack(side="left", fill="both", expand=True)
        self.raw_text = tk.Text(text_frame, wrap="none", font=("Consolas", 10))
        self.raw_text.grid(row=0, column=0, sticky="nsew")
        scrollbar = ttk.Scrollbar(text_frame, command=self.raw_text.yview)
        scrollbar.grid(row=0, column=1, sticky="ns")
        horizontal = ttk.Scrollbar(text_frame, orient="horizontal", command=self.raw_text.xview)
        horizontal.grid(row=1, column=0, sticky="ew")
        text_frame.rowconfigure(0, weight=1)
        text_frame.columnconfigure(0, weight=1)
        self.raw_text.configure(yscrollcommand=scrollbar.set, xscrollcommand=horizontal.set)
        self.raw_text.bind("<ButtonRelease-1>", self.pick_raw_row)
        for region in self.raw_regions:
            suffix = " [READ FAILED]" if "read_error" in region else ""
            self.raw_list.insert("end", region["name"] + suffix)
        self.raw_list.bind("<<ListboxSelect>>", self.select_raw)
        controls = tk.Frame(window)
        controls.pack(fill="x")
        tk.Label(controls, text="Byte offset (decimal or 0xHEX):").pack(side="left")
        self.raw_offset = tk.StringVar(value="")
        ttk.Entry(controls, textvariable=self.raw_offset, width=14).pack(side="left")
        ttk.Button(controls, text="Inspect address / values", command=self.inspect_raw).pack(side="left")
        ttk.Button(controls, text="Attach word comment to raw address", command=self.comment_raw).pack(side="left")
        self.raw_values = tk.StringVar(value="Choose a region and byte offset. Image clicks can also attach this raw location.")
        tk.Label(window, textvariable=self.raw_values, wraplength=1080).pack()
        self.raw_list.selection_set(0)
        self.select_raw()

    def select_raw(self, _event=None):
        selection = self.raw_list.curselection()
        if not selection:
            return
        self.raw_region = self.raw_regions[selection[0]]
        self.raw_offset.set("")
        self.raw_values.set("Choose an offset to inspect or associate with a comment.")
        region = self.raw_region
        lines = [region["name"], f"Address: {region['address']}  Source: {region['source']}",
                 f"Extent: {region.get('extent_status', 'unavailable')}",
                 f"Vtable RVA: {region.get('vtable_rva', 'not classified')}", ""]
        if "read_error" in region:
            lines.append("READ FAILED: " + region["read_error"])
        else:
            raw = bytes.fromhex(region["body_hex"])
            base = int(region["address"], 16)
            for offset in range(0, len(raw), 16):
                block = raw[offset:offset+16]
                readable = "".join(chr(value) if 32 <= value < 127 else "." for value in block)
                lines.append(f"+{offset:04X} {base+offset:016X}  {block.hex(' '):47}  {readable}")
        self.raw_text.configure(state="normal")
        self.raw_text.delete("1.0", "end")
        self.raw_text.insert("1.0", "\n".join(lines))
        self.raw_text.configure(state="disabled")

    def raw_anchor(self):
        region = self.raw_region
        if region is None or "body_hex" not in region:
            raise ValueError("Selected region has no captured bytes")
        try:
            offset = int(self.raw_offset.get().strip(), 0)
        except ValueError:
            raise ValueError("Enter a decimal offset or 0x-prefixed hexadecimal offset") from None
        raw = bytes.fromhex(region["body_hex"])
        if not 0 <= offset < len(raw):
            raise ValueError(f"Offset must be within 0x0..0x{len(raw)-1:X}")
        return {"region_address": region["address"], "region_name": region["name"],
                "source": region["source"], "byte_offset": hex(offset),
                "absolute_address": hex(int(region["address"], 16)+offset),
                "captured_bytes_hex": raw[offset:offset+16].hex(),
                "mapping_status": "operator_association_not_verified_screen_to_memory_mapping"}

    def pick_raw_row(self, event):
        index = self.raw_text.index(f"@{event.x},{event.y}")
        line = self.raw_text.get(index + " linestart", index + " lineend")
        if line.startswith("+"):
            self.raw_offset.set("0x" + line.split()[0][1:])
            self.inspect_raw()

    def inspect_raw(self):
        try:
            anchor = self.raw_anchor()
        except ValueError as error:
            messagebox.showerror("Raw location", str(error), parent=self.raw_window)
            return
        raw = bytes.fromhex(anchor["captured_bytes_hex"])
        values = f"{anchor['absolute_address']}: {raw.hex(' ')}"
        if len(raw) >= 4:
            integer, = struct.unpack_from("<I", raw)
            floating, = struct.unpack_from("<f", raw)
            values += f" | u32={integer} | f32={floating if math.isfinite(floating) else 'nonfinite'}"
        if len(raw) >= 8:
            values += f" | u64=0x{struct.unpack_from('<Q', raw)[0]:X}"
        self.raw_values.set(values + " (interpretations only; native field type unknown)")

    def comment_raw(self):
        try:
            anchor = self.raw_anchor()
        except ValueError as error:
            messagebox.showerror("Raw location", str(error), parent=self.raw_window)
            return
        comment = simpledialog.askstring("Raw address comment", f"Comment on {anchor['absolute_address']}:",
                                         parent=self.raw_window)
        if comment is None or not comment.strip():
            return
        point = next(p for p in self.pending["points"] if p["id"] == self.raw_subject)
        record = {"id": len(self.raw_comments)+1, "pawn_id": point["id"], "address": point["address"],
                  "instance_signature": point["instance_signature"], "text": comment.strip(),
                  "annotated_utc": utc_now(), "raw_association": anchor}
        self.raw_comments.append(record)
        self.event("raw_comment_added", message=record["text"], annotation=record)
        self.raw_values.set(f"Saved draft comment #{record['id']}: {record['text']} — Save frame to commit.")

    def save(self):
        if self.pending is None:
            return
        if self.pending["points"] and self.witness_key is None:
            messagebox.showerror("Current Player required", "Select your witness pawn and click Set selected pawn as Current Player.")
            return
        self.stash()
        records = []
        for point in self.pending["points"]:
            label = self.labels.get(point["id"], {})
            observation = label.get("observation", "unknown")
            los = {"visible": "visible", "invisible": "obstructed",
                   "partial_cursor_visible": "partial", "partial_cursor_invisible": "partial"}.get(
                       observation, label.get("los", "unknown"))
            records.append({"id": point["id"], "address": point["address"],
                            "instance_signature": point["instance_signature"],
                            "role": label.get("role", "unknown"), "los": los,
                            "observation": observation,
                            "current_player": (point["address"], point["instance_signature"]) == self.witness_key,
                            "body_parts": {part: label.get(part, "unknown") for part in PARTS},
                            "note": label.get("note", "")})
        path = self.directory / f"frame-{self.sequence:04d}" / "annotations.json"
        try:
            write_json(path, {"format": "apb-visibility-labels-v1", "annotated_utc": utc_now(),
                             "capture": "capture.json", "source": "operator_visual_ground_truth",
                             "labels": records, "cursor_comments": self.cursor_comments,
                             "raw_comments": self.raw_comments,
                             "coordinate_space": "before.png client pixels at original resolution",
                             "bone_mapping_status": "unverified"})
        except (OSError, RuntimeError) as error:
            self.event("save_failed", message=str(error))
            messagebox.showerror("Save failed", str(error))
            return
        self.pending = None
        self.close_raws()
        self.event("annotations_saved", message=str(path))
        self.status.set("Saved. Prepare the next view in APB and press F8, or close to finish.")

    def poll(self):
        pressed = bool(self.session.user32.GetAsyncKeyState(0x77) & 0x8000)
        if pressed and not self.was_down:
            self.was_down = pressed
            if self.pending is not None:
                self.status.set("Save the current frame annotations before the next F8 snapshot.")
                self.event("capture_blocked", message=self.status.get())
            else:
                try:
                    self.session.require_foreground()
                except ForegroundLost as error:
                    self.status.set(str(error))
                    self.event("foreground_required", message=str(error))
                else:
                    self.sequence += 1
                    frame = self.directory / f"frame-{self.sequence:04d}"
                    frame.mkdir()
                    self.status.set("CAPTURING: keep APB foreground and camera/witness still.")
                    self.root.update_idletasks()
                    self.event("capture_started", message=str(frame))
                    try:
                        self.pending = capture(self.session, frame, self.sequence)
                        self.display()
                        count = len(self.pending["points"])
                        failures = len(self.pending["rejected"])
                        self.status.set(f"CAPTURE COMPLETE: {count} pawn candidates, {failures} rejected. "
                                        "Alt-tab here to annotate. Check before.png and after.png for motion.")
                        self.event("capture_complete", message=self.status.get(), directory=str(frame))
                    except (OSError, RuntimeError, ValueError) as error:
                        self.pending = None
                        self.status.set(f"CAPTURE FAILED: {error}. Inspect events.jsonl; correct and press F8 to retry.")
                        self.event("capture_failed", message=str(error), directory=str(frame))
                        write_json(frame / "failure.json", {"utc": utc_now(), "error": str(error)})
        self.was_down = pressed
        self.root.after(75, self.poll)

    def close(self):
        if self.pending is not None:
            self.stash()
            write_json(self.directory / f"frame-{self.sequence:04d}" / "annotations.unsaved.json",
                       {"status": "unfinished_not_accepted_ground_truth", "draft": self.labels,
                        "cursor_comments": self.cursor_comments, "raw_comments": self.raw_comments})
        self.close_raws()
        self.event("closed", message="Session ended; captured files retained")
        self.events.close()
        self.root.destroy()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output_directory.exists():
        parser.error("Use a valid PID and a new output directory")
    session = Session(args.pid)
    try:
        args.output_directory.mkdir(parents=True)
        write_json(args.output_directory / "session.json", {"identity": session.identity,
                   "transport": "EchoDriver", "mode": "F8_single_snapshot_then_operator_annotation"})
        panel = Panel(session, args.output_directory)
        panel.root.mainloop()
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # PowerShell 5 treats native stderr as a terminating pipeline error under
        # Stop. Preserve the complete diagnostic in the launcher log on stdout.
        traceback.print_exc(file=sys.stdout)
        raise SystemExit(1)
