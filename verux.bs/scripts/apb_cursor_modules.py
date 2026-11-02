"""Inventory APB modules through EchoDriver when Windows module APIs are denied."""

import argparse
import ctypes
import json
import struct
from ctypes import wintypes
from datetime import datetime, timezone
from pathlib import Path

from apb_cursor_scan import Session


class ProcessBasicInformation(ctypes.Structure):
    _fields_ = [
        ("reserved1", ctypes.c_void_p),
        ("peb", ctypes.c_void_p),
        ("reserved2", ctypes.c_void_p * 2),
        ("unique_pid", ctypes.c_size_t),
        ("reserved3", ctypes.c_void_p),
    ]


def read_u64(session, address):
    return struct.unpack("<Q", session.read(address, 8))[0]


def peb_address(session):
    ntdll = ctypes.WinDLL("ntdll", use_last_error=True)
    ntdll.NtQueryInformationProcess.argtypes = [ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    ntdll.NtQueryInformationProcess.restype = ctypes.c_long
    info = ProcessBasicInformation()
    returned = wintypes.DWORD()
    status = ntdll.NtQueryInformationProcess(session.process, 0, ctypes.byref(info), ctypes.sizeof(info), ctypes.byref(returned))
    if status != 0 or not info.peb:
        raise RuntimeError(f"Locate APB PEB failed: NTSTATUS 0x{status & 0xFFFFFFFF:08X}")
    return info.peb


def module_base(session):
    base = read_u64(session, peb_address(session) + 0x10)
    if not 0x10000 <= base < 0x800000000000 or session.read(base, 2) != b"MZ":
        raise RuntimeError(f"Invalid APB module base 0x{base:X}")
    return base


def read_unicode(session, descriptor):
    length, maximum, pointer = struct.unpack("<HH4xQ", descriptor)
    if length > maximum or length > 65534 or length % 2:
        raise RuntimeError("Invalid loader Unicode descriptor")
    return session.read(pointer, length).decode("utf-16-le") if length else ""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output already exists")
    session = Session(args.pid)
    try:
        ldr = read_u64(session, peb_address(session) + 0x18)
        if not 0x10000 <= ldr < 0x800000000000:
            raise RuntimeError(f"Invalid loader address 0x{ldr:X}")
        head = ldr + 0x10
        current = read_u64(session, head)
        modules = []
        visited = set()
        while current != head:
            if current in visited or len(modules) >= 1024 or current < 0x10000:
                raise RuntimeError(f"Invalid or cyclic APB module list at 0x{current:X}")
            visited.add(current)
            entry = session.read(current, 0x68)
            next_entry = struct.unpack_from("<Q", entry, 0)[0]
            base = struct.unpack_from("<Q", entry, 0x30)[0]
            size = struct.unpack_from("<I", entry, 0x40)[0]
            path = read_unicode(session, entry[0x48:0x58])
            name = read_unicode(session, entry[0x58:0x68])
            if not base or size < 0x1000 or size > 0x80000000:
                raise RuntimeError(f"Invalid module {name} at 0x{base:X}, size 0x{size:X}")
            try:
                header = session.read(base, 2)
                header_status = "mz" if header == b"MZ" else f"unexpected:{header.hex()}"
            except (OSError, RuntimeError) as error:
                header_status = f"unreadable:{error}"
            modules.append({"name": name, "base": f"0x{base:X}", "size": size, "path": path, "header": header_status})
            current = next_entry
        report = {
            "format": "apb-modules-echo-v1",
            "created_utc": datetime.now(timezone.utc).isoformat(),
            "identity": session.identity,
            "modules": modules,
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"Recorded {len(modules)} APB modules at {args.output}")
        for module in modules:
            if "apb.exe" in module["name"].lower() or "eossdk" in module["name"].lower():
                print(f"  {module['name']} {module['base']} size=0x{module['size']:X}")
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, UnicodeError, struct.error) as error:
        raise SystemExit(f"EchoDriver module inventory failed: {error}")
