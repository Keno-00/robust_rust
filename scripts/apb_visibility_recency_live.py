"""Fresh-witness live check of the explicit pawn float64 render-recency candidate."""

import argparse
import hashlib
import json
import math
import struct
import sys
import time
import traceback
from pathlib import Path

from apb_f8_common import Session, module_base, utc_now, write_json
from apb_f8_common import camera_at, client_capture
from apb_visibility_f8 import capture
from apb_visibility_scope import VisibilityScope
from apb_visibility_trace_overlay import TraceOverlay


PAWN_TIMESTAMP = 0xB4
WORLD_CLOCK = 0x6A0
RECENCY_SECONDS = 1.0
NEGATIVE_AGE_TOLERANCE = 0.25
SETTLE_SECONDS = 1.5


def timed_double(session, address):
    # Windows monotonic() can have a coarse tick. QPC-backed perf_counter()
    # resolves successive driver reads for bracket interpolation.
    start = time.perf_counter()
    raw = session.read(address, 8)
    end = time.perf_counter()
    value = struct.unpack('<d', raw)[0]
    if not math.isfinite(value) or value < 0:
        raise RuntimeError(f'Invalid float64 timestamp at {hex(address)}: {raw.hex()}')
    return {'address': hex(address), 'raw_hex': raw.hex(), 'value': value,
            'start_monotonic_s': start, 'end_monotonic_s': end, 'midpoint_s': (start+end)/2}


