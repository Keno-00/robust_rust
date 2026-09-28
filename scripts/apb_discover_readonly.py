"""Discover the observed APB camera and local position in the current build.

Read-only diagnostic. This does not select targets or send mouse movement.
It fails if the build, object table, or observed object relationships differ.
"""

import argparse
import ctypes
import hashlib
import json
import math
import struct
import sys
from ctypes import wintypes
from datetime import datetime, timezone
from pathlib import Path


EXPECTED_SHA256 = "3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640"
TABLE_RVA = 0x3CFAC90
TABLE_ENTRIES = 32768
CAMERA_CLASS = 16920
CONTROLLER_CLASS = 4159
LOCAL_LINK_CLASS = 6683
POSITION_CLASS = 4741
OWNER_CLASS = 4155
IOCTL_ECHO_READ_PROCESS = 0x226002
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value


class ProcessBasicInformation(ctypes.Structure):
    _fields_ = [
        ("reserved1", ctypes.c_void_p),
        ("peb", ctypes.c_void_p),
        ("reserved2", ctypes.c_void_p * 2),
        ("unique_pid", ctypes.c_size_t),
        ("reserved3", ctypes.c_void_p),
    ]


def executable_identity(kernel32, process):
    path_buffer = ctypes.create_unicode_buffer(32768)
    length = wintypes.DWORD(len(path_buffer))
    if not kernel32.QueryFullProcessImageNameW(process, 0, path_buffer, ctypes.byref(length)):
        raise ctypes.WinError(ctypes.get_last_error())
    path = Path(path_buffer.value)
    if path.name.lower() != "apb.exe":
        raise RuntimeError(f"PID belongs to {path.name}, not APB.exe")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    actual = digest.hexdigest().upper()
    if actual != EXPECTED_SHA256:
        raise RuntimeError(f"Unverified APB build SHA-256 {actual}; expected {EXPECTED_SHA256}")
    return str(path), actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or args.output.exists():
        raise ValueError("PID must be valid and output must not already exist")

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    ntdll = ctypes.WinDLL("ntdll", use_last_error=True)
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.WaitForSingleObject.argtypes = [ctypes.c_void_p, wintypes.DWORD]
    kernel32.WaitForSingleObject.restype = wintypes.DWORD
    kernel32.QueryFullProcessImageNameW.argtypes = [
        ctypes.c_void_p, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)
    ]
    kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
    kernel32.CreateFileW.argtypes = [
        wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
        wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
    ]
    kernel32.CreateFileW.restype = ctypes.c_void_p
    kernel32.DeviceIoControl.argtypes = [
        ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD,
        ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p,
    ]
    kernel32.DeviceIoControl.restype = wintypes.BOOL
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = wintypes.BOOL
    ntdll.NtQueryInformationProcess.argtypes = [
        ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
    ]
    ntdll.NtQueryInformationProcess.restype = ctypes.c_long

    process = kernel32.OpenProcess(0x101000, False, args.pid)
    if not process:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        if kernel32.WaitForSingleObject(process, 0) != 258:
            raise RuntimeError("APB process is not running")
        image_path, image_hash = executable_identity(kernel32, process)
        info = ProcessBasicInformation()
        returned = wintypes.DWORD()
        status = ntdll.NtQueryInformationProcess(
            process, 0, ctypes.byref(info), ctypes.sizeof(info), ctypes.byref(returned)
        )
        if status != 0 or not info.peb:
            raise RuntimeError(f"Cannot locate APB PEB (NTSTATUS 0x{status & 0xFFFFFFFF:08X})")

        device = kernel32.CreateFileW(r"\\.\EchoDevice", 0x80000000, 1, None, 3, 0, None)
        if device in (None, INVALID_HANDLE_VALUE):
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            def read(address, size):
                request = ctypes.create_string_buffer(struct.pack("<IIQ", args.pid, size, address), 16)
                output = ctypes.create_string_buffer(size)
                count = wintypes.DWORD()
                if not kernel32.DeviceIoControl(
                    device, IOCTL_ECHO_READ_PROCESS, request, 16,
                    output, size, ctypes.byref(count), None,
                ):
                    raise ctypes.WinError(ctypes.get_last_error())
                if count.value != size:
                    raise RuntimeError(f"Short read at 0x{address:X}: {count.value}/{size}")
                return output.raw

            (base,) = struct.unpack("<Q", read(info.peb + 0x10, 8))
            if base < 0x10000 or read(base, 2) != b"MZ":
                raise RuntimeError(f"Invalid APB module base from PEB: 0x{base:X}")
            table_address = base + TABLE_RVA
            table = bytearray()
            for index in range(0, TABLE_ENTRIES, 512):
                table.extend(read(table_address + 8 * index, min(512, TABLE_ENTRIES - index) * 8))
            table_addresses = {
                struct.unpack_from("<Q", table, index * 8)[0]
                for index in range(TABLE_ENTRIES)
            }

            cameras = []
            controllers = []
            owners = []
            plausible = readable = failed = 0
            for index in range(TABLE_ENTRIES):
                (address,) = struct.unpack_from("<Q", table, index * 8)
                if not (0x10000 <= address < 0x800000000000 and address % 8 == 0):
                    continue
                plausible += 1
                try:
                    header = read(address, 0x40)
                except OSError:
                    failed += 1
                    continue
                readable += 1
                (class_id,) = struct.unpack_from("<I", header, 0x3C)
                if class_id == CAMERA_CLASS:
                    cameras.append((index, address))
                elif class_id == CONTROLLER_CLASS:
                    controllers.append((index, address))
                elif class_id == OWNER_CLASS:
                    owners.append((index, address))

            if len(cameras) != 1:
                raise RuntimeError(f"Expected one camera-class object, found {len(cameras)}")
            local_chains = []
            for controller_index, controller in controllers:
                data = read(controller + 0x140, 0x288)
                (first,) = struct.unpack_from("<Q", data, 0)
                (local_link,) = struct.unpack_from("<Q", data, 8)
                (second,) = struct.unpack_from("<Q", data, 0x280)
                if first != second or first < 0x10000:
                    continue
                pawn_data = read(first + 0x3C, 0x58)
                (pawn_class,) = struct.unpack_from("<I", pawn_data, 0)
                position = struct.unpack_from("<fff", pawn_data, 0x88 - 0x3C)
                if pawn_class == POSITION_CLASS and all(math.isfinite(v) for v in position):
                    link_class = None
                    if local_link >= 0x10000 and local_link in table_addresses:
                        (link_class,) = struct.unpack("<I", read(local_link + 0x3C, 4))
                    local_chains.append((controller_index, controller, first, position, local_link, link_class))
            camera_index, camera = cameras[0]
            camera_data = read(camera + 0x680, 0x50)
            matrix = struct.unpack_from("<16f", camera_data, 0)
            camera_position = struct.unpack_from("<fff", camera_data, 0x40)
            if not all(math.isfinite(v) for v in (*matrix, *camera_position)):
                raise RuntimeError("Camera matrix or position contains nonfinite values")
            distances = sorted(
                (math.dist(camera_position, position), index, controller, pawn, position, link, link_class)
                for index, controller, pawn, position, link, link_class in local_chains
            )
            local_matches = [item for item in distances if item[6] == LOCAL_LINK_CLASS]
            ambiguous = len(local_matches) != 1 or local_matches[0][0] > 400
            if ambiguous:
                details = [
                    (
                        index, f"0x{controller:X}", f"0x{pawn:X}",
                        tuple(round(value, 1) for value in position), round(distance, 1),
                        f"0x{link:X}", link_class,
                    )
                    for distance, index, controller, pawn, position, link, link_class in distances
                ]
                ambiguity = {
                    "valid": False,
                    "reason": "local controller link is missing, ambiguous, or too far from camera",
                    "pid": args.pid,
                    "camera_address": f"0x{camera:X}",
                    "camera_position": camera_position,
                    "candidates": [
                        {
                            "controller_index": index,
                            "controller_address": f"0x{controller:X}",
                            "position_address": f"0x{pawn:X}",
                            "position": position,
                            "camera_distance": distance,
                            "local_link_address": f"0x{link:X}",
                            "local_link_class": link_class,
                            "controller_bytes_hex": read(controller, 0x800).hex(),
                        }
                        for distance, index, controller, pawn, position, link, link_class in distances
                    ],
                }
                args.output.parent.mkdir(parents=True, exist_ok=True)
                args.output.write_text(json.dumps(ambiguity, indent=2), encoding="utf-8")
                raise RuntimeError(
                    f"Cannot uniquely identify the local controller linked to class {LOCAL_LINK_CLASS} "
                    f"near camera 0x{camera:X}; candidates: {details}"
                )
            separation, controller_index, controller, local_position_address, local_position, local_link, _ = local_matches[0]

            controller_positions = []
            for distance, index, linked_controller, pawn, position, link, link_class in distances:
                relative = tuple(position[i] - camera_position[i] for i in range(3)) + (1.0,)
                clip = tuple(
                    sum(relative[i] * matrix[i * 4 + j] for i in range(4))
                    for j in range(4)
                )
                ndc = (clip[0] / clip[3], clip[1] / clip[3]) if clip[3] > 0 else None
                controller_positions.append({
                    "controller_index": index,
                    "controller_address": f"0x{linked_controller:X}",
                    "position_address": f"0x{pawn:X}",
                    "position": position,
                    "camera_distance": distance,
                    "local_link_address": f"0x{link:X}",
                    "local_link_class": link_class,
                    "is_local": pawn == local_position_address,
                    "clip_w": clip[3],
                    "ndc_xy": ndc,
                    "within_viewport": ndc is not None and all(-1 <= v <= 1 for v in ndc),
                })

            actor_positions = []
            owner_links = []
            owner_read_failures = []
            for owner_index, owner in owners:
                try:
                    data = read(owner + 0xA0, 0x148)
                    linked = (
                        (0xA0, struct.unpack_from("<Q", data, 0)[0]),
                        (0x1E0, struct.unpack_from("<Q", data, 0x140)[0]),
                    )
                    for field, pawn in linked:
                        owner_links.append({
                            "owner_index": owner_index,
                            "owner_address": f"0x{owner:X}",
                            "position_field": f"0x{field:X}",
                            "raw_pointer": f"0x{pawn:X}",
                            "pointer_in_table": pawn in table_addresses,
                        })
                        if pawn < 0x10000 or pawn not in table_addresses:
                            continue
                        pawn_data = read(pawn + 0x3C, 0x58)
                        (pawn_class,) = struct.unpack_from("<I", pawn_data, 0)
                        position = struct.unpack_from("<fff", pawn_data, 0x88 - 0x3C)
                        if pawn_class != POSITION_CLASS or not all(math.isfinite(v) for v in position):
                            continue
                        relative = tuple(position[i] - camera_position[i] for i in range(3)) + (1.0,)
                        clip = tuple(
                            sum(relative[i] * matrix[i * 4 + j] for i in range(4))
                            for j in range(4)
                        )
                        ndc = (clip[0] / clip[3], clip[1] / clip[3]) if clip[3] > 0 else None
                        actor_positions.append({
                            "owner_index": owner_index,
                            "owner_address": f"0x{owner:X}",
                            "position_field": f"0x{field:X}",
                            "position_address": f"0x{pawn:X}",
                            "position": position,
                            "is_local": pawn == local_position_address,
                            "clip_w": clip[3],
                            "ndc_xy": ndc,
                            "within_viewport": ndc is not None and all(-1 <= v <= 1 for v in ndc),
                        })
                except (OSError, RuntimeError) as error:
                    owner_read_failures.append(f"owner 0x{owner:X}: {error}")

            if kernel32.WaitForSingleObject(process, 0) != 258:
                raise RuntimeError("APB exited during discovery")
            for index, address, label in (
                (camera_index, camera, "camera"),
                (controller_index, controller, "controller"),
            ):
                (current,) = struct.unpack("<Q", read(table_address + index * 8, 8))
                if current != address:
                    raise RuntimeError(f"{label} object table slot changed during discovery")

            record = {
                "timestamp_utc": datetime.now(timezone.utc).isoformat(),
                "pid": args.pid,
                "executable": image_path,
                "sha256": image_hash,
                "module_base": f"0x{base:X}",
                "object_table": f"0x{table_address:X}",
                "table_entries": TABLE_ENTRIES,
                "plausible_entries": plausible,
                "readable_headers": readable,
                "failed_headers": failed,
                "camera_index": camera_index,
                "camera_address": f"0x{camera:X}",
                "camera_position": camera_position,
                "matrix_row_major": matrix,
                "controller_index": controller_index,
                "controller_address": f"0x{controller:X}",
                "local_position_address": f"0x{local_position_address:X}",
                "local_link_address": f"0x{local_link:X}",
                "local_position": local_position,
                "camera_local_separation": separation,
                "local_selection": "sole controller with a class-6683 link at +0x148 and position within 400 units of the camera",
                "unclassified_controller_positions": controller_positions,
                "unclassified_actor_positions": actor_positions,
                "owner_class_count": len(owners),
                "owner_links": owner_links,
                "owner_read_failures": owner_read_failures,
            }
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(record, indent=2), encoding="utf-8")
            print(f"Discovered camera 0x{camera:X} and local position 0x{local_position_address:X}; saved {args.output}")
        finally:
            kernel32.CloseHandle(device)
    finally:
        kernel32.CloseHandle(process)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"APB discovery failed: {error}", file=sys.stderr)
        sys.exit(1)
