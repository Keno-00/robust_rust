"""EchoDriver v3 AES-256-GCM transport using Windows CNG (no Python crypto dependency)."""
import ctypes
import hmac
import hashlib
import struct
import threading
import winreg
from ctypes import wintypes

IOCTL_ECHO_READ_PROCESS = 0x226002
IOCTL_ECHO_OPEN_SESSION = 0x226004
VERSION = 3
MAX_BYTES = 4096
HEADER_BYTES = 48
FRAME_OVERHEAD = 64
_LOCK = threading.RLock()
_BCRYPT = ctypes.WinDLL("bcrypt", use_last_error=True)
_P = ctypes.c_void_p
_U = wintypes.ULONG

class _Auth(ctypes.Structure):
    _fields_ = [("cbSize", _U), ("dwInfoVersion", _U), ("pbNonce", _P), ("cbNonce", _U),
                ("pbAuthData", _P), ("cbAuthData", _U), ("pbTag", _P), ("cbTag", _U),
                ("pbMacContext", _P), ("cbMacContext", _U), ("cbAAD", _U),
                ("cbData", ctypes.c_ulonglong), ("dwFlags", _U)]

for _name, _args in {
    "BCryptOpenAlgorithmProvider": [ctypes.POINTER(_P), wintypes.LPCWSTR, wintypes.LPCWSTR, _U],
    "BCryptSetProperty": [_P, wintypes.LPCWSTR, _P, _U, _U],
    "BCryptGenerateSymmetricKey": [_P, ctypes.POINTER(_P), _P, _U, _P, _U, _U],
    "BCryptEncrypt": [_P, _P, _U, _P, _P, _U, _P, _U, ctypes.POINTER(_U), _U],
    "BCryptDecrypt": [_P, _P, _U, _P, _P, _U, _P, _U, ctypes.POINTER(_U), _U],
    "BCryptDestroyKey": [_P], "BCryptCloseAlgorithmProvider": [_P, _U],
}.items():
    getattr(_BCRYPT, _name).argtypes = _args
    getattr(_BCRYPT, _name).restype = ctypes.c_long

def _check(status, operation):
    if status < 0:
        raise RuntimeError(f"EchoDriver {operation} failed (NTSTATUS 0x{status & 0xffffffff:08X}); check v3 transport key/authentication")

def _crypt(header: bytes, payload: bytes, tag=None):
    """One-shot CNG; close key handles and wipe key/plaintext scratch on every path."""
    algorithm, key = _P(), _P()
    secret = ctypes.create_string_buffer(32)
    source = ctypes.create_string_buffer(payload, len(payload))
    output = ctypes.create_string_buffer(len(payload))
    nonce = ctypes.create_string_buffer(header[32:44], 12)
    aad = ctypes.create_string_buffer(header, len(header))
    tag_buffer = ctypes.create_string_buffer(tag if tag is not None else bytes(16), 16)
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                r"SYSTEM\CurrentControlSet\Services\EchoDriver\Parameters", 0,
                winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as registry:
            raw, kind = winreg.QueryValueEx(registry, "TransportKey")
        if kind != winreg.REG_BINARY or len(raw) != 32:
            raise RuntimeError("Invalid EchoDriver TransportKey; provision a 32-byte registry key")
        derived = hmac.new(raw, b"EchoDriver-v3-session\0" + header[8:24], hashlib.sha256).digest()
        ctypes.memmove(secret, derived, 32)
        del derived
        del raw
        _check(_BCRYPT.BCryptOpenAlgorithmProvider(ctypes.byref(algorithm), "AES", None, 0), "AES initialization")
        mode = ctypes.create_unicode_buffer("ChainingModeGCM")
        _check(_BCRYPT.BCryptSetProperty(algorithm, "ChainingMode", mode, ctypes.sizeof(mode), 0), "GCM initialization")
        _check(_BCRYPT.BCryptGenerateSymmetricKey(algorithm, ctypes.byref(key), None, 0, secret, 32, 0), "key loading")
        auth = _Auth()
        auth.cbSize, auth.dwInfoVersion = ctypes.sizeof(_Auth), 1
        auth.pbNonce, auth.cbNonce = ctypes.addressof(nonce), 12
        auth.pbAuthData, auth.cbAuthData = ctypes.addressof(aad), len(header)
        auth.pbTag, auth.cbTag = ctypes.addressof(tag_buffer), 16
        written = _U()
        operation = _BCRYPT.BCryptEncrypt if tag is None else _BCRYPT.BCryptDecrypt
        _check(operation(key, source, len(payload), ctypes.byref(auth), None, 0, output,
                         len(payload), ctypes.byref(written), 0), "encryption" if tag is None else "authentication")
        if written.value != len(payload):
            raise RuntimeError("EchoDriver AES output length mismatch")
        return output.raw, tag_buffer.raw
    except OSError as error:
        raise RuntimeError("EchoDriver transport key unavailable; run drayebr/scripts/initialize-transport-key.ps1 as Administrator and load the v3 driver") from error
    finally:
        if key.value:
            _BCRYPT.BCryptDestroyKey(key)
        if algorithm.value:
            _BCRYPT.BCryptCloseAlgorithmProvider(algorithm, 0)
        for buffer in (secret, source, output):
            ctypes.memset(buffer, 0, ctypes.sizeof(buffer))

