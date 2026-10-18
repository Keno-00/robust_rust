"""Bounded current-level discovery; all projectile names remain candidates."""

import math
import struct
import time
from datetime import datetime, timezone

from apb_manual_labels import instance_signature
from apb_runtime_audit import TYPES


class SurveyChanged(RuntimeError):
    """A normal actor lifecycle change invalidated this snapshot; retry."""


def integer(value):
    return int(value, 0) if isinstance(value, str) else int(value)


class LiveObjectSurvey:
    def __init__(self, session, profile, diagnostic=None):
        if profile['identity'] != session.identity:
            raise RuntimeError('Live survey profile identity differs from process')
        if session.identity['image_sha256'].upper() != '35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453':
            raise RuntimeError('Live survey layout is only observed for the 2026-10-18 build')
        self.session, self.profile = session, profile
        self.base = integer(profile['module_base'])
        dos = self.read(self.base, 0x40)
        if dos[:2] != b'MZ':
            raise RuntimeError('Live survey module has no DOS signature')
        pe_offset, = struct.unpack_from('<I', dos, 0x3C)
        if not 0x40 <= pe_offset <= 0x100000:
            raise RuntimeError('Live survey PE header offset invalid')
        pe = self.read(self.base+pe_offset, 0x60)
        if pe[:4] != b'PE\0\0' or struct.unpack_from('<H', pe, 24)[0] != 0x20B:
            raise RuntimeError('Live survey module is not a PE32+ image')
        self.image_size, = struct.unpack_from('<I', pe, 24+56)
        if not pe_offset+0x60 <= self.image_size <= 0x40000000:
            raise RuntimeError('Live survey PE SizeOfImage invalid')
        if 'image_size' in profile and integer(profile['image_size']) != self.image_size:
            raise RuntimeError('Live survey profile SizeOfImage differs from current PE')
        self.diagnostic = diagnostic
        self.names, self.observed = {}, {}
        self.name_root = None
        self.classifications = {}

    def emit(self, event, **fields):
        if self.diagnostic:
            self.diagnostic({'event': event, **fields})

    def read(self, address, size):
        return b''.join(self.session.read(address+n, min(4096, size-n))
                        for n in range(0, size, 4096))

    def name(self, index, table, count):
        if not 0 <= index < count:
            raise RuntimeError(f'Name index outside table: {index}')
        if index not in self.names:
            entry, = struct.unpack('<Q', self.read(table+index*8, 8))
            body = self.read(entry, 512)
            flags, = struct.unpack_from('<I', body, 8)
            if flags >> 1 != index:
                raise RuntimeError(f'Name entry identity differs: {index}')
            if flags & 1:
                units = struct.unpack_from('<244H', body, 24)
                text = body[24:24+units.index(0)*2].decode('utf-16-le')
            else:
                text = body[24:].split(b'\0', 1)[0].decode('ascii')
            self.names[index] = text
        return self.names[index]

    def snapshot(self, world):
        body = self.read(world, 0x50)
        if (struct.unpack_from('<Q', body)[0]-self.base,
                struct.unpack_from('<I', body, 0x3C)[0]) != TYPES['world']:
            raise RuntimeError('Live survey World type differs')
        level, = struct.unpack_from('<Q', body, 0x44)
        level_body = self.read(level, 0xC4)
        if struct.unpack_from('<Q', level_body)[0]-self.base != 0x3208BF8:
            raise RuntimeError('Live survey level type differs')
        root = level_body[0xB4:0xC4]
        table, count, capacity = struct.unpack('<Qii', root)
        if not 0x10000 <= table < 0x800000000000 or not 0 <= count <= capacity <= 100000:
            raise RuntimeError('Live survey actor-array bounds invalid')
        try:
            entries = self.read(table, count*8)
        except OSError:
            if self.read(level+0xB4, 16) != root:
                raise SurveyChanged('Actor array resized during membership read') from None
            raise
        current_level = self.read(level, 0xC4)
        current_world = self.read(world, 0x50)
        if (current_level[0xB4:0xC4] != root
                or instance_signature(current_level) != instance_signature(level_body)
                or instance_signature(current_world) != instance_signature(body)
                or struct.unpack_from('<Q', current_world, 0x44)[0] != level):
            raise SurveyChanged('Actor-array binding changed during membership snapshot')
        return instance_signature(body), level, instance_signature(level_body), root, entries

    def refresh(self, world, camera, radius=5000, world_time=None):
        world = integer(world)
        if not math.isfinite(radius) or radius <= 0:
            raise ValueError('Live survey radius must be finite and positive')
        before = self.snapshot(world)
        root = self.read(self.base+0x3CD1F00, 16)
        table, count, capacity = struct.unpack('<Qii', root)
        if not 0x10000 <= table < 0x800000000000 or not 0 < count <= capacity <= 1000000:
            raise RuntimeError('Live survey name-array bounds invalid')
        if self.name_root != table:
            self.names.clear()
            self.name_root = table
        addresses = {item[0] for item in struct.iter_unpack('<Q', before[4]) if item[0]}
        found, current_keys = {}, set()
        for address in addresses:
            try:
                body = self.read(address, 0x100)
                vtable, = struct.unpack_from('<Q', body)
                if not self.base <= vtable < self.base+self.image_size:
                    raise RuntimeError(f'Actor APB type pointer invalid at {hex(address)}')
                fingerprint = instance_signature(body)
                marker, = struct.unpack_from('<I', body, 0x3C)
                key = f'{hex(address)}:{fingerprint}'
                current_keys.add(key)
                classification = self.classifications.get(key)
                if classification is None:
                    object_name = self.name(marker, table, count)
                    class_address, = struct.unpack_from('<Q', body, 0x30)
                    class_body = self.read(class_address, 0x40)
                    class_name = self.name(struct.unpack_from('<I', class_body, 0x3C)[0], table, count)
                    names = (object_name+' '+class_name).lower()
                    classification = (object_name, class_name, 'projectile' in names and 'weapon' not in names)
                    self.classifications[key] = classification
                object_name, class_name, projectile = classification
                if not projectile:
                    continue
                body = self.read(address, 0x1000)
                if instance_signature(body) != fingerprint:
                    self.emit('instance_changed', address=hex(address))
                    continue
                if struct.unpack_from('<Q', body, 0x2E0)[0] != world:
                    raise RuntimeError(f'Projectile owner World differs at {hex(address)}')
                position = struct.unpack_from('<3f', body, 0x88)
                if not all(math.isfinite(v) and abs(v) < 1000000 for v in position):
                    raise RuntimeError(f'Invalid candidate position at {hex(address)}')
                observed_utc = datetime.now(timezone.utc).isoformat()
                observed_monotonic = time.monotonic()
                observed_world_time, = struct.unpack('<d', self.read(world+0x6A0, 8))
                if not math.isfinite(observed_world_time) or observed_world_time < 0:
                    raise RuntimeError('Invalid World clock during projectile observation')
                key = f'{hex(address)}:{instance_signature(body)}'
                previous = self.observed.get(key, {})
                spec = {'address': hex(address), 'vtable_rva': hex(struct.unpack_from('<Q', body)[0]-self.base),
                        'marker': marker, 'position_offset': 0x88, 'instance_signature': instance_signature(body),
                        'instance_key': key, 'object_name': object_name, 'class_name': class_name,
                        'position': position, 'body_hex': body.hex(),
                        'capture_extent': 0x1000, 'extent_status': 'bounded_probe_not_verified_object_size',
                        'candidate_kind': 'projectile',
                        'first_seen_utc': previous.get('first_seen_utc', observed_utc),
                        'first_seen_monotonic': previous.get('first_seen_monotonic', observed_monotonic),
                        'last_seen_utc': observed_utc, 'last_seen_monotonic': observed_monotonic,
                        'first_seen_world_time': previous.get('first_seen_world_time', observed_world_time),
                        'last_seen_world_time': observed_world_time,
                        'observed_world_time': observed_world_time,
                        'survey_world_time_before': world_time,
                        'timestamp_semantics': 'observation_time_not_spawn_fuse_or_detonation'}
                found[key] = spec
            except OSError:
                current = self.snapshot(world)
                current_members = {item[0] for item in struct.iter_unpack('<Q', current[4]) if item[0]}
                if current[:3] != before[:3]:
                    raise SurveyChanged('World/level binding changed during actor read') from None
                if address in current_members:
                    raise
                self.emit('absent_during_read', address=hex(address))
        after = self.snapshot(world)
        if after[:3] != before[:3]:
            raise SurveyChanged('World/level binding changed during survey')
        members = {item[0] for item in struct.iter_unpack('<Q', after[4]) if item[0]}
        if addresses != members:
            self.emit('membership_changed', added_count=len(members-addresses), removed_count=len(addresses-members))
        # Recheck only accepted projectile instances. Unrelated actor lifecycle
        # changes do not invalidate still-live exact-instance observations.
        for key, spec in list(found.items()):
            address = integer(spec['address'])
            if address not in members:
                del found[key]
                continue
            try:
                final = self.read(address, 0x2E8)
            except OSError:
                final_snapshot = self.snapshot(world)
                if final_snapshot[:3] != before[:3]:
                    raise SurveyChanged('World/level binding changed during projectile check') from None
                if address in {item[0] for item in struct.iter_unpack('<Q', final_snapshot[4])}:
                    raise
                del found[key]
                continue
            if instance_signature(final) != spec['instance_signature']:
                self.emit('instance_changed', address=hex(address))
                del found[key]
                continue
            if (struct.unpack_from('<Q', final)[0] != self.base+integer(spec['vtable_rva'])
                    or struct.unpack_from('<Q', final, 0x2E0)[0] != world):
                raise RuntimeError(f'Projectile type/World recheck differs at {hex(address)}')
            position = struct.unpack_from('<3f', final, 0x88)
            if not all(math.isfinite(v) and abs(v) < 1000000 for v in position):
                raise RuntimeError(f'Projectile position recheck invalid at {hex(address)}')
            spec['position'] = position
        if self.read(self.base+0x3CD1F00, 16) != root:
            self.emit('lifecycle_change', reason='Name array changed during survey')
            raise SurveyChanged('Name array changed during survey')
        for key in self.observed.keys()-found.keys():
            self.emit('expired', instance_key=key, last_observation=self.observed[key])
        for key in found.keys()-self.observed.keys():
            self.emit('first_seen', candidate=found[key])
        self.observed = found
        self.classifications = {key: value for key, value in self.classifications.items()
                                if key in current_keys}
        result = [spec for spec in found.values() if math.dist(spec['position'], camera['position']) <= radius]
        return result
