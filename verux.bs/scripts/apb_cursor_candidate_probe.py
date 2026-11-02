"""Explicit white/green/red observations of bounded new-build cursor-path candidates."""

import struct

import numpy as np


class CursorCandidateProbe:
    # Prior root shifted by the observed HUD-root delta; the window is a
    # recorded search boundary, not a claim that all globals shift equally.
    START_RVA = 0x3BB3360
    END_RVA = 0x3BF3360
    FIELD_OFFSET = 0x134

    def __init__(self, session, base):
        self.session, self.base = session, base
        self.candidates = None
        self.phases = []

    def capture(self, label):
        if label not in ("white", "green", "red"):
            raise ValueError("Cursor label must be white, green, or red")
        self.session.require_foreground()
        failures = []
        if self.candidates is None:
            data = b"".join(self.session.read(self.base+rva, min(4096, self.END_RVA-rva))
                            for rva in range(self.START_RVA, self.END_RVA, 4096))
            words = np.frombuffer(data, dtype="<u8")
            mask = (words >= 0x10000) & (words < 0x800000000000) & (words % 8 == 0)
            mask &= (words < self.base) | (words >= self.base+0x537A000)
            sampled, values = {}, {}
            for index in np.flatnonzero(mask):
                pointer = int(words[index])
                if pointer not in values:
                    try:
                        values[pointer] = struct.unpack("<I", self.session.read(pointer+self.FIELD_OFFSET, 4))[0]
                    except OSError as error:
                        failures.append({"pointer": hex(pointer), "error": str(error)})
                        values[pointer] = None
                value = values[pointer]
                if value is not None and value <= 65535:
                    sampled[self.START_RVA+int(index)*8] = {"samples": [(label, value)], "pointer": hex(pointer)}
            updated = sampled
        else:
            remaining = {}
            for rva, candidate in self.candidates.items():
                pointer = struct.unpack("<Q", self.session.read(self.base+rva, 8))[0]
                if not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
                    failures.append({"rva": hex(rva), "error": "Root pointer unavailable"})
                    continue
                value = struct.unpack("<I", self.session.read(pointer+self.FIELD_OFFSET, 4))[0]
                previous = candidate["samples"]
                if (any(old == label and old_value != value for old, old_value in previous)
                        or any(old != label and old_value == value for old, old_value in previous)
                        or value > 65535):
                    continue
                remaining[rva] = {"samples": [*previous, (label, value)], "pointer": hex(pointer)}
            updated = remaining
        self.session.require_foreground()
        self.candidates = updated
        self.phases.append(label)
        result = {"label": label, "phases": list(self.phases),
                  "search_rva_start": hex(self.START_RVA), "search_rva_end": hex(self.END_RVA),
                  "field_offset": hex(self.FIELD_OFFSET), "candidate_count": len(self.candidates),
                  "read_failures": failures,
                  "candidates": [{"root_rva": hex(rva), **candidate} for rva, candidate in self.candidates.items()],
                  "scope": "state-correlated candidates; uncaptured colors and restart stability unverified"}
        return result
