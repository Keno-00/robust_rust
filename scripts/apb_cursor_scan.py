"""Read-only EchoDriver scan for a changing APB cursor-state integer.

Run from an elevated shell while APB is open. Capture a white cursor first,
then filter the saved candidates while the cursor is green and red. Results
are diagnostic only; no address is accepted by the product from this scan.
"""

import argparse
import ctypes
import hashlib
import json
import os
import struct
import sys
from ctypes import wintypes
from datetime import datetime, timezone
from pathlib import Path
from echo_read_protocol import read_exact as echo_read_exact


EXPECTED_SHA256 = "3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640"
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value
PAGE_SIZE = 4096
MAX_ADDRESS = 0x800000000000
MEM_COMMIT = 0x1000
PAGE_GUARD = 0x100
PAGE_NOACCESS = 0x01
WRITABLE = {0x04, 0x08, 0x40, 0x80}


def verify_build_hash(image_hash):
    if image_hash == EXPECTED_SHA256:
        return
    if os.environ.get("APB_SCAN_ALLOW_UNVERIFIED_BUILD") != "1":
        raise RuntimeError(f"Unverified APB.exe SHA-256 {image_hash}; expected {EXPECTED_SHA256}")
    print(
        f"WARNING: APB_SCAN_ALLOW_UNVERIFIED_BUILD=1; discovery hash gate bypassed. "
        f"Actual SHA-256 {image_hash}; verified SHA-256 {EXPECTED_SHA256}. "
        "Offsets and field meanings remain unverified for this build.",
        file=sys.stderr, flush=True,
    )


class FileTime(ctypes.Structure):
    _fields_ = [("low", wintypes.DWORD), ("high", wintypes.DWORD)]


class MemoryBasicInformation(ctypes.Structure):
    _fields_ = [
        ("base", ctypes.c_void_p),
        ("allocation_base", ctypes.c_void_p),
        ("allocation_protect", wintypes.DWORD),
        ("partition_id", wintypes.WORD),
        ("padding", wintypes.WORD),
        ("region_size", ctypes.c_size_t),
        ("state", wintypes.DWORD),
        ("protect", wintypes.DWORD),
        ("kind", wintypes.DWORD),
        ("padding2", wintypes.DWORD),
    ]


class ForegroundLost(RuntimeError):
    """APB is running, but its window is no longer foreground."""


