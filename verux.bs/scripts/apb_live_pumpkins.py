"""Live emitter/prefab location associations; availability is not established."""

import math
import struct
from datetime import datetime, timezone

from apb_cursor_modules import module_base
from apb_manual_labels import instance_signature
from apb_runtime_audit import TYPES, kind, u64


BUILD = '35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453'
EMITTER = (0x2AAD618, 16985)
PREFAB = (0x3151050, 4741)
LEVEL_VTABLE = 0x3208BF8


def integer(value):
    return int(value, 0) if isinstance(value, str) else int(value)


class LivePumpkinSurvey:
    def __init__(self, session, profile, diagnostic=None):
        if (profile.get('format') != 'apb-build-candidates-v1' or profile.get('identity') != session.identity
                or session.identity['image_sha256'].upper() != BUILD):
            raise RuntimeError('Pumpkin survey requires exact current process and audited build')
        self.session, self.profile, self.diagnostic = session, profile, diagnostic
        self.base = module_base(session)
        if integer(profile['module_base']) != self.base:
            raise RuntimeError('Pumpkin survey module base differs from profile')
        self.levels = set()
        self.fingerprints = {}

    def emit(self, event, **fields):
        if self.diagnostic:
            self.diagnostic(event, **fields)

    def read(self, address, size):
        if not 0x10000 <= address < 0x800000000000:
            raise RuntimeError(f'Invalid pumpkin survey pointer {hex(address)}')
        return b''.join(self.session.read(address+n, min(4096, size-n)) for n in range(0, size, 4096))

    def level_snapshot(self, level):
        body = self.read(level, 0xC4)
        signature = instance_signature(body)
        if u64(body, 0)-self.base != LEVEL_VTABLE:
            self.emit('pumpkin_level_expired', level=hex(level), reason='ULevel type changed',
                      observed_type=kind(body, self.base))
            return None
        if level in self.fingerprints and self.fingerprints[level] != signature:
            self.emit('pumpkin_level_expired', level=hex(level), reason='ULevel fingerprint changed',
                      before=self.fingerprints[level], after=signature)
            return None
        self.fingerprints[level] = signature
        table, count, capacity = struct.unpack_from('<Qii', body, 0xB4)
        if not 0 <= count <= capacity <= 100000:
            raise RuntimeError(f'Pumpkin survey actor-array bounds invalid: {hex(level)}')
        entries = self.read(table, count*8) if count else b''
        return signature, {item[0] for item in struct.iter_unpack('<Q', entries) if item[0]}, body[0xB4:0xC4].hex()

    def position(self, body):
        xyz = struct.unpack_from('<3f', body, 0x88)
        if not all(math.isfinite(v) and abs(v) < 1000000 for v in xyz):
            raise RuntimeError('Pumpkin association contains invalid XYZ')
        return xyz

    def current_levels(self, persistent):
        level_body = self.read(persistent, 0x50)
        if u64(level_body, 0)-self.base != LEVEL_VTABLE:
            raise RuntimeError('Persistent level type differs')
        outer = u64(level_body, 0x44)
        body = self.read(outer, 0xFC)
        if kind(body, self.base) != (0x3164988, 1151):
            raise RuntimeError('Current level registry owner type differs')
        pointer, count, capacity = struct.unpack_from('<Qii', body, 0xEC)
        if not 0 < count <= capacity <= 4096:
            raise RuntimeError('Current level registry bounds invalid')
        data = self.read(pointer, count*8)
        levels = {item[0] for item in struct.iter_unpack('<Q', data) if item[0]}
        if persistent not in levels:
            raise RuntimeError('Current level registry excludes persistent level')
        for level in levels:
            header = self.read(level, 0x50)
            if u64(header, 0)-self.base != LEVEL_VTABLE:
                raise RuntimeError(f'Current level registry entry type differs: {hex(level)}')
        if (self.read(outer+0xEC, 16) != body[0xEC:0xFC]
                or self.read(pointer, count*8) != data):
            raise RuntimeError('Current level registry changed; repeat scan')
        return levels

    def refresh(self, world, camera, radius=10000):
        world = integer(world)
        if not math.isfinite(radius) or radius <= 0:
            raise ValueError('Pumpkin survey radius must be finite and positive')
        if not all(math.isfinite(v) for v in camera['position']):
            raise RuntimeError('Pumpkin survey camera position invalid')
        self.session.require_foreground()
        world_body = self.read(world, 0x50)
        if kind(world_body, self.base) != TYPES['world']:
            raise RuntimeError('Pumpkin survey World type differs')
        persistent = u64(world_body, 0x44)
        self.levels = self.current_levels(persistent)
        result, rejected, expired, actor_count = {}, 0, 0, 0
        observed_utc = datetime.now(timezone.utc).isoformat()
        for level in sorted(self.levels):
            before = self.level_snapshot(level)
            if before is None:
                expired += 1
                continue
            pairs = []
            for address in before[1]:
                actor_count += 1
                if not 0x10000 <= address < 0x800000000000 or address % 8:
                    rejected += 1
                    continue
                try:
                    header = self.read(address, 0x40)
                    if kind(header, self.base) != EMITTER:
                        continue
                    emitter = self.read(address, 0x300)
                    if instance_signature(emitter) != instance_signature(header) or u64(emitter, 0x44) != level:
                        rejected += 1
                        continue
                    prefab_address = u64(emitter, 0x274)
                    if not 0x10000 <= prefab_address < 0x800000000000 or prefab_address % 8:
                        rejected += 1
                        continue
                    prefab = self.read(prefab_address, 0x300)
                    if kind(prefab, self.base) != PREFAB:
                        rejected += 1
                        continue
                    emitter_xyz, prefab_xyz = self.position(emitter), self.position(prefab)
                    if math.dist(emitter_xyz, prefab_xyz) > 500:
                        rejected += 1
                        continue
                    if min(math.dist(emitter_xyz, camera['position']), math.dist(prefab_xyz, camera['position'])) > radius:
                        continue
                    pairs.append((address, emitter, prefab_address, prefab))
                except OSError:
                    after_error = self.level_snapshot(level)
                    if after_error is None or address not in after_error[1]:
                        rejected += 1
                        self.emit('pumpkin_actor_expired', level=hex(level), address=hex(address),
                                  reason='Actor removed from current level during read')
                        continue
                    raise
            after = self.level_snapshot(level)
            if after is None:
                rejected += len(pairs)
                expired += 1
                continue
            if before[2] != after[2] or before[1] != after[1]:
                self.emit('pumpkin_actor_membership_changed', level=hex(level),
                          before_root_hex=before[2], after_root_hex=after[2],
                          added=len(after[1]-before[1]), removed=len(before[1]-after[1]))
            for address, emitter, prefab_address, prefab in pairs:
                if address not in after[1] or prefab_address not in after[1]:
                    rejected += 1
                    continue
                current_emitter = self.read(address, 0x300)
                current_prefab = self.read(prefab_address, 0x300)
                if (kind(current_emitter, self.base) != EMITTER or kind(current_prefab, self.base) != PREFAB
                        or instance_signature(current_emitter) != instance_signature(emitter)
                        or instance_signature(current_prefab) != instance_signature(prefab)
                        or u64(current_emitter, 0x44) != level or u64(current_emitter, 0x274) != prefab_address):
                    rejected += 1
                    continue
                current_xyz = [self.position(current_emitter), self.position(current_prefab)]
                if math.dist(*current_xyz) > 500:
                    rejected += 1
                    continue
                keys = [f'{hex(pointer)}:{instance_signature(body)}:136'
                        for pointer, body in ((address, current_emitter), (prefab_address, current_prefab))]
                for index, (pointer, body, name) in enumerate(((address, current_emitter, 'cAudioEmitterPositioningActor'),
                                                              (prefab_address, current_prefab, 'PrefabInstance'))):
                    result[keys[index]] = {'address': hex(pointer), 'vtable_rva': hex(kind(body, self.base)[0]),
                        'marker': kind(body, self.base)[1], 'instance_signature': instance_signature(body),
                        'position_offset': 0x88, 'position': current_xyz[index], 'object_name': name,
                        'candidate_kind': 'pumpkin_location_candidate', 'display_tag': 'pumpkin location (availability unknown)',
                        'related_instance_keys': [keys[1-index]], 'level': hex(level),
                        'association_emitter': hex(address), 'association_prefab': hex(prefab_address),
                        'association_offset': 0x274, 'association_distance': math.dist(*current_xyz),
                        'observed_utc': observed_utc, 'identity': self.session.identity,
                        'body_hex': body.hex(), 'capture_extent': 0x300,
                        'scope': 'typed emitter-to-prefab association only; pumpkin identity and availability unconfirmed'}
        final_world = self.read(world, 0x50)
        if (kind(final_world, self.base) != TYPES['world']
                or instance_signature(final_world) != instance_signature(world_body)
                or u64(final_world, 0x44) != persistent):
            raise RuntimeError('Pumpkin survey World/persistent level changed during refresh')
        final_levels = self.current_levels(persistent)
        result = {key: point for key, point in result.items() if integer(point['level']) in final_levels}
        self.session.require_foreground()
        self.emit('pumpkin_survey_scope', world=hex(world), persistent_level=hex(persistent),
                  considered_levels=len(self.levels), expired_levels=expired, actor_slots=actor_count,
                  rejected_instances=rejected, candidate_points=len(result), radius=radius,
                  scope='current typed level registry; nearby emitter-to-prefab associations only')
        return list(result.values())
