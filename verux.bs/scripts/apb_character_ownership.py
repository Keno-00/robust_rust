"""Explicit diagnostic local ownership chain; all reads use EchoDriver.

These exact types/links were observed in live captures. This identifies the
current local pawn, not a persistent account identity or a district roster.
"""

import struct
import math
from collections import defaultdict

from apb_camera_live import valid_pointer
from apb_manual_labels import instance_signature


WORLD_TYPE = (0x3161E78, 430)
CONTROLLER_TYPE = (0x2A7F158, 998)
PAWN_TYPE = (0x2AAEFA0, 5778)
VEHICLE_TYPE = (0x2B29FB8, 6064)
COMPONENT_TYPE = (0x3142FB0, 4741)
# Both exact registered component types have recorded +0x178 pawn backlinks.
# The local character can retain only the second type after vehicle exit.
REGISTERED_COMPONENT_TYPES = {COMPONENT_TYPE, (0x2B4B7A0, 6683)}


def typed_header(session, base, address, expected):
    if not valid_pointer(address):
        raise RuntimeError(f"Ownership pointer unavailable: 0x{address:X}")
    header = session.read(address, 0x40)
    actual = (struct.unpack_from("<Q", header)[0]-base,
              struct.unpack_from("<I", header, 0x3C)[0])
    if actual != expected:
        raise RuntimeError(f"Ownership type changed at 0x{address:X}: {actual}")
    return instance_signature(header)


def pointer_field(session, address, offset):
    return struct.unpack("<Q", session.read(address+offset, 8))[0]


def resolve_local(session, base, camera):
    """Recheck every traversed link; no cached addresses survive a new sample."""
    world = pointer_field(session, camera, 0x2E0)
    world_signature = typed_header(session, base, world, WORLD_TYPE)
    controller = pointer_field(session, world, 0x358)
    controller_signature = typed_header(session, base, controller, CONTROLLER_TYPE)
    possessed = pointer_field(session, controller, 0x790)
    pawn = possessed
    pawn_signature = None
    state = "local_controller_without_pawn"
    vehicle_signature = None
    if possessed:
        header = session.read(possessed, 0x40)
        kind = (struct.unpack_from("<Q", header)[0]-base, struct.unpack_from("<I", header, 0x3C)[0])
        if kind == VEHICLE_TYPE:
            vehicle_signature = typed_header(session, base, possessed, VEHICLE_TYPE)
            pawn = pointer_field(session, controller, 0x1380)
            if not pawn or pointer_field(session, controller, 0x1388) != pawn:
                raise RuntimeError("Vehicle possession has no agreeing local-character links")
            pawn_signature = typed_header(session, base, pawn, PAWN_TYPE)
            if (pointer_field(session, pawn, 0x178) != possessed
                    or pointer_field(session, possessed, 0x3E0) != controller):
                raise RuntimeError("Local driver/character/vehicle links do not agree")
            state = "local_driver_in_vehicle"
        elif kind != PAWN_TYPE:
            raise RuntimeError(f"Unsupported possession type at 0x{possessed:X}: {kind}")
        else:
            state = "local_pawn_present"
        pawn_signature = typed_header(session, base, pawn, PAWN_TYPE)
        if not vehicle_signature and pointer_field(session, pawn, 0x3E0) != controller:
            raise RuntimeError("Local pawn/controller reciprocal link is unavailable")
    if (pointer_field(session, camera, 0x2E0) != world
            or pointer_field(session, world, 0x358) != controller
            or pointer_field(session, controller, 0x790) != possessed
            or typed_header(session, base, world, WORLD_TYPE) != world_signature
            or typed_header(session, base, controller, CONTROLLER_TYPE) != controller_signature
            or (pawn and typed_header(session, base, pawn, PAWN_TYPE) != pawn_signature)
            or (vehicle_signature and (
                typed_header(session, base, possessed, VEHICLE_TYPE) != vehicle_signature
                or pointer_field(session, controller, 0x1380) != pawn
                or pointer_field(session, controller, 0x1388) != pawn
                or pointer_field(session, pawn, 0x178) != possessed
                or pointer_field(session, possessed, 0x3E0) != controller))):
        raise RuntimeError("Local ownership changed during sample; projections withheld")
    return {"world": hex(world), "controller": hex(controller),
            "pawn": hex(pawn) if pawn else None, "pawn_signature": pawn_signature,
            "possessed": hex(possessed) if possessed else None,
            "status": state}


