"""Explicit experimental mission-role hypothesis from labelled live captures.

Uses the full raw token and observed side values, never faction or distance.
Unknown states remain unknown; this is not a production opponent resolver.
"""

import struct

from apb_character_ownership import PAWN_TYPE, typed_header


TOKEN_OFFSET = 0x990
SIDE_OFFSET = 0x994
NO_MISSION_TOKEN = 0xFFFFFFFF
OBSERVED_INACTIVE_SIDES = {0x80, 0xC9, 0xD9}


def decode(raw):
    token, side, relation_a, relation_b = struct.unpack("<IIII", raw)
    return {"token": token, "token_hex": hex(token), "side": side,
            "raw_998": relation_a, "raw_99c": relation_b}


def read_mission(session, base, pawn, expected_signature):
    signature = typed_header(session, base, pawn, PAWN_TYPE)
    if signature != expected_signature:
        raise RuntimeError("Pawn instance changed before mission-field read")
    raw = session.read(pawn+TOKEN_OFFSET, 16)
    if (session.read(pawn+TOKEN_OFFSET, 16) != raw
            or typed_header(session, base, pawn, PAWN_TYPE) != signature):
        raise RuntimeError("Mission fields changed during sample")
    return decode(raw)


def membership(fields):
    if fields["token"] == NO_MISSION_TOKEN or fields["side"] in OBSERVED_INACTIVE_SIDES:
        return "inactive"
    if fields["side"] in (0, 1) and fields["token"] & 0x80000000:
        return "active"
    return "unknown"


def classify(local, remote):
    local_state, remote_state = membership(local), membership(remote)
    if "unknown" in (local_state, remote_state):
        return {"role": "unknown", "reason": "Unobserved mission token/side state; preserve for inspection"}
    if local_state == "inactive" or remote_state == "inactive":
        return {"role": "outside_mission", "reason": "Observed inactive mission state"}
    if local["token"] != remote["token"]:
        return {"role": "outside_mission", "reason": "Full mission tokens differ"}
    if local["side"] == remote["side"]:
        return {"role": "teammate", "reason": "Same active mission token and side"}
    return {"role": "opposition", "reason": "Same active mission token, opposite observed side"}
