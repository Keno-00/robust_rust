"""Live, read-only candidates for the explicitly audited 2026-10-18 build."""

import math
import struct

from apb_cursor_modules import module_base
from apb_manual_labels import instance_signature
from apb_runtime_audit import TYPES, kind, u64


BUILD_SHA256 = "35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453"
ROOT_RVA = 0x3DCE240
RECENCY_SCOPE = "render recency candidate, not geometry LOS"


class RuntimeChanged(RuntimeError):
    """Reject this frame because a guarded runtime binding changed."""


class ValidationRuntime:
    def __init__(self, session, profile, diagnostic=None):
        if (profile.get("format") != "apb-build-candidates-v1"
                or profile.get("identity") != session.identity
                or session.identity["image_sha256"] != BUILD_SHA256):
            raise RuntimeError("Live validation requires the exact audited build and current process profile")
        self.session, self.profile, self.diagnostic = session, profile, diagnostic
        self.base = module_base(session)
        if int(profile["module_base"], 0) != self.base:
            raise RuntimeError("Profile module base differs from current APB")
        self.previous_binding = None
        self.previous_world_clock = None

    def read(self, address, size):
        if not 0x10000 <= address < 0x800000000000:
            raise RuntimeError(f"Invalid runtime pointer {hex(address)}")
        return b"".join(self.session.read(address+offset, min(4096, size-offset))
                        for offset in range(0, size, 4096))

    def typed(self, address, label, size=0x40):
        body = self.read(address, size)
        if kind(body, self.base) != TYPES[label]:
            raise RuntimeError(f"Runtime {label} type differs at {hex(address)}: {kind(body, self.base)}")
        return body

    def stable(self, address, body, label):
        if instance_signature(self.typed(address, label)) != instance_signature(body):
            raise RuntimeChanged(f"Runtime {label} fingerprint changed at {hex(address)}")

    def root_snapshot(self):
        raw = self.read(self.base+ROOT_RVA, 16)
        table, count, capacity = struct.unpack("<Qii", raw)
        if count == 0 and 0 <= capacity <= 1000000:
            raise RuntimeChanged("HUD table has no current entries; waiting for loading")
        if not 0 < count <= capacity <= 1000000:
            raise RuntimeError("Current HUD root unavailable: invalid table bounds")
        return raw, table, self.read(table, min(count, 128)*8)

    def guard_hud(self, binding, body):
        before_root, table, index, address, before_entries = binding
        root, current_table, entries = self.root_snapshot()
        changes = []
        if current_table != table or index*8+8 > len(entries):
            if self.diagnostic:
                self.diagnostic("hud_binding_changed", before_root_hex=before_root.hex(),
                                after_root_hex=root.hex(), index=index, reason="table or active slot changed")
            raise RuntimeChanged("HUD table pointer or active selected slot changed")
        matches = []
        for slot, (pointer,) in enumerate(struct.iter_unpack("<Q", entries)):
            old = u64(before_entries, slot*8) if slot*8+8 <= len(before_entries) else None
            changed = pointer != old
            record = {"index": slot, "before_address": hex(old) if old is not None else None,
                      "after_address": hex(pointer)}
            if not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
                record["classification"] = "null_or_nonobject_pointer"
            else:
                try:
                    header = self.read(pointer, 0x40)
                except OSError as error:
                    if self.diagnostic:
                        self.diagnostic("hud_entry_unreadable", before_root_hex=before_root.hex(),
                                        after_root_hex=root.hex(), changed=changed, error=str(error), **record)
                    raise RuntimeChanged(f"HUD entry {slot} unreadable during binding verification") from error
                observed_kind = kind(header, self.base)
                record.update(vtable_rva=hex(observed_kind[0]), marker=observed_kind[1],
                              instance_signature=instance_signature(header),
                              classification="hud" if observed_kind == TYPES["hud"] else "other_typed_object")
                if observed_kind == TYPES["hud"]:
                    matches.append((slot, pointer, instance_signature(header)))
            if changed:
                changes.append(record)
        final_root, final_table, final_entries = self.root_snapshot()
        if self.diagnostic and (root != before_root or changes or final_root != root):
            self.diagnostic("hud_table_observed", before_root_hex=before_root.hex(),
                            after_root_hex=root.hex(), final_root_hex=final_root.hex(),
                            before_entries_hex=before_entries.hex(), after_entries_hex=entries.hex(),
                            final_entries_hex=final_entries.hex(), changed_entries=changes,
                            index=index, selected_address=hex(address))
        if (matches != [(index, address, instance_signature(body))]
                or final_table != table or final_entries != entries
                or u64(entries, index*8) != address):
            raise RuntimeChanged("HUD uniqueness, fingerprint or table entries changed during verification")
        self.stable(address, body, "hud")
        if u64(self.read(table+index*8, 8), 0) != address:
            raise RuntimeChanged("HUD selected slot changed again during verification")
        return final_root, table, index, address, final_entries

    def camera(self):
        root, table, entries = self.root_snapshot()
        # This root's audited HUD binding is in the first 128 slots.
        matches = []
        for index, (address,) in enumerate(struct.iter_unpack("<Q", entries)):
            if not 0x10000 <= address < 0x800000000000 or address % 8:
                continue
            header = self.read(address, 0x40)
            if kind(header, self.base) == TYPES["hud"]:
                matches.append((index, address))
        if len(matches) != 1:
            raise RuntimeChanged(f"Waiting for a unique typed HUD; found {len(matches)}")
        index, address = matches[0]
        body = self.typed(address, "hud", 0x700)
        matrix = struct.unpack_from("<16f", body, 0x680)
        xyz = struct.unpack_from("<3f", body, 0x6C0)
        if (not all(math.isfinite(v) for v in (*matrix, *xyz))
                or abs(matrix[14]+9.99) > .02 or abs(matrix[15]) > .02
                or any(sum(v*v for v in matrix[col::4]) < 1e-12 for col in (0, 1, 3))):
            raise RuntimeError("Current HUD camera projection shape invalid")
        self.stable(address, body, "hud")
        binding = self.guard_hud((root, table, index, address, entries), body)
        return {"address": hex(address), "instance_signature": instance_signature(body),
                "matrix": matrix, "position": xyz, "matrix_offset": 0x680,
                "position_offset": 0x6C0, "body_hex": body.hex()}, body, binding

    def pose(self, pawn, pawn_body):
        address = u64(pawn_body, 0x248)
        body = self.read(address, 0x240)
        if kind(body, self.base) != (0x31A54F8, 3269):
            raise RuntimeError("Local collision type differs from new-build candidate")
        origin = struct.unpack_from("<3f", body, 0x108)
        extent = struct.unpack_from("<3f", body, 0x114)
        sphere = struct.unpack_from("<f", body, 0x120)[0]
        half_height, radius = struct.unpack_from("<2f", body, 0x234)
        if (u64(body, 0x44) != pawn or u64(body, 0x7C) != pawn
                or not all(math.isfinite(v) and abs(v) < 1000000 for v in origin)
                or not all(math.isfinite(v) and v > 0 for v in (*extent, sphere, half_height, radius))
                or not 10 <= half_height <= 300 or not 1 <= radius <= 150
                or max(abs(extent[0]-radius), abs(extent[1]-radius), abs(extent[2]-half_height)) > .05
                or abs(sphere-math.sqrt(sum(v*v for v in extent))) > .05):
            raise RuntimeError("Local collision ownership/geometry invalid")
        after = self.read(address, 0x240)
        if (kind(after, self.base) != kind(body, self.base)
                or instance_signature(after) != instance_signature(body)
                or after[0x234:0x23C] != body[0x234:0x23C]
                or u64(after, 0x44) != pawn or u64(after, 0x7C) != pawn
                or u64(self.read(pawn+0x248, 8), 0) != address):
            raise RuntimeChanged("Local collision changed during sample")
        return {"address": hex(address), "instance_signature": instance_signature(body),
                "half_height": half_height, "radius": radius, "origin": origin,
                "extent": extent, "sphere": sphere, "body_hex": body.hex(),
                "scope": "numeric collision geometry; posture names unconfirmed"}

    def sample(self):
        self.session.require_foreground()
        camera, hud_body, root_binding = self.camera()
        hud = int(camera["address"], 16)
        world = u64(hud_body, 0x2E0)
        if not world:
            raise RuntimeChanged("HUD World unavailable during loading")
        world_body = self.typed(world, "world", 0x6A8)
        controller = u64(world_body, 0x358)
        control = self.typed(controller, "controller", 0x1390)
        possessed = u64(control, 0x790)
        if not possessed:
            raise RuntimeChanged("Controller has no current possession; waiting for spawn")
        possessed_body = self.read(possessed, 0x3E8)
        if kind(possessed_body, self.base) == TYPES["pawn"]:
            pawn, state = possessed, "on_foot"
            if u64(possessed_body, 0x3E0) != controller:
                raise RuntimeError("On-foot controller backlink differs")
        elif kind(possessed_body, self.base) == TYPES["vehicle"]:
            pawn, state = u64(control, 0x1380), "vehicle_possession"
            if (u64(control, 0x1388) != pawn
                    or u64(self.read(pawn+0x178, 8), 0) != possessed
                    or u64(possessed_body, 0x3E0) != controller):
                raise RuntimeError("Vehicle/current-character ownership agreement failed")
        else:
            raise RuntimeError("Current possession has an unsupported new-build type")
        pawn_body = self.typed(pawn, "pawn", 0x410)
        if u64(pawn_body, 0x2E0) != world or u64(possessed_body, 0x2E0) != world:
            raise RuntimeError("Current possession/character World differs")
        local_xyz = struct.unpack_from("<3f", pawn_body, 0x88)
        if not all(math.isfinite(v) and abs(v) < 1000000 for v in local_xyz):
            raise RuntimeError("Local character position invalid")
        local = {"state": state, "world": hex(world), "controller": hex(controller),
                 "possessed": hex(possessed), "pawn": hex(pawn), "position": local_xyz,
                 "instance_signature": instance_signature(pawn_body), "pose": self.pose(pawn, pawn_body)}
        head = u64(world_body, 0x48C)
        address, seen, nodes, points = head, set(), [], []
        before_clock = struct.unpack_from("<d", world_body, 0x6A0)[0]
        while address:
            if address in seen or len(nodes) >= 512:
                raise RuntimeError("Rooted roster cycle or 512-node limit exceeded")
            seen.add(address)
            body = self.read(address, 0x3E8)
            node_kind = kind(body, self.base)
            if node_kind not in (TYPES["pawn"], TYPES["vehicle"]) or u64(body, 0x2E0) != world:
                raise RuntimeError(f"Rooted roster type/World invalid at {hex(address)}")
            next_address = u64(body, 0x350)
            nodes.append((address, body, next_address))
            if node_kind == TYPES["pawn"] and address != pawn:
                xyz = struct.unpack_from("<3f", body, 0x88)
                timestamp = struct.unpack_from("<d", body, 0xB4)[0]
                if not all(math.isfinite(v) and abs(v) < 1000000 for v in xyz):
                    raise RuntimeError("Remote character position invalid")
                points.append({"address": hex(address), "vtable_rva": hex(node_kind[0]),
                    "marker": node_kind[1], "instance_signature": instance_signature(body),
                    "position_offset": 0x88, "xyz": xyz, "object_name": "cAPBPawn",
                    "render_timestamp": timestamp, "body_hex": body.hex(),
                    "scope": RECENCY_SCOPE})
            address = next_address
        after_world = self.typed(world, "world", 0x6A8)
        clock = struct.unpack_from("<d", after_world, 0x6A0)[0]
        if (not all(math.isfinite(v) and v >= 0 for v in (before_clock, clock)) or clock < before_clock
                or self.previous_world_clock and self.previous_world_clock[0] == (world, instance_signature(world_body))
                and clock < self.previous_world_clock[1]):
            raise RuntimeError("World clock invalid or moved backward")
        for point in points:
            timestamp = point["render_timestamp"]
            if not math.isfinite(timestamp) or timestamp < 0 or timestamp > clock+.25:
                raise RuntimeError("Remote render timestamp inconsistent with World clock")
            point.update(age=clock-timestamp, recently_rendered=timestamp > 0 and 0 <= clock-timestamp <= 1.0)
        for address, body, next_address in nodes:
            after = self.read(address, 0x358)
            if (kind(after, self.base) != kind(body, self.base)
                    or instance_signature(after) != instance_signature(body)
                    or u64(after, 0x2E0) != world or u64(after, 0x350) != next_address):
                raise RuntimeChanged("Roster changed during sample")
        if pawn not in seen:
            raise RuntimeError("Local character absent from rooted roster")
        self.stable(world, world_body, "world")
        self.stable(controller, control, "controller")
        self.stable(pawn, pawn_body, "pawn")
        self.stable(hud, hud_body, "hud")
        self.stable(possessed, possessed_body, "pawn" if state == "on_foot" else "vehicle")
        self.guard_hud(root_binding, hud_body)
        if (u64(self.read(hud+0x2E0, 8), 0) != world
                or instance_signature(after_world) != instance_signature(world_body)
                or u64(after_world, 0x48C) != head or u64(after_world, 0x358) != controller
                or u64(self.read(controller+0x790, 8), 0) != possessed):
            raise RuntimeChanged("Root/camera/ownership chain changed during sample")
        if state == "vehicle_possession":
            check = self.typed(controller, "controller", 0x1390)
            if (u64(check, 0x1380) != pawn or u64(check, 0x1388) != pawn
                    or u64(self.read(pawn+0x178, 8), 0) != possessed
                    or u64(self.read(possessed+0x3E0, 8), 0) != controller):
                raise RuntimeChanged("Vehicle ownership changed during sample")
        elif u64(self.read(pawn+0x3E0, 8), 0) != controller:
            raise RuntimeChanged("On-foot ownership changed during sample")
        self.session.require_foreground()
        binding = (hud, instance_signature(hud_body), world, instance_signature(world_body),
                   controller, instance_signature(control), pawn, instance_signature(pawn_body), state)
        if binding != self.previous_binding and self.diagnostic:
            self.diagnostic("runtime_binding", previous=self.previous_binding, current=binding, local=local)
        self.previous_binding = binding
        self.previous_world_clock = ((world, instance_signature(world_body)), clock)
        return {"camera": camera, "local": local, "world_clock": clock, "points": points,
                "status": f"{state} | half-height {local['pose']['half_height']:.2f} radius {local['pose']['radius']:.2f} | {RECENCY_SCOPE}"}