def read_recency(session, scope):
    scope.verify()
    first = timed_double(session, scope.world+WORLD_CLOCK)
    timestamp = timed_double(session, scope.pawn+PAWN_TIMESTAMP)
    last = timed_double(session, scope.world+WORLD_CLOCK)
    if last['value'] < first['value']:
        raise RuntimeError('World clock moved backward during bracketed reading; restart witness selection')
    span = last['midpoint_s']-first['midpoint_s']
    if span <= 0:
        raise RuntimeError('Nonpositive clock read-time span')
    fraction = (timestamp['midpoint_s']-first['midpoint_s'])/span
    interpolated = first['value']+fraction*(last['value']-first['value'])
    age = interpolated-timestamp['value']
    if age < -NEGATIVE_AGE_TOLERANCE:
        raise RuntimeError(f'Pawn timestamp exceeds bracketed World clock by {-age:.6f}s; raw values retained in console: '
                           f'{json.dumps([first, timestamp, last])}')
    scope.verify()
    return {'world_before': first, 'pawn_timestamp': timestamp, 'world_after': last,
            'interpolated_world_clock': interpolated, 'candidate_age_s': age,
            'raw_age_bounds_s': [first['value']-timestamp['value'], last['value']-timestamp['value']],
            'candidate_predicts_clear': age <= RECENCY_SECONDS}


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('evidence_report', type=Path)
    parser.add_argument('output_directory', type=Path)
    parser.add_argument('--hz', type=float, default=10)
    args = parser.parse_args()
    if not math.isfinite(args.hz) or not 1 <= args.hz <= 15:
        parser.error('Use 1..15 Hz')
    args.output_directory = args.output_directory.resolve()
    args.evidence_report = args.evidence_report.resolve()
    if args.output_directory.exists() or not args.output_directory.parent.is_dir():
        parser.error('Use a new output directory with an existing parent')
    args.output_directory.mkdir()
    status = {'format': 'apb-visibility-recency-live-v1', 'started_utc': utc_now(),
              'read_timing_clock': 'time.perf_counter (Windows QueryPerformanceCounter)',
              'status': 'initializing', 'requested_hz': args.hz, 'evidence_report': str(args.evidence_report),
              'layout': {'pawn_timestamp': hex(PAWN_TIMESTAMP), 'world_clock': hex(WORLD_CLOCK),
                         'pawn_world': '0x2e0', 'encoding': 'little-endian float64'},
              'threshold_s': RECENCY_SECONDS, 'negative_age_tolerance_s': NEGATIVE_AGE_TOLERANCE,
              'settle_seconds': SETTLE_SECONDS,
              'limits': 'Render-recency candidate; operator labels independent. Fresh witness selected explicitly. Movement permitted; no automatic reacquisition.'}
    session = ui = None
    code = 0
    rows_path = args.output_directory/'samples.jsonl.partial'
    rounds = images = missed = 0
    counts = {label: {'samples': 0, 'agree': 0, 'settled_samples': 0, 'settled_agree': 0}
              for label in ('clear', 'blocked')}
    try:
        report_bytes = args.evidence_report.read_bytes()
        report = json.loads(report_bytes)
        layout = report['layout']
        if (report['format'] != 'apb-visibility-float64-recency-cross-session-v1'
                or int(layout['pawn_timestamp_offset'], 16) != PAWN_TIMESTAMP
                or int(layout['world_clock_offset'], 16) != WORLD_CLOCK
                or int(layout['pawn_to_world_pointer_offset'], 16) != 0x2E0
                or layout['numeric_encoding'] != 'little-endian float64'
                or layout['threshold_seconds'] != RECENCY_SECONDS):
            raise RuntimeError('Evidence report does not describe the explicit float64 recency candidate')
        hashes = {s['identity']['image_sha256'] for s in report['sessions']}
        session = Session(args.pid)
        if hashes != {session.identity['image_sha256']}:
            raise RuntimeError('APB executable differs from the reviewed build; do not reuse these offsets')
        status.update(identity=session.identity, evidence_sha256=hashlib.sha256(report_bytes).hexdigest())
        write_json(args.output_directory/'session-start.json', status)
        camera_events = args.output_directory/'camera-table-events.jsonl'
        camera_events.touch(exist_ok=False)

        def diagnostic(event):
            with camera_events.open('a', encoding='utf-8') as stream:
                stream.write(json.dumps(event, allow_nan=False)+'\n')

        ui = TraceOverlay(session)
        ui.panel.title('APB render recency live check')
        base = module_base(session)
        selection = args.output_directory/'selection'
        selection.mkdir()
        print('LIVE FLOAT64 RECENCY. Expose laptop character. F8 survey; F5 next pawn; F6 select laptop. '
              'F1 exposed / F2 hidden / F4 partial / F3 unknown; F8 image; F11 finish. Movement permitted.', flush=True)
        ui.wait('LIVE RECENCY: SELECT LAPTOP\nExpose laptop character. F8 survey | F11 exit')
        ui.draw('Surveying current World pawn list. Keep characters still until selection.')
        source = capture(session, selection, 1, diagnostic, pawn_source='world-roster')
        point = ui.select(source)
        scope = VisibilityScope(session, base, source, point, selection, diagnostic,
                                allow_camera_rebinding=True)
        if scope.owner['pawn'] == scope.local['pawn']:
            raise RuntimeError('Selected the PC observer; restart and select the laptop character')
        for name, offset in (('pawn', PAWN_TIMESTAMP), ('world', WORLD_CLOCK)):
            region = next(r for r in scope.regions if r['name'] == name)
            if offset+8 > region['size']:
                raise RuntimeError(f'{name} float64 field lies outside its validated prefix')
        status.update(module_base=hex(base), selected_pawn=point, world=hex(scope.world),
                      regions=scope.regions, observer=scope.local,
                      camera_address=hex(scope.camera_address), camera_owner=scope.camera_owner,
                      camera_signature=scope.camera_signature,
                      camera_binding_policy='Rebind only the unique exact same typed camera instance; log every move')
        write_json(args.output_directory/'session-scope.json', status)
        ui.wait('LIVE RECENCY READY\nF8 start | F11 exit\nF1 exposed | F2 hidden | F4 partial')
        observation, label_time = 'unknown', time.perf_counter()
        deadline = time.perf_counter()
        previous_clock = previous_timestamp = None
        keys = {0x70: 'clear', 0x71: 'blocked', 0x72: 'unknown', 0x73: 'partial'}
        with rows_path.open('x', encoding='utf-8') as stream:
            while True:
                edges = ui.keys()
                if edges is None:
                    raise RuntimeError('APB lost focus; live check stopped and partial evidence retained')
                if 0x7A in edges:
                    break
                label_edges = edges & keys.keys()
                if len(label_edges) > 1:
                    raise RuntimeError('Multiple label keys pressed together; evidence retained, restart with one label at a time')
                if label_edges:
                    observation = keys[next(iter(label_edges))]
                    label_time = time.perf_counter()
                    stream.write(json.dumps({'event': 'operator_label', 'utc': utc_now(),
                        'monotonic_s': label_time, 'observation': observation})+'\n')
                    stream.flush()
                if 0x77 in edges or label_edges:
                    images += 1
                    frame = client_capture(session, args.output_directory/f'frame-{images:06d}.png')
                    stream.write(json.dumps({'event': 'operator_image', 'utc': utc_now(),
                        'monotonic_s': time.perf_counter(), 'observation': observation, 'image': frame})+'\n')
                    stream.flush()
                now = time.perf_counter()
                if now < deadline:
                    time.sleep(min(deadline-now, 0.005))
                    continue
                value = read_recency(session, scope)
                current_clock = value['world_after']['value']
                current_timestamp = value['pawn_timestamp']['value']
                if ((previous_clock is not None and current_clock < previous_clock)
                        or (previous_timestamp is not None and current_timestamp < previous_timestamp-0.05)):
                    raise RuntimeError(f'Clock/timestamp moved backward: {json.dumps(value)}; restart witness selection')
                previous_clock, previous_timestamp = current_clock, current_timestamp
                label_age = value['pawn_timestamp']['midpoint_s']-label_time
                row = {'round': rounds, 'utc': utc_now(), 'observation': observation,
                       'seconds_since_operator_label': label_age, 'missed_deadlines': missed, **value}
                stream.write(json.dumps(row, allow_nan=False)+'\n')
                stream.flush()
                if observation in counts:
                    agree = value['candidate_predicts_clear'] == (observation == 'clear')
                    count = counts[observation]
                    count['samples'] += 1
                    count['agree'] += int(agree)
                    if label_age >= SETTLE_SECONDS:
                        count['settled_samples'] += 1
                        count['settled_agree'] += int(agree)
                result = 'RECENT' if value['candidate_predicts_clear'] else 'STALE'
                ui.draw(f"LIVE Pawn #{point['id']}: {result} | age {value['candidate_age_s']:.3f}s\n"
                        f"World {value['interpolated_world_clock']:.3f} | pawn +B4 {current_timestamp:.3f}\n"
                        f"YOUR LABEL: {observation.upper()} | samples {rounds+1}\n"
                        'F1 exposed F2 hidden F4 partial | F8 image F11 finish',
                        camera_at(session, scope.camera_address), [scope.point], point['id'])
                if rounds % max(1, int(args.hz)) == 0:
                    print(f'{observation}: {result}, age {value["candidate_age_s"]:.6f}s, '
                          f'world {current_clock:.6f}, pawn {current_timestamp:.6f}', flush=True)
                rounds += 1
                deadline += 1/args.hz
                ended = time.perf_counter()
                if deadline < ended:
                    skipped = int((ended-deadline)*args.hz)+1
                    missed += skipped
                    deadline += skipped/args.hz
        if not rounds:
            raise RuntimeError('No live samples captured')
        rows_path.rename(args.output_directory/'samples.jsonl')
        status['status'] = 'completed_candidate_observations'
        print(f'LIVE CHECK COMPLETE: {json.dumps(counts)}. Evidence: {args.output_directory}', flush=True)
    except KeyboardInterrupt as error:
        code = 130
        status.update(status='operator_stopped', reason=str(error))
        print(f'Live check stopped: {error}. Evidence retained.', flush=True)
    except Exception as error:
        code = 1
        status.update(status='failed', error=f'{type(error).__name__}: {error}')
        (args.output_directory/'failure.txt').write_text(traceback.format_exc(), encoding='utf-8')
        traceback.print_exc()
        if ui is not None:
            from tkinter import messagebox
            ui.panel.deiconify()
            messagebox.showerror('Live recency check failed', f'{error}\n\nEvidence: {args.output_directory}', parent=ui.panel)
    finally:
        status.update(completed_utc=utc_now(), rounds=rounds, images=images,
                      missed_deadlines=missed, operator_label_agreement=counts)
        if rows_path.exists():
            status['partial_samples_sha256'] = hashlib.sha256(rows_path.read_bytes()).hexdigest()
        elif (args.output_directory/'samples.jsonl').exists():
            status['samples_sha256'] = hashlib.sha256((args.output_directory/'samples.jsonl').read_bytes()).hexdigest()
        write_json(args.output_directory/'session.json', status)
        if ui is not None:
            ui.close()
        if session is not None:
            session.close()
    return code


if __name__ == '__main__':
    sys.stderr = sys.stdout
    raise SystemExit(run())
