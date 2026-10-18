"""Read-only new-build structural audit; no findings are promoted to production."""

import argparse
import json
import math
import struct
import time
from datetime import datetime, timezone
from pathlib import Path

from apb_cursor_scan import Session
from apb_cursor_modules import module_base
from apb_manual_labels import instance_signature


# Candidates observed in the 2026-10-18 capture, not approved production types.
TYPES = {"hud": (0x2B67088, 16929), "world": (0x316FF28, 430),
         "controller": (0x2A87258, 998), "pawn": (0x2AB7090, 5778),
         "vehicle": (0x2B320A8, 6064)}


def u64(body, offset):
    return struct.unpack_from("<Q", body, offset)[0]


def kind(body, base):
    return u64(body, 0)-base, struct.unpack_from("<I", body, 0x3C)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("profile", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()
    source = json.loads(args.profile.read_text(encoding="utf-8"))
    args.output_directory.mkdir(parents=True, exist_ok=False)
    session = Session(args.pid)
    report = {"utc": datetime.now(timezone.utc).isoformat(), "identity": session.identity,
              "source": str(args.profile.resolve()), "checks": {}, "status": "in_progress"}
    context = {}
    try:
        if source.get("format") != "apb-build-candidates-v1" or source["identity"] != session.identity:
            raise RuntimeError("Profile is not evidence for this exact process/build")
        if session.identity["image_sha256"] != "35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453":
            raise RuntimeError("Audit constants are only candidates for the 2026-10-18 executable")
        base = module_base(session)
        report["module_base"] = hex(base)

        def read(address, size):
            return b"".join(session.read(address+offset, min(4096, size-offset))
                            for offset in range(0, size, 4096))

        def typed(address, label, size=0x40):
            body = read(address, size)
            if kind(body, base) != TYPES[label]:
                raise RuntimeError(f"{label} type mismatch at {hex(address)}: {kind(body, base)}")
            return body

        def check(label, action):
            try:
                result = action()
                report["checks"][label] = {"status": "passed_snapshot", **result}
                print(f"PASS {label}", flush=True)
            except (OSError, RuntimeError, ValueError, struct.error) as error:
                report["checks"][label] = {"status": "failed", "reason": str(error)}
                print(f"FAIL {label}: {error}", flush=True)

        def hud_check():
            root = read(base+0x3DCE240, 16)
            table, count, capacity = struct.unpack("<Qii", root)
            if not 0 < count <= capacity <= 1000000:
                raise RuntimeError("HUD root bounds invalid")
            entries = read(table, 1024)
            matches = []
            for index, (address,) in enumerate(struct.iter_unpack("<Q", entries)):
                if not 0x10000 <= address < 0x800000000000 or address % 8:
                    continue
                header = read(address, 0x40)
                if kind(header, base) == TYPES["hud"]:
                    matches.append((index, address))
            if len(matches) != 1:
                raise RuntimeError(f"Expected unique HUD in 128 slots, found {len(matches)}")
            index, address = matches[0]
            body = typed(address, "hud", 0x700)
            matrix = struct.unpack_from("<16f", body, 0x680)
            xyz = struct.unpack_from("<3f", body, 0x6C0)
            if (not all(math.isfinite(v) for v in (*matrix, *xyz))
                    or abs(matrix[14]+9.99) > 0.02 or abs(matrix[15]) > 0.02):
                raise RuntimeError("HUD projection shape differs")
            if (read(base+0x3DCE240, 16) != root
                    or u64(read(table+index*8, 8), 0) != address
                    or instance_signature(typed(address, "hud")) != instance_signature(body)):
                raise RuntimeError("HUD binding changed during audit")
            context.update(hud=address)
            return {"root_rva": "0x3DCE240", "index": index, "address": hex(address),
                    "matrix": matrix, "position": xyz, "body_hex": body.hex()}

        def ownership_check():
            hud = context["hud"]
            world = u64(read(hud+0x2E0, 8), 0)
            world_body = typed(world, "world", 0x6A8)
            controller = u64(world_body, 0x358)
            controller_body = typed(controller, "controller", 0x1390)
            possessed = u64(controller_body, 0x790)
            if not possessed:
                raise RuntimeError("Controller currently has no possessed object")
            header = read(possessed, 0x40)
            if kind(header, base) == TYPES["pawn"]:
                pawn, state = possessed, "on_foot"
                if u64(read(pawn+0x3E0, 8), 0) != controller:
                    raise RuntimeError("On-foot pawn/controller backlink differs")
            elif kind(header, base) == TYPES["vehicle"]:
                pawn, state = u64(controller_body, 0x1380), "vehicle_possession"
                if (u64(controller_body, 0x1388) != pawn
                        or u64(read(pawn+0x178, 8), 0) != possessed
                        or u64(read(possessed+0x3E0, 8), 0) != controller):
                    raise RuntimeError("Vehicle/local-character reciprocal links disagree")
            else:
                raise RuntimeError(f"Unrecognized possession type: {kind(header, base)}")
            pawn_body = typed(pawn, "pawn", 0x410)
            xyz = struct.unpack_from("<3f", pawn_body, 0x88)
            if u64(pawn_body, 0x2E0) != world or not all(math.isfinite(v) and abs(v) < 1000000 for v in xyz):
                raise RuntimeError("Pawn World/position differs")
            if (u64(read(hud+0x2E0, 8), 0) != world
                    or u64(read(world+0x358, 8), 0) != controller
                    or u64(read(controller+0x790, 8), 0) != possessed
                    or instance_signature(typed(pawn, "pawn")) != instance_signature(pawn_body)):
                raise RuntimeError("Ownership chain changed during audit")
            context.update(world=world, pawn=pawn, controller=controller)
            return {"state": state, "world": hex(world), "controller": hex(controller),
                    "possessed": hex(possessed), "pawn": hex(pawn), "position": xyz,
                    "world_body_hex": world_body.hex(), "controller_body_hex": controller_body.hex(),
                    "pawn_body_hex": pawn_body.hex()}

        def roster_check():
            world = context["world"]
            before = typed(world, "world", 0x494)
            head = u64(before, 0x48C)
            address, nodes, seen = head, [], set()
            while address:
                if address in seen or len(nodes) >= 512:
                    raise RuntimeError("Pawn-list cycle or 512-node bound exceeded")
                seen.add(address)
                body = read(address, 0x3E8)
                node_kind = kind(body, base)
                if node_kind not in (TYPES["pawn"], TYPES["vehicle"]) or u64(body, 0x2E0) != world:
                    raise RuntimeError(f"Unsupported roster node/type/World at {hex(address)}: {node_kind}")
                nodes.append({"address": hex(address), "type": "pawn" if node_kind == TYPES["pawn"] else "vehicle",
                              "next": hex(u64(body, 0x350)), "signature": instance_signature(body),
                              "body_hex": body.hex()})
                address = u64(body, 0x350)
            for node in nodes:
                body = read(int(node["address"], 16), 0x358)
                if (instance_signature(body) != node["signature"] or u64(body, 0x2E0) != world
                        or u64(body, 0x350) != int(node["next"], 16)):
                    raise RuntimeError(f"Roster node changed during verification: {node['address']}")
            after = typed(world, "world", 0x494)
            if u64(after, 0x48C) != head or instance_signature(after) != instance_signature(before):
                raise RuntimeError("Roster World/head changed")
            if hex(context["pawn"]) not in [node["address"] for node in nodes]:
                raise RuntimeError("Current local character missing from rooted roster")
            context["nodes"] = nodes
            return {"head_offset": "0x48C", "next_offset": "0x350", "nodes": nodes,
                    "pawn_count": sum(node["type"] == "pawn" for node in nodes),
                    "vehicle_count": sum(node["type"] == "vehicle" for node in nodes)}

        def collision_check():
            pawn = context["pawn"]
            address = u64(read(pawn+0x248, 8), 0)
            body = read(address, 0x240)
            if not base <= u64(body, 0) < base+0x537A000:
                raise RuntimeError("Collision candidate has no module vtable")
            origin = struct.unpack_from("<3f", body, 0x108)
            extent = struct.unpack_from("<3f", body, 0x114)
            sphere = struct.unpack_from("<f", body, 0x120)[0]
            half_height, radius = struct.unpack_from("<2f", body, 0x234)
            if (u64(body, 0x44) != pawn or u64(body, 0x7C) != pawn
                    or not all(math.isfinite(v) and abs(v) < 1000000 for v in origin)
                    or not all(math.isfinite(v) and v > 0 for v in (*extent, sphere, half_height, radius))
                    or not 10 <= half_height <= 300 or not 1 <= radius <= 150
                    or max(abs(extent[0]-radius), abs(extent[1]-radius), abs(extent[2]-half_height)) > 0.05
                    or abs(sphere-math.sqrt(sum(v*v for v in extent))) > 0.05):
                raise RuntimeError("Collision ownership/geometry does not satisfy product checks")
            after = read(address, 0x240)
            if (instance_signature(after) != instance_signature(body) or after[0x234:0x23C] != body[0x234:0x23C]
                    or u64(read(pawn+0x248, 8), 0) != address
                    or u64(after, 0x44) != pawn or u64(after, 0x7C) != pawn):
                raise RuntimeError("Collision ownership or stance changed during audit")
            return {"address": hex(address), "vtable_rva": hex(kind(body, base)[0]), "name_index": kind(body, base)[1],
                    "origin": origin, "extent": extent, "sphere": sphere,
                    "half_height": half_height, "radius": radius, "body_hex": body.hex(),
                    "type_status": "newly_observed_candidate_not_promoted"}

        def recency_check():
            samples = []
            for index in range(3):
                if index:
                    time.sleep(0.25)
                before = struct.unpack("<d", read(context["world"]+0x6A0, 8))[0]
                values = []
                for node in context["nodes"]:
                    if node["type"] != "pawn":
                        continue
                    body = typed(int(node["address"], 16), "pawn", 0x2E8)
                    if instance_signature(body) != node["signature"] or u64(body, 0x2E0) != context["world"]:
                        raise RuntimeError("Timestamp pawn instance/World changed")
                    values.append({"pawn": node["address"], "last_render": struct.unpack_from("<d", body, 0xB4)[0]})
                after = struct.unpack("<d", read(context["world"]+0x6A0, 8))[0]
                if (not all(math.isfinite(v) and v >= 0 for v in (before, after)) or after < before
                        or samples and before < samples[-1]["world_after"]):
                    raise RuntimeError("World clock is nonfinite/negative or moved backward")
                for value in values:
                    if not math.isfinite(value["last_render"]) or value["last_render"] < 0 or value["last_render"] > after+0.25:
                        raise RuntimeError(f"Render timestamp is inconsistent with World clock: {value}")
                    value["age"] = after-value["last_render"]
                samples.append({"world_before": before, "world_after": after, "pawns": values})
            if samples[-1]["world_after"] <= samples[0]["world_before"]:
                raise RuntimeError("World clock did not advance during the observation")
            return {"samples": samples, "scope": "clock/timestamp consistency only; visibility not established"}

        def mission_check():
            values = []
            for node in context["nodes"]:
                if node["type"] != "pawn":
                    continue
                address = int(node["address"], 16)
                raw = read(address+0x990, 16)
                if read(address+0x990, 16) != raw or instance_signature(typed(address, "pawn")) != node["signature"]:
                    raise RuntimeError("Pawn/mission block changed during read")
                token, side, a, b = struct.unpack("<4I", raw)
                values.append({"pawn": node["address"], "token": hex(token), "side": side,
                               "relation_a": hex(a), "relation_b": hex(b), "raw_hex": raw.hex()})
            return {"values": values, "scope": "stable raw block only; team/mission meaning not established"}

        check("hud_projection_binding", hud_check)
        if "hud" in context:
            check("local_ownership", ownership_check)
        if "pawn" in context:
            check("rooted_roster", roster_check)
            check("collision_bounds", collision_check)
        if "nodes" in context:
            check("render_timestamp_consistency", recency_check)
            check("mission_raw_layout", mission_check)
        report["status"] = "structural_audit_complete_not_behavioral_approval"
        report["unverified"] = ["cursor root and colors", "mission/team behavior", "visibility/occlusion",
                                "vehicle entry/exit/passengers", "death/respawn", "district travel", "full process restart"]
    except BaseException as error:
        report.update(status="audit_failed", failure=str(error))
        raise
    finally:
        session.close()
        with (args.output_directory / "audit.json").open("x", encoding="utf-8") as stream:
            json.dump(report, stream, indent=2, allow_nan=False)


if __name__ == "__main__":
    main()