def _ioctl(kernel32, device, code, request_bytes, output_size):
    request = ctypes.create_string_buffer(request_bytes, len(request_bytes)) if request_bytes else None
    response = ctypes.create_string_buffer(output_size)
    returned = wintypes.DWORD()
    if not kernel32.DeviceIoControl(device, code, request, len(request_bytes), response,
                                   output_size, ctypes.byref(returned), None):
        error = ctypes.get_last_error()
        raise OSError(f"EchoDriver encrypted v3 IOCTL failed: {ctypes.WinError(error)}. Check matching driver, key and range.")
    if returned.value != output_size:
        raise RuntimeError(f"EchoDriver v3 length mismatch: {returned.value}/{output_size}")
    return response.raw

def read_exact(kernel32, device, process_id: int, address: int, size: int) -> bytes:
    if not (0 < process_id < 2**32 and 0x10000 <= address < 2**64 and
            0 < size <= MAX_BYTES and address + size <= 2**64):
        raise ValueError("Invalid EchoDriver process-read request")
    # Query the session on every read so a reused numeric handle cannot reuse stale state.
    # This query never resets an established session or its replay counter.
    with _LOCK:
        hello = _ioctl(kernel32, device, IOCTL_ECHO_OPEN_SESSION, b"", 32)
        version, reserved, session, last = struct.unpack("<II16sQ", hello)
        if version != VERSION or reserved != 0 or last >= 0xFFFFFF:
            raise RuntimeError("EchoDriver v3 session mismatch/exhausted; reconnect with matching driver")
        sequence = last + 1
        header = struct.pack("<II16sQ12sI", VERSION, 1, session, sequence, struct.pack("<IQ", 1, sequence), 16)
        ciphertext, tag = _crypt(header, struct.pack("<IIQ", process_id, size, address))
        response = _ioctl(kernel32, device, IOCTL_ECHO_READ_PROCESS, header + tag + ciphertext, FRAME_OVERHEAD + size)
        reply_header = response[:HEADER_BYTES]
        version, direction, reply_session, reply_sequence, nonce, actual_size = struct.unpack("<II16sQ12sI", reply_header)
        if (version, direction, reply_session, reply_sequence, actual_size) != (VERSION, 2, session, sequence, size):
            raise RuntimeError("EchoDriver v3 response session/sequence/header mismatch")
        if nonce != struct.pack("<IQ", 2, sequence):
            raise RuntimeError("EchoDriver v3 response nonce mismatch")
        plaintext, _ = _crypt(reply_header, response[FRAME_OVERHEAD:], response[HEADER_BYTES:FRAME_OVERHEAD])
        return plaintext
