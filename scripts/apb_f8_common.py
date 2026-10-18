"""Shared read-only capture primitives for the camera and witness scans."""

import ctypes
import hashlib
import json
import math
import struct
from collections import Counter
from ctypes import wintypes
from datetime import datetime, timezone
from pathlib import Path

from PIL import ImageGrab
import numpy as np

from apb_cursor_modules import module_base
from apb_cursor_scan import Session
from apb_cursor_unknown import wait_f8


TABLE_RVA = 0x3CFAC90
TABLE_ENTRIES = 32768
CAMERA_CLASS = 16920
CONTROLLER_CLASS = 4159
POSITION_CLASS = 4741
OWNER_CLASS = 4155
INTERESTING_CLASSES = {CAMERA_CLASS, CONTROLLER_CLASS, POSITION_CLASS, OWNER_CLASS}


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class Rect(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long), ("right", ctypes.c_long), ("bottom", ctypes.c_long)]


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def write_json(path, value):
    path = Path(path)
    if path.exists():
        raise RuntimeError(f"Refusing to replace existing report: {path}")
    temporary = Path(str(path) + ".partial")
    if temporary.exists():
        raise RuntimeError(f"Partial report already exists: {temporary}")
    with temporary.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
    temporary.replace(path)


def client_capture(session, path, include_layered_windows=False):
    """Record a foreground APB client image and its real screen rectangle."""
    hwnd = session.require_foreground()
    user32 = session.user32
    user32.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
    user32.GetClientRect.restype = wintypes.BOOL
    user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(Point)]
    user32.ClientToScreen.restype = wintypes.BOOL
    rect = Rect()
    origin = Point()
    if not user32.GetClientRect(hwnd, ctypes.byref(rect)) or not user32.ClientToScreen(hwnd, ctypes.byref(origin)):
        raise ctypes.WinError(ctypes.get_last_error())
    width = rect.right - rect.left
    height = rect.bottom - rect.top
    if width < 320 or height < 240:
        raise RuntimeError(f"Invalid APB client size: {width}x{height}")
    before = utc_now()
    image = ImageGrab.grab(bbox=(origin.x, origin.y, origin.x + width, origin.y + height),
                           all_screens=True, include_layered_windows=include_layered_windows)
    after = utc_now()
    session.require_foreground()
    if image.size != (width, height):
        raise RuntimeError(f"Client capture size {image.size} differs from {width}x{height}")
    if path.exists():
        raise RuntimeError(f"Refusing to replace existing image: {path}")
    image.save(path, format="PNG")
    extrema = image.convert("RGB").getextrema()
    if all(low == high for low, high in extrema):
        raise RuntimeError(f"APB client screenshot is uniform; inspect {path} and check capture permissions/window mode")
    return {
        "path": str(path), "before_utc": before, "after_utc": after,
        "client_width": width, "client_height": height,
        "screen_left": origin.x, "screen_top": origin.y,
        "image_extrema": extrema,
    }


def capture_table(session, base, prefix, entries=TABLE_ENTRIES):
    """Keep the observed table and readable headers, without naming the table GObjects."""
    table_path = Path(str(prefix) + ".table.bin")
    headers_path = Path(str(prefix) + ".headers.bin")
    if table_path.exists() or headers_path.exists():
        raise RuntimeError(f"Raw table output already exists for {prefix}")
    table = bytearray()
    address = base + TABLE_RVA
    if not 1 <= entries <= 131072:
        raise ValueError(f"Table entry scan count is outside the diagnostic limit: {entries}")
    for index in range(0, entries, 512):
        table.extend(session.read(address + index * 8, min(512, entries - index) * 8))
    table_path.write_bytes(table)
    objects = {kind: [] for kind in INTERESTING_CLASSES}
    classes = Counter()
    readable = failed = 0
    with headers_path.open("xb") as stream:
        for index in range(entries):
            (object_address,) = struct.unpack_from("<Q", table, index * 8)
            if not 0x10000 <= object_address < 0x800000000000 or object_address % 8:
                continue
            try:
                header = session.read(object_address, 0x40)
            except (OSError, RuntimeError):
                failed += 1
                continue
            stream.write(struct.pack("<IQ", index, object_address) + header)
            readable += 1
            (kind,) = struct.unpack_from("<I", header, 0x3C)
            classes[kind] += 1
            if kind in objects:
                objects[kind].append(object_address)
    session.require_foreground()
    return objects, {
        "table_address": f"0x{address:X}", "entries": entries,
        "table_path": str(table_path), "table_sha256": hashlib.sha256(table).hexdigest(),
        "headers_path": str(headers_path), "readable_headers": readable,
        "failed_headers": failed,
        "class_counts": dict(classes.most_common(30)),
    }