def owned_position(session, base, point):
    """Validate a direct character pawn on foot or attached to a typed vehicle."""
    relation = point["structure_relation"]
    pawn = int(relation["address"], 16)
    signature = typed_header(session, base, pawn, PAWN_TYPE)
    if signature != relation["instance_signature"]:
        raise RuntimeError("Pawn instance changed during component capture")
    body = session.read(pawn, 0x410)
    if instance_signature(body) != signature:
        raise RuntimeError("Pawn instance changed during ownership read")
    raw = body[0x140:0x150]
    array, count, capacity = struct.unpack("<Qii", raw)
    if not 0 <= count <= capacity <= 128 or (count and not valid_pointer(array)):
        raise RuntimeError("Current pawn component array has invalid bounds")
    entries = session.read(array, count*8) if count else b""
    addresses = [value[0] for value in struct.iter_unpack("<Q", entries)]
    attachment = struct.unpack_from("<Q", body, 0x178)[0]
    vehicle_signature = None
    component_checks = []
    if valid_pointer(attachment):
        header = session.read(attachment, 0x40)
        if (struct.unpack_from("<Q", header)[0]-base, struct.unpack_from("<I", header, 0x3C)[0]) == VEHICLE_TYPE:
            vehicle_signature = typed_header(session, base, attachment, VEHICLE_TYPE)
    if not vehicle_signature:
        for component in addresses:
            header = session.read(component, 0x40)
            kind = (struct.unpack_from("<Q", header)[0]-base, struct.unpack_from("<I", header, 0x3C)[0])
            if kind in REGISTERED_COMPONENT_TYPES:
                if pointer_field(session, component, 0x178) != pawn:
                    raise RuntimeError("Registered character component points to another pawn")
                component_checks.append((component, instance_signature(header), kind))
        if not component_checks:
            raise RuntimeError("On-foot character has no reciprocal current position component")
    xyz = struct.unpack_from("<fff", body, 0x88)
    if not all(math.isfinite(value) and abs(value) < 1_000_000 for value in xyz):
        raise RuntimeError("Canonical pawn XYZ is invalid")
    controller = struct.unpack_from("<Q", body, 0x3E0)[0]
    if (int(point["address"], 16) != pawn):
        raise RuntimeError("Ownership position source must be the direct typed pawn")
    if (session.read(pawn+0x140, 16) != raw or (count and session.read(array, count*8) != entries)
            or pointer_field(session, pawn, 0x178) != attachment
            or typed_header(session, base, pawn, PAWN_TYPE) != signature
            or pointer_field(session, pawn, 0x3E0) != controller
            or (vehicle_signature and typed_header(session, base, attachment, VEHICLE_TYPE) != vehicle_signature)
            or any(typed_header(session, base, component, kind) != fingerprint
                   or pointer_field(session, component, 0x178) != pawn for component, fingerprint, kind in component_checks)):
        raise RuntimeError("Component ownership changed during sample")
    return {"pawn": hex(pawn), "pawn_signature": signature, "controller": hex(controller),
            "components": [hex(value) for value in addresses], "position": xyz,
            "vehicle_attachment": hex(attachment) if vehicle_signature else None,
            "position_source": "typed_current_pawn_0x88",
            "status": "typed_vehicle_attachment" if vehicle_signature else "reciprocal_component_membership"}


def filter_owned(points, local):
    """No address history: one unambiguous point per current pawn instance."""
    visible, excluded, ambiguous = [], [], []
    groups = defaultdict(list)
    controller = int(local["controller"], 16)
    for point in points:
        owner = point["ownership"]
        if owner["pawn"] == local["pawn"] or int(owner["controller"], 16) == controller:
            excluded.append(point)
        else:
            groups[(owner["pawn"], owner["pawn_signature"])].append(point)
    for (pawn, signature), members in groups.items():
        if len(members) != 1:
            ambiguous.append({"pawn": pawn, "pawn_signature": signature,
                              "components": [p["address"] for p in members],
                              "action": "all_ambiguous_points_withheld"})
        else:
            visible.extend(members)
    return visible, excluded, ambiguous
