"""Live raw-value recheck of one same-launch visibility discovery candidate."""

import argparse
import hashlib
import json
import math
import struct
import sys
import time
import traceback
from pathlib import Path

from apb_f8_common import Session, camera_at, client_capture, module_base, utc_now, write_json
from apb_manual_labels import instance_signature
from apb_visibility_candidates import load
from apb_visibility_scope import VisibilityScope
from apb_visibility_trace_overlay import TraceOverlay


def read_candidate(session, scope, candidate):
    scope.verify()
    region = next(r for r in scope.regions if r['name'] == candidate['region'])
    address = region['address'] + int(candidate['offset'], 16)
    size = 1 if 'bit' in candidate else 4
    if not region['address'] <= address <= region['address']+region['size']-size:
        raise RuntimeError('Candidate offset is outside its recorded scope')
    clock_bytes, clock_mid = None, None
    if 'clock_offset' in candidate:
        world = next(r for r in scope.regions if r['name'] == 'world')
        clock_offset = int(candidate['clock_offset'], 16)
        if not 0 <= clock_offset <= world['size']-4:
            raise RuntimeError('Clock offset is outside the recorded world prefix')
        begin = time.monotonic()
        clock_bytes = session.read(scope.world+clock_offset, 4)
        clock_mid = (begin+time.monotonic())/2
    begin = time.monotonic()
    raw = session.read(address, size)
    midpoint = (begin+time.monotonic())/2
    result = {'address': hex(address), 'raw_hex': raw.hex(), 'read_monotonic_s': midpoint}
    if 'bit' in candidate:
        value = (raw[0] >> candidate['bit']) & 1
        result.update(byte=raw[0], bit=candidate['bit'], value=value,
                      candidate_predicts_clear=(value == candidate['clear_value']))
    else:
        value = struct.unpack('<f', raw)[0]
        valid = math.isfinite(value)
        result['raw_float32'] = value if valid else None
        if clock_bytes is not None:
            clock_value = struct.unpack('<f', clock_bytes)[0]
            valid = valid and math.isfinite(clock_value)
            result.update(clock_raw_hex=clock_bytes.hex(), clock_float32=clock_value if math.isfinite(clock_value) else None,
                          measured_read_skew_s=midpoint-clock_mid)
            value = clock_value + candidate['clock_slope']*(midpoint-clock_mid)-value if valid else None
            valid = valid and value >= -0.25
            result['candidate_age_s'] = value if valid else None
        if not valid:
            raise RuntimeError(f'Candidate scalar/clock is nonfinite or its age is invalid at {hex(address)}; inspect raw bytes')
        result['candidate_predicts_clear'] = value < candidate['cutoff'] if candidate['clear_when'] == 'less' else value > candidate['cutoff']
    scope.verify()
    for item in scope.regions:
        if instance_signature(session.read(item['address'], 0x40)) != item['signature']:
            raise RuntimeError(f"Candidate owning instance changed: {item['name']}")
    return result


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('source', type=Path)
    parser.add_argument('candidate_id', help='ID from candidates.json, e.g. bits-1 or timestamps-1')
    parser.add_argument('output_directory', type=Path)
    parser.add_argument('--hz', type=float, default=5)
    args = parser.parse_args()
    if not math.isfinite(args.hz) or not 1 <= args.hz <= 15:
        parser.error('Use 1..15 Hz')
    args.source, args.output_directory = args.source.resolve(), args.output_directory.resolve()
    if args.output_directory.exists() or not args.output_directory.parent.is_dir():
        parser.error('Use a new live output directory with an existing parent')
    args.output_directory.mkdir()
    session = ui = None
    status = {'format': 'apb-visibility-live-v2', 'source': str(args.source), 'status': 'initializing',
              'candidate_id': args.candidate_id, 'started_utc': utc_now(), 'requested_hz': args.hz,
              'limits': 'Same-launch diagnostic. Predicted clear is a candidate rule, never verified LOS. PC movement permitted.'}
    code = 0
    try:
        saved, _, _, unfinished, _ = load(args.source)
        if unfinished:
            raise RuntimeError('Source contains unfinished captures; review those before live rechecking')
        report = json.loads((args.source / 'candidates.json').read_text(encoding='utf-8'))
        if (report['format'] != 'apb-visibility-candidates-v2' or report['identity'] != saved['identity']
                or report['scope_sha256'] != hashlib.sha256((args.source / 'session-scope.json').read_bytes()).hexdigest()):
            raise RuntimeError('Candidate report identity/format mismatch')
        choices = [item for key in ('bits', 'scalars', 'timestamps') for item in report[key]
                   if item['id'] == args.candidate_id]
        if len(choices) != 1:
            raise RuntimeError('Candidate ID is absent or ambiguous; inspect candidates.json')
        candidate = choices[0]
        if candidate['negative_observation'] != 'blocked':
            raise RuntimeError('Only in-view clear versus wall-blocked candidates are in scope')
        session = Session(args.pid)
        if session.identity != saved['identity']:
            raise RuntimeError('Candidate belongs to another APB launch. Collect a new discovery; no cross-launch pointer path is established.')
        base = module_base(session)
        if base != int(saved['module_base'], 16):
            raise RuntimeError('Module base differs from discovery')
        source = json.loads((args.source / 'selection' / 'capture.json').read_text(encoding='utf-8'))
        old_owner = dict(saved['selected_pawn']['ownership'])
        scope = VisibilityScope(session, base, source, saved['selected_pawn'], args.source / 'selection')
        saved_context = saved['scope_context']
        if (scope.local != saved_context['local'] or scope.camera_address != saved_context['camera_address']
                or scope.camera_signature != saved_context['camera_signature']
                or scope.camera_owner != tuple(saved_context['camera_owner'])):
            raise RuntimeError('Observer/camera context differs from discovery; collect a new session')
        for key in ('pawn', 'pawn_signature', 'controller', 'components', 'vehicle_attachment', 'status'):
            if old_owner[key] != scope.owner[key]:
                raise RuntimeError(f'Witness ownership differs from discovery: {key}')
        if [(r['name'], r['address'], r['size'], r['signature']) for r in scope.regions] != [
                (r['name'], r['address'], r['size'], r['signature']) for r in saved['regions']]:
            raise RuntimeError('Live scope differs from recorded discovery')
        ui = TraceOverlay(session)
        status.update(identity=session.identity, candidate=candidate)
        write_json(args.output_directory / 'session-start.json', status)
        print(f"LIVE CANDIDATE {args.candidate_id}. F1 clear / F2 wall-blocked / F4 partial. "
              'F8 saves a screenshot; F11 exits. Candidate predictions are unverified.', flush=True)
        ui.wait(f"LIVE RAW RECHECK: {args.candidate_id}\nF8 start | F11 exit\nCandidate rule remains unverified.")
        observation, rounds, missed, maximum_gap, previous = 'unknown', 0, 0, 0.0, None
        image_sequence = 0
        deadline = time.monotonic()
        keys = {0x70: 'clear', 0x71: 'blocked', 0x73: 'partial'}
        with (args.output_directory / 'samples.jsonl.partial').open('x', encoding='utf-8') as stream:
            while True:
                edges = ui.keys()
                if edges is None:
                    raise RuntimeError('APB lost focus during live reading; diagnostic stopped')
                if 0x7A in edges:
                    break
                for key in sorted(edges & keys.keys()):
                    observation = keys[key]
                if 0x77 in edges:
                    image_sequence += 1
                    image = client_capture(session, args.output_directory / f'frame-{image_sequence:06d}.png')
                    stream.write(json.dumps({'event': 'operator_image', 'observation': observation,
                        'utc': utc_now(), 'monotonic_s': time.monotonic(), 'image': image})+'\n')
                    stream.flush()
                now = time.monotonic()
                if now < deadline:
                    time.sleep(min(deadline-now, 0.005))
                    continue
                if previous is not None:
                    maximum_gap = max(maximum_gap, now-previous)
                previous = now
                value = read_candidate(session, scope, candidate)
                row = {'round': rounds, 'utc': utc_now(), 'observation': observation,
                       'max_gap_s': maximum_gap, 'missed_deadlines': missed, **value}
                stream.write(json.dumps(row, allow_nan=False)+'\n')
                stream.flush()
                print(f"{args.candidate_id} {observation}: {json.dumps(value)}", flush=True)
                displayed = value.get('candidate_age_s', value.get('raw_float32', value.get('value')))
                ui.draw(f"RAW {args.candidate_id}: {displayed:.6g}\nYOUR LABEL: {observation} | samples {rounds+1}\n"
                        'F1 clear F2 wall F4 partial\nF8 image | F11 exit; unverified candidate')
                rounds += 1
                deadline += 1/args.hz
                ended = time.monotonic()
                if deadline < ended:
                    skipped = int((ended-deadline)*args.hz)+1
                    missed += skipped
                    deadline += skipped/args.hz
        if not rounds:
            raise RuntimeError('No live samples captured')
        (args.output_directory / 'samples.jsonl.partial').rename(args.output_directory / 'samples.jsonl')
        status.update(status='completed_candidate_observations', rounds=rounds,
                      missed_deadlines=missed, max_gap_s=maximum_gap)
    except KeyboardInterrupt as error:
        code = 130
        status.update(status='operator_stopped', reason=str(error))
        print(f'Live reading stopped: {error}; evidence retained.', flush=True)
    except Exception as error:
        code = 1
        status.update(status='failed', error=f'{type(error).__name__}: {error}')
        (args.output_directory / 'failure.txt').write_text(traceback.format_exc(), encoding='utf-8')
        status['completed_utc'] = utc_now()
        write_json(args.output_directory / 'session.json', status)
        traceback.print_exc()
        if ui is not None:
            from tkinter import messagebox
            ui.panel.deiconify()
            messagebox.showerror('APB visibility live reading failed', f'{error}\n\nEvidence: {args.output_directory}', parent=ui.panel)
    finally:
        status['completed_utc'] = utc_now()
        if not (args.output_directory / 'session.json').exists():
            write_json(args.output_directory / 'session.json', status)
        if session is not None:
            session.close()
        if ui is not None:
            ui.close()
    return code


if __name__ == '__main__':
    sys.stderr = sys.stdout
    raise SystemExit(run())