def capture_matrix_blocks(session, prefix):
    """Capture +0x680..+0x6CF from every readable table object, without a class assumption."""
    headers_path = Path(str(prefix) + ".headers.bin")
    output = Path(str(prefix) + ".matrix680.bin")
    if output.exists():
        raise RuntimeError(f"Matrix-block output already exists: {output}")
    records = failed = 0
    with headers_path.open("rb") as source, output.open("xb") as stream:
        while raw := source.read(0x4C):
            if len(raw) != 0x4C:
                raise RuntimeError(f"Truncated table-header record in {headers_path}")
            index, address = struct.unpack_from("<IQ", raw, 0)
            (marker,) = struct.unpack_from("<I", raw, 0xC + 0x3C)
            try:
                block = session.read(address + 0x680, 0x50)
            except (OSError, RuntimeError):
                failed += 1
                continue
            stream.write(struct.pack("<IQI", index, address, marker) + block)
            records += 1
    session.require_foreground()
    return {"path": str(output), "record_bytes": 0x60,
            "data_offset": "0x680", "data_bytes": 0x50,
            "records": records, "failed_reads": failed}


def capture_object_prefixes(session, prefix):
    """Save first 0x100 bytes of all readable table objects for offline field search."""
    headers_path = Path(str(prefix) + ".headers.bin")
    output = Path(str(prefix) + ".objects100.bin")
    if output.exists():
        raise RuntimeError(f"Object-prefix output already exists: {output}")
    records = failed = 0
    with headers_path.open("rb") as source, output.open("xb") as stream:
        while raw := source.read(0x4C):
            if len(raw) != 0x4C:
                raise RuntimeError(f"Truncated table-header record in {headers_path}")
            index, address = struct.unpack_from("<IQ", raw, 0)
            try:
                body = session.read(address, 0x100)
            except (OSError, RuntimeError):
                failed += 1
                continue
            stream.write(struct.pack("<IQ", index, address) + body)
            records += 1
    session.require_foreground()
    return {"path": str(output), "record_bytes": 0x10C, "body_bytes": 0x100,
            "records": records, "failed_reads": failed}


def position_at(session, address):
    data = session.read(address + 0x3C, 0x58)
    kind = struct.unpack_from("<I", data, 0)[0]
    if kind != POSITION_CLASS:
        raise RuntimeError(f"0x{address:X} has class marker {kind}, expected {POSITION_CLASS}")
    xyz = struct.unpack_from("<fff", data, 0x88 - 0x3C)
    if not all(math.isfinite(v) and abs(v) < 1_000_000 for v in xyz):
        raise RuntimeError(f"Invalid position at 0x{address:X}: {xyz}")
    return xyz


def candidate_positions(session, objects):
    """Include direct table entries and observed controller/owner references."""
    addresses = set(objects[POSITION_CLASS])
    links = []
    body_errors = []
    for kind, offsets, size in (
        (CONTROLLER_CLASS, (0x140, 0x3C0), 0x3C8),
        (OWNER_CLASS, (0xA0, 0x1E0), 0x1E8),
    ):
        for owner in objects[kind]:
            try:
                body = session.read(owner, size)
            except (OSError, RuntimeError) as error:
                body_errors.append({"address": f"0x{owner:X}", "error": str(error)})
                continue
            for offset in offsets:
                (target,) = struct.unpack_from("<Q", body, offset)
                if 0x10000 <= target < 0x800000000000 and target % 8 == 0:
                    links.append({"source": f"0x{owner:X}", "source_marker": kind,
                                  "offset": f"0x{offset:X}", "target": f"0x{target:X}"})
                    addresses.add(target)
    positions = {}
    rejected = []
    for address in sorted(addresses):
        try:
            positions[f"0x{address:X}"] = position_at(session, address)
        except (OSError, RuntimeError) as error:
            rejected.append({"address": f"0x{address:X}", "error": str(error)})
    return positions, links, rejected, body_errors


