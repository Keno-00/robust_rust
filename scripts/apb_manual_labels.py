"""Explicit operator labels for exact observed instances, not an actor classifier."""

import math
import struct


def instance_signature(body):
    # Keep raw object-index/name/class fields as an audit fingerprint. Their
    # semantics are not assumed; mutable flags and hash links are omitted.
    return body[:8].hex() + ":" + body[0x24:0x40].hex()


def instance_key(point):
    return point["address"] + ":" + point["instance_signature"]


class ManualLabels:
    def __init__(self):
        self.labels = {}
        self.undo_stack = []
        self.nearby = []
        self.selected_index = 0
        self.message = "Aim the red centre reticle at a candidate"

    def select(self, points, width, height):
        previous_key = (instance_key(self.nearby[self.selected_index % len(self.nearby)])
                        if self.nearby else None)
        nearby = [p for p in points if p["projection"]["status"] == "inside"
                  and self.labels.get(instance_key(p)) != "exclude"
                  and math.hypot(p["projection"]["client_x"]-width/2,
                                 p["projection"]["client_y"]-height/2) <= 24]
        nearby.sort(key=lambda p: (math.hypot(p["projection"]["client_x"]-width/2,
                                             p["projection"]["client_y"]-height/2), p["address"]))
        keys = [instance_key(p) for p in nearby]
        self.selected_index = keys.index(previous_key) if previous_key in keys else 0
        self.nearby = nearby
        return nearby[self.selected_index % len(nearby)] if nearby else None

    def cycle(self):
        if self.nearby:
            self.selected_index = (self.selected_index+1) % len(self.nearby)

    def assign(self, point, label):
        key = instance_key(point)
        previous = self.labels.get(key)
        self.undo_stack.append((key, previous))
        self.labels[key] = label
        self.message = f"{label.upper()}: {point['address']}"
        return {"instance_key": key, "label": label, "previous_label": previous}

    def undo(self):
        if not self.undo_stack:
            self.message = "Nothing to undo"
            return None
        key, previous = self.undo_stack.pop()
        if previous is None:
            self.labels.pop(key, None)
        else:
            self.labels[key] = previous
        self.message = "Last label undone"
        return {"instance_key": key, "restored_label": previous}


def annotation_evidence(session, base, lead, point, mission=False):
    address = int(point["address"], 16)
    extent = 0x3000 if mission else 0x800
    body = b"".join(session.read(address+offset, min(4096, extent-offset))
                    for offset in range(0, extent, 4096))
    if instance_signature(body) != point["instance_signature"]:
        raise RuntimeError("Selected instance changed; label was not applied")
    refs = []
    seen = set()
    for offset in range(8, 0x1000 if mission else 0x400, 8):
        pointer = struct.unpack_from("<Q", body, offset)[0]
        if pointer in seen or not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
            continue
        seen.add(pointer)
        try:
            target = session.read(pointer, 0x1000 if mission else 0x100)
        except (OSError, RuntimeError) as error:
            refs.append({"offset": hex(offset), "pointer": hex(pointer), "read_error": str(error)})
            continue
        vtable = struct.unpack_from("<Q", target)[0]
        if base <= vtable < base + 0x5380000:
            refs.append({"offset": hex(offset), "pointer": hex(pointer),
                         "vtable_rva": hex(vtable-base), "body_hex": target.hex()})
    if instance_signature(session.read(address, 0x40)) != point["instance_signature"]:
        raise RuntimeError("Selected instance changed while capturing annotation context")
    return {"lead": lead, "body_hex": body.hex(), "pointer_context_unclassified": refs,
            "capture_extent": extent, "extent_status": "bounded_probe_not_verified_object_size"}