class Session:
    def __init__(self, pid):
        self.pid = pid
        self.kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k = self.kernel32
        k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        k.OpenProcess.restype = ctypes.c_void_p
        k.QueryFullProcessImageNameW.argtypes = [ctypes.c_void_p, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
        k.QueryFullProcessImageNameW.restype = wintypes.BOOL
        k.GetProcessTimes.argtypes = [ctypes.c_void_p] + [ctypes.POINTER(FileTime)] * 4
        k.GetProcessTimes.restype = wintypes.BOOL
        k.VirtualQueryEx.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(MemoryBasicInformation), ctypes.c_size_t]
        k.VirtualQueryEx.restype = ctypes.c_size_t
        k.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p]
        k.CreateFileW.restype = ctypes.c_void_p
        k.DeviceIoControl.argtypes = [ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        k.DeviceIoControl.restype = wintypes.BOOL
        k.CloseHandle.argtypes = [ctypes.c_void_p]
        k.CloseHandle.restype = wintypes.BOOL
        k.WaitForSingleObject.argtypes = [ctypes.c_void_p, wintypes.DWORD]
        k.WaitForSingleObject.restype = wintypes.DWORD
        self.user32 = ctypes.WinDLL("user32", use_last_error=True)
        self.user32.GetForegroundWindow.restype = ctypes.c_void_p
        self.user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD)]
        self.user32.GetWindowThreadProcessId.restype = wintypes.DWORD

        self.process = k.OpenProcess(0x101400, False, pid)
        if not self.process:
            raise ctypes.WinError(ctypes.get_last_error())
        self.device = None
        try:
            name = ctypes.create_unicode_buffer(32768)
            size = wintypes.DWORD(len(name))
            if not k.QueryFullProcessImageNameW(self.process, 0, name, ctypes.byref(size)):
                raise ctypes.WinError(ctypes.get_last_error())
            image = Path(name.value)
            if image.name.lower() != "apb.exe":
                raise RuntimeError(f"PID {pid} is {image.name}, not APB.exe")
            digest = hashlib.sha256()
            with image.open("rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(block)
            image_hash = digest.hexdigest().upper()
            verify_build_hash(image_hash)
            times = [FileTime() for _ in range(4)]
            if not k.GetProcessTimes(self.process, *(ctypes.byref(t) for t in times)):
                raise ctypes.WinError(ctypes.get_last_error())
            self.identity = {
                "pid": pid,
                "created_filetime": (times[0].high << 32) | times[0].low,
                "image_path": str(image),
                "image_sha256": image_hash,
            }
            if image_hash != EXPECTED_SHA256:
                self.identity["unverified_build_discovery"] = True
            self.device = k.CreateFileW(r"\\.\EchoDevice", 0x80000000, 1, None, 3, 0, None)
            if self.device in (None, INVALID_HANDLE_VALUE):
                raise RuntimeError(f"Open EchoDevice failed: {ctypes.WinError(ctypes.get_last_error())}. Start EchoDriver and run elevated.")
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.device not in (None, INVALID_HANDLE_VALUE):
            self.kernel32.CloseHandle(self.device)
            self.device = None
        if self.process:
            self.kernel32.CloseHandle(self.process)
            self.process = None

    def read(self, address, size):
        return echo_read_exact(self.kernel32, self.device, self.pid, address, size)

    def require_foreground(self):
        if self.kernel32.WaitForSingleObject(self.process, 0) != 258:
            raise RuntimeError("APB process exited during scan")
        window = self.user32.GetForegroundWindow()
        active_pid = wintypes.DWORD()
        if not window:
            raise ForegroundLost("No foreground window during focus transition; refocus APB")
        if not self.user32.GetWindowThreadProcessId(window, ctypes.byref(active_pid)):
            raise RuntimeError("Cannot determine the foreground window process")
        if active_pid.value != self.pid:
            raise ForegroundLost(f"APB is not foreground (active PID {active_pid.value}); refocus the game before capture")
        return window

    def writable_regions(self):
        address = 0x10000
        while address < MAX_ADDRESS:
            info = MemoryBasicInformation()
            size = self.kernel32.VirtualQueryEx(self.process, ctypes.c_void_p(address), ctypes.byref(info), ctypes.sizeof(info))
            if size == 0:
                error = ctypes.get_last_error()
                if error == 87:  # End of address space.
                    break
                raise RuntimeError(f"VirtualQueryEx failed at 0x{address:X}: {ctypes.WinError(error)}")
            if size != ctypes.sizeof(info) or not info.region_size:
                raise RuntimeError(f"VirtualQueryEx returned an invalid region at 0x{address:X}")
            end = info.base + info.region_size
            if end <= address:
                raise RuntimeError(f"VirtualQueryEx did not advance at 0x{address:X}")
            if info.state == MEM_COMMIT and not (info.protect & PAGE_GUARD) and not (info.protect & PAGE_NOACCESS) and (info.protect & 0xFF) in WRITABLE:
                yield info.base, end
            address = end


def state_paths(prefix):
    return Path(str(prefix) + ".bin"), Path(str(prefix) + ".json")


def save_scan(prefix, identity, value, source, scan):
    binary, metadata = state_paths(prefix)
    if binary.exists() or metadata.exists():
        raise RuntimeError(f"Output already exists: {binary} or {metadata}")
    binary.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(str(binary) + ".partial")
    if temporary.exists():
        raise RuntimeError(f"Partial scan output exists: {temporary}")
    try:
        with temporary.open("wb") as stream:
            stats = scan(stream)
        temporary.replace(binary)
        record = {
            "format": "apb-cursor-candidates-v1",
            "captured_utc": datetime.now(timezone.utc).isoformat(),
            "identity": identity,
            "value": value,
            "source": str(source) if source else None,
            **stats,
        }
        metadata.write_text(json.dumps(record, indent=2), encoding="utf-8")
        print(json.dumps(record, indent=2))
        print(f"Candidates: {binary}")
    finally:
        temporary.unlink(missing_ok=True)


def first_scan(session, value, stream):
    session.require_foreground()
    needle = struct.pack("<I", value)
    stats = {"regions": 0, "pages_read": 0, "pages_failed": 0, "bytes_read": 0, "candidate_count": 0}
    for start, end in session.writable_regions():
        stats["regions"] += 1
        for address in range(start, end, PAGE_SIZE):
            size = min(PAGE_SIZE, end - address)
            try:
                data = session.read(address, size)
            except (OSError, RuntimeError):
                stats["pages_failed"] += 1
                continue
            stats["pages_read"] += 1
            stats["bytes_read"] += size
            offset = data.find(needle)
            while offset >= 0:
                candidate = address + offset
                if candidate % 4 == 0:
                    stream.write(struct.pack("<Q", candidate))
                    stats["candidate_count"] += 1
                offset = data.find(needle, offset + 1)
        if stats["regions"] % 100 == 0:
            session.require_foreground()
            print(f"Scanned {stats['regions']} regions, {stats['bytes_read'] // (1024 * 1024)} MiB, {stats['candidate_count']} candidates", file=sys.stderr, flush=True)
    session.require_foreground()
    return stats


def next_scan(session, source, value, stream):
    session.require_foreground()
    binary, metadata = state_paths(source)
    prior = json.loads(metadata.read_text(encoding="utf-8"))
    if prior.get("format") != "apb-cursor-candidates-v1" or prior.get("identity") != session.identity:
        raise RuntimeError("Candidate file belongs to another APB process/build or has an unknown format")
    if binary.stat().st_size != prior["candidate_count"] * 8:
        raise RuntimeError("Candidate file length does not match its metadata")
    stats = {"pages_read": 0, "pages_failed": 0, "candidate_count": 0, "previous_candidates": prior["candidate_count"], "observed_samples": []}
    with binary.open("rb") as candidates:
        page_address = None
        page_data = None
        previous = -1
        while raw := candidates.read(8):
            (address,) = struct.unpack("<Q", raw)
            if address <= previous or address % 4:
                raise RuntimeError(f"Candidate list is unsorted or unaligned at 0x{address:X}")
            previous = address
            page = address & ~(PAGE_SIZE - 1)
            if page != page_address:
                page_address = page
                try:
                    page_data = session.read(page, PAGE_SIZE)
                    stats["pages_read"] += 1
                except (OSError, RuntimeError):
                    page_data = None
                    stats["pages_failed"] += 1
            if page_data is not None:
                (observed,) = struct.unpack_from("<I", page_data, address - page)
                if len(stats["observed_samples"]) < 16:
                    stats["observed_samples"].append({"address": f"0x{address:X}", "value": observed})
                if observed == value:
                    stream.write(raw)
                    stats["candidate_count"] += 1
    session.require_foreground()
    return stats


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    parser.add_argument("phase", choices=("first", "next"))
    parser.add_argument("value", type=int, help="Observed four-byte cursor value (0..65535)")
    parser.add_argument("output", type=Path, help="Unique output prefix (creates .bin and .json)")
    parser.add_argument("--from", dest="source", type=Path, help="Previous scan prefix; required for next")
    args = parser.parse_args()
    if not 0 < args.pid < 2**32 or not 0 <= args.value <= 65535 or (args.phase == "next") != bool(args.source):
        parser.error("Use a valid PID, value 0..65535, and --from only with next")
    session = Session(args.pid)
    try:
        if args.phase == "first":
            save_scan(args.output, session.identity, args.value, None, lambda stream: first_scan(session, args.value, stream))
        else:
            save_scan(args.output, session.identity, args.value, args.source, lambda stream: next_scan(session, args.source, args.value, stream))
    finally:
        session.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, struct.error) as error:
        print(f"APB cursor scan failed: {error}", file=sys.stderr)
        raise SystemExit(1)