def camera_at(session, address):
    body = session.read(address, 0x700)
    kind = struct.unpack_from("<I", body, 0x3C)[0]
    if kind != CAMERA_CLASS:
        raise RuntimeError(f"Camera candidate 0x{address:X} changed marker to {kind}")
    matrix = struct.unpack_from("<16f", body, 0x680)
    position = struct.unpack_from("<fff", body, 0x6C0)
    if not all(math.isfinite(v) for v in (*matrix, *position)):
        raise RuntimeError(f"Nonfinite camera data at 0x{address:X}")
    return {"address": f"0x{address:X}", "matrix": matrix, "position": position,
            "body_hex": body.hex()}


def project(camera, xyz, width, height):
    rel = [xyz[i] - camera["position"][i] for i in range(3)] + [1.0]
    clip = [sum(rel[row] * camera["matrix"][row * 4 + col] for row in range(4)) for col in range(4)]
    if not all(math.isfinite(v) for v in clip):
        return {"status": "invalid_clip", "clip": clip}
    if clip[3] <= 0:
        return {"status": "behind", "clip_w": clip[3]}
    x, y = clip[0] / clip[3], clip[1] / clip[3]
    return {"status": "inside" if abs(x) <= 1 and abs(y) <= 1 else "outside",
            "client_x": (x + 1) * width / 2, "client_y": (1 - y) * height / 2,
            "ndc": [x, y], "clip_w": clip[3]}


def scan_pointer_refs(session, targets, base):
    """Bounded exact-pointer search of readable writable pages in this launch."""
    wanted = {int(address, 16) for address in targets}
    if not wanted:
        raise RuntimeError("No target object addresses for pointer-reference scan")
    (pe_offset,) = struct.unpack("<I", session.read(base + 0x3C, 4))
    if pe_offset > 0x1000 or session.read(base + pe_offset, 4) != b"PE\0\0":
        raise RuntimeError("Cannot validate APB PE header for pointer-reference scan")
    (image_size,) = struct.unpack("<I", session.read(base + pe_offset + 0x50, 4))
    if not 0x1000 <= image_size <= 0x80000000:
        raise RuntimeError(f"Invalid APB SizeOfImage: 0x{image_size:X}")
    hits = []
    stats = {"regions": 0, "pages_read": 0, "pages_failed": 0, "bytes_read": 0}
    for start, end in session.writable_regions():
        stats["regions"] += 1
        for address in range(start, end, 4096):
            size = min(4096, end - address)
            try:
                data = session.read(address, size)
            except (OSError, RuntimeError):
                stats["pages_failed"] += 1
                continue
            stats["pages_read"] += 1
            stats["bytes_read"] += size
            aligned = size // 8 * 8
            values = np.frombuffer(data[:aligned], dtype="<u8")
            for index in np.flatnonzero(np.isin(values, list(wanted))):
                source = address + int(index) * 8
                if source % 8:
                    continue
                target = int(values[index])
                hits.append({"source": f"0x{source:X}", "target": f"0x{target:X}",
                             "source_module_rva": f"0x{source - base:X}"
                             if base <= source < base + image_size else None})
                if len(hits) > 10000:
                    raise RuntimeError("More than 10,000 exact pointer references; narrow the targets")
        if stats["regions"] % 200 == 0:
            session.require_foreground()
            print(f"  Pointer refs: {stats['regions']} regions, {stats['bytes_read'] // (1024 * 1024)} MiB read", flush=True)
    session.require_foreground()
    return {"targets": [f"0x{target:X}" for target in sorted(wanted)],
            "module_size": image_size,
            "hits": hits, **stats}
