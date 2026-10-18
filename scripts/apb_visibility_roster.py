"""Guarded World pawn traversal matching src/apb_roster.rs; EchoDriver only."""

import struct

from apb_camera_live import valid_pointer
from apb_character_ownership import PAWN_TYPE, VEHICLE_TYPE, WORLD_TYPE, typed_header
from apb_f8_common import utc_now, write_json
from apb_manual_labels import instance_signature


WORLD_PAWN_HEAD = 0x48C
PAWN_NEXT = 0x350
PAWN_WORLD = 0x2E0
MAX_NODES = 512


def read_roster(session, base, world):
    signature = typed_header(session, base, world, WORLD_TYPE)
    root = session.read(world, WORLD_PAWN_HEAD+8)
    if instance_signature(root) != signature:
        raise RuntimeError("World instance changed before pawn traversal; retry selection")
    head = struct.unpack_from('<Q', root, WORLD_PAWN_HEAD)[0]
    address, seen, nodes, actors = head, set(), [], []
    while address:
        if len(nodes) >= MAX_NODES or not valid_pointer(address) or address in seen:
            raise RuntimeError(f"World pawn list has an invalid pointer, cycle or exceeds {MAX_NODES} nodes at {hex(address)}")
        seen.add(address)
        body = session.read(address, PAWN_NEXT+8)
        kind = (struct.unpack_from('<Q', body)[0]-base, struct.unpack_from('<I', body, 0x3C)[0])
        if kind not in (PAWN_TYPE, VEHICLE_TYPE):
            raise RuntimeError(f"Unsupported World pawn-list type {kind} at {hex(address)}; targets withheld")
        if struct.unpack_from('<Q', body, PAWN_WORLD)[0] != world:
            raise RuntimeError(f"Pawn-list node {hex(address)} changed World; retry selection")
        link = struct.unpack_from('<Q', body, PAWN_NEXT)[0]
        node = {'address': address, 'body': body, 'kind': kind, 'signature': instance_signature(body), 'next': link}
        nodes.append(node)
        if kind == PAWN_TYPE:
            actors.append({'address': hex(address), 'vtable_rva': hex(kind[0]), 'marker': kind[1],
                           'position_offset': 0x88, 'slot_rvas': [], 'discovery_source': 'world_0x48c_pawn_0x350',
                           'instance_signature': node['signature']})
        address = link
    for node in nodes:
        body = session.read(node['address'], PAWN_NEXT+8)
        if (instance_signature(body) != node['signature']
                or (struct.unpack_from('<Q', body)[0]-base, struct.unpack_from('<I', body, 0x3C)[0]) != node['kind']
                or struct.unpack_from('<Q', body, PAWN_WORLD)[0] != world
                or struct.unpack_from('<Q', body, PAWN_NEXT)[0] != node['next']):
            raise RuntimeError(f"World pawn-list node or link changed at {hex(node['address'])}; retry selection")
    final = session.read(world, WORLD_PAWN_HEAD+8)
    if instance_signature(final) != signature or struct.unpack_from('<Q', final, WORLD_PAWN_HEAD)[0] != head:
        raise RuntimeError("World pawn-list root changed during traversal; retry selection")
    return {'world': world, 'world_body': root, 'world_signature': signature, 'head': head, 'nodes': nodes, 'actors': actors}


def capture_roster(session, base, world, directory, sequence):
    started = utc_now()
    roster = read_roster(session, base, world)
    headers_name = f'roster-{sequence:04d}.headers.bin'
    bodies_name = f'roster-{sequence:04d}.bodies.bin'
    with (directory/headers_name).open('xb') as headers, (directory/bodies_name).open('xb') as bodies:
        headers.write(struct.pack('<Q', world)+roster['world_body'][:0x40])
        for node in roster['nodes']:
            headers.write(struct.pack('<Q', node['address'])+node['body'][:0x40])
            bodies.write(struct.pack('<Q', node['address'])+node['body'])
    report = {'source': 'world_0x48c_pawn_0x350', 'started_utc': started, 'completed_utc': utc_now(),
              'world': hex(world), 'world_signature': roster['world_signature'], 'head': hex(roster['head']),
              'header_file': headers_name, 'body_file': bodies_name, 'nodes': len(roster['nodes']),
              'characters': len(roster['actors']), 'vehicles': len(roster['nodes'])-len(roster['actors']),
              'links': [{'address': hex(n['address']), 'signature': n['signature'],
                         'kind': [hex(n['kind'][0]), n['kind'][1]], 'next': hex(n['next'])} for n in roster['nodes']],
              'status': 'complete_guarded_rooted_roster', 'limits': 'Sequential reads; every edge and instance rechecked. No partial roster accepted.'}
    write_json(directory/f'roster-{sequence:04d}.json', report)
    return roster['actors'], report
