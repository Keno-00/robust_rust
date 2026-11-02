"""Fixed visibility probe scope and strict identity/ownership checks."""

import math
import struct
from pathlib import Path

from apb_camera_live import locate_camera, camera_identity
from apb_character_ownership import owned_position, resolve_local, typed_header, WORLD_TYPE
from apb_district_candidates import CAMERA_ROOT
from apb_f8_common import camera_at, project
from apb_manual_labels import instance_signature


def compact_camera(camera):
    return {key: camera[key] for key in ("address", "position", "matrix")}


class VisibilityScope:
    def __init__(self, session, base, source, point, selection_directory, camera_diagnostic=None,
                 allow_camera_rebinding=False):
        self.session, self.base, self.point = session, base, point
        self.camera_diagnostic = camera_diagnostic
        self.allow_camera_rebinding = allow_camera_rebinding
        self.camera_address = int(source["camera"]["address"], 16)
        self.camera_signature = instance_signature(session.read(self.camera_address, 0x40))
        self.camera_owner = camera_identity(locate_camera(session, base, CAMERA_ROOT, 128, self.camera_diagnostic))
        if self.camera_owner[2] != self.camera_address:
            raise RuntimeError("Selected survey camera differs from current rooted camera; restart selection")
        self.local = resolve_local(session, base, self.camera_address)
        if self.local["status"] != "local_pawn_present":
            raise RuntimeError("Observer must be alive and on foot; restart selection")
        self.owner = owned_position(session, base, point)
        if self.owner["vehicle_attachment"]:
            raise RuntimeError("Witness must be on foot; restart selection")
        point["ownership"] = self.owner
        self.pawn = int(point["address"], 16)
        self.world = int(self.local["world"], 16)
        self.world_signature = typed_header(session, base, self.world, WORLD_TYPE)
        self.components = tuple(int(value, 16) for value in self.owner["components"])
        if len(self.components) > 32:
            raise RuntimeError("More than 32 witness components; inspect ownership before scanning")
        # A recorded census bounds out known neighbors. It is NOT a proof of
        # allocation size, reflection metadata, mesh ownership or full coverage.
        census = (selection_directory / "survey-0001.headers.bin").read_bytes()
        if len(census) % 0x48:
            raise RuntimeError("Selection object-header census is truncated")
        addresses = {struct.unpack_from("<Q", census, offset)[0]
                     for offset in range(0, len(census), 0x48)}
        if source.get('pawn_source') == 'world-roster':
            name = source['pawn_roster']['header_file']
            if name != Path(name).name:
                raise RuntimeError('Rooted roster census must be a selection-local file')
            rooted_census = (selection_directory/name).read_bytes()
            if len(rooted_census) % 0x48:
                raise RuntimeError('Rooted pawn census is truncated')
            addresses.update(struct.unpack_from('<Q', rooted_census, offset)[0]
                             for offset in range(0, len(rooted_census), 0x48))
        roots = [("pawn", self.pawn, 0x3000), ("world", self.world, 0x1000)]
        roots += [(f"registered_component_{index}", address, 0x1000)
                  for index, address in enumerate(self.components)]
        addresses.update(address for _, address, _ in roots)
        self.regions = []
        seen = set()
        for name, address, requested in roots:
            if address in seen:
                raise RuntimeError(f"Aliased scope root: {name} at 0x{address:X}")
            seen.add(address)
            neighbors = sorted(value for value in addresses if address < value < address+requested)
            size = neighbors[0]-address if neighbors else requested
            if size < 0x40:
                raise RuntimeError(f"Overlapping object headers at {name}; inspect scope")
            self.regions.append({"name": name, "address": address, "size": size,
                "requested_size": requested, "signature": instance_signature(session.read(address, 0x40)),
                "next_known_object": hex(neighbors[0]) if neighbors else None,
                "limit": "Probe prefix; object extent and component type semantics unverified"})
        self.reference_camera = None
        self.observer_protocol = False
        self.allow_camera_motion = False
        self.reference_target = None

    def begin_capture(self, phase, args):
        self.observer_protocol = args.protocol == "observer-los"
        self.allow_camera_motion = self.observer_protocol and phase.get("moving_observer", False)
        if self.observer_protocol:
            # Only the declared observer protocol changes the baseline between
            # bursts. The fixed-observer discovery keeps its session baseline.
            self.reference_camera = None
            if self.reference_target is None:
                self.reference_target = tuple(self.owner["position"])

    def verify(self):
        session = self.session
        session.require_foreground()
        candidate = locate_camera(session, self.base, CAMERA_ROOT, 128, self.camera_diagnostic)
        binding = camera_identity(candidate)
        current_signature = instance_signature(session.read(candidate['camera'], 0x40))
        if candidate['camera'] != self.camera_address or current_signature != self.camera_signature:
            raise RuntimeError(f"Rooted camera instance changed: address {hex(self.camera_address)} -> "
                               f"{hex(candidate['camera'])}, signature {self.camera_signature} -> "
                               f"{current_signature}; restart witness selection")
        if binding != self.camera_owner:
            if not self.allow_camera_rebinding:
                raise RuntimeError(f"Rooted camera binding changed: {self.camera_owner} -> {binding}; restart witness selection")
            from apb_f8_common import utc_now
            event = {'utc': utc_now(), 'outcome': 'accepted_same_camera_rebinding',
                     'binding_before': self.camera_owner, 'binding_after': binding,
                     'camera': hex(self.camera_address), 'signature': current_signature,
                     'reason': 'Unique exact camera instance reverified at its current rooted table slot'}
            if self.camera_diagnostic is not None:
                self.camera_diagnostic(event)
            print(f"CAMERA REBOUND: {self.camera_owner} -> {binding}; same verified instance retained.", flush=True)
            self.camera_owner = binding
        if resolve_local(session, self.base, self.camera_address) != self.local:
            raise RuntimeError("Local ownership changed; restart selection after respawn/district change")
        if typed_header(session, self.base, self.world, WORLD_TYPE) != self.world_signature:
            raise RuntimeError("World instance changed; restart selection")
        if struct.unpack('<Q', session.read(self.pawn+0x2E0, 8))[0] != self.world:
            raise RuntimeError('Witness moved to another World; restart selection')
        owner = owned_position(session, self.base, self.point)
        for key in ("pawn", "pawn_signature", "controller", "components", "vehicle_attachment", "status"):
            if owner[key] != self.owner[key]:
                raise RuntimeError(f"Witness ownership changed ({key}); restart selection")
        self.point["position"] = owner["position"]
        return owner

    def check_camera(self, camera, position_tolerance, matrix_tolerance):
        current = compact_camera(camera)
        if self.reference_camera is None:
            self.reference_camera = current
        reference = self.reference_camera
        displacement = math.dist(camera["position"], reference["position"])
        matrix_delta = max(abs(a-b) for a, b in zip(camera["matrix"], reference["matrix"]))
        if not self.allow_camera_motion and (displacement > position_tolerance or matrix_delta > matrix_tolerance):
            action = ("Stop the PC for each settled burst and restart selection."
                      if self.observer_protocol else "Keep PC still and restart the session.")
            raise RuntimeError(f"PC camera moved: displacement {displacement:.6g}, matrix delta "
                               f"{matrix_delta:.6g}. {action}")
        return {"position_delta": displacement, "matrix_max_delta": matrix_delta}

    def read_round(self, origin, image, args, observation):
        import time
        self.verify()
        camera_before = camera_at(self.session, self.camera_address)
        delta = self.check_camera(camera_before, args.camera_position_tolerance, args.camera_matrix_tolerance)
        bodies, timing = {}, []
        for region in self.regions:
            begin = time.monotonic()-origin
            body = b"".join(self.session.read(region["address"]+offset, min(4096, region["size"]-offset))
                            for offset in range(0, region["size"], 4096))
            end = time.monotonic()-origin
            if (instance_signature(body) != region["signature"]
                    or instance_signature(self.session.read(region["address"], 0x40)) != region["signature"]):
                raise RuntimeError(f"Changed instance in {region['name']}; restart selection")
            bodies[region["name"]] = body
            timing.append({"name": region["name"], "size": len(body), "start_s": begin, "end_s": end})
        owner = self.verify()
        camera_after = camera_at(self.session, self.camera_address)
        self.check_camera(camera_after, args.camera_position_tolerance, args.camera_matrix_tolerance)
        xyz = struct.unpack("<fff", self.session.read(self.pawn+0x88, 12))
        if not all(math.isfinite(value) and abs(value) < 1_000_000 for value in xyz):
            raise RuntimeError("Witness position is invalid")
        target_delta = None
        if self.observer_protocol:
            target_delta = max(math.dist(xyz, self.reference_target),
                               math.dist(owner["position"], self.reference_target),
                               math.dist(struct.unpack_from('<fff', bodies['pawn'], 0x88), self.reference_target))
            if target_delta > args.target_position_tolerance:
                raise RuntimeError(f"Fixed laptop target moved {target_delta:.6g} world units from session reference. "
                                   "Restart selection with the laptop stationary; evidence retained.")
        projection = project(camera_after, xyz, image["client_width"], image["client_height"])
        if observation not in ("clear", "blocked"):
            raise RuntimeError(f"Scene {observation} is outside the in-view visibility protocol")
        if projection["status"] != "inside":
            raise RuntimeError(f"Witness origin projects {projection['status']} during {observation}. "
                               "Check selected witness and scene before starting a new session.")
        return bodies, timing, {"camera": compact_camera(camera_after), "camera_delta": delta,
            "position": xyz, "ownership_position": owner["position"], "projection": projection,
            "target_displacement_from_session_reference": target_delta}
