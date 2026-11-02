"""Capture nearby current-level emitter/prefab pairs and open their annotation overlay."""

import argparse
import json
import time
from pathlib import Path
from types import SimpleNamespace

from apb_cursor_scan import Session, ForegroundLost
from apb_live_pumpkins import LivePumpkinSurvey
from apb_object_annotation_overlay import run
from apb_runtime_audit import kind, u64
from apb_validation_runtime import ValidationRuntime, RuntimeChanged


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--radius', type=float, default=8000)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False, parents=True)
    session = Session(args.pid)
    try:
        source = json.loads(args.source.read_text(encoding='utf-8-sig'))
        runtime = ValidationRuntime(session, source)
        with (args.output/'scan.jsonl').open('x', encoding='utf-8', buffering=1) as stream:
            def log(event, **fields):
                stream.write(json.dumps({'event': event, **fields}, allow_nan=False)+'\n')
            survey = LivePumpkinSurvey(session, source, diagnostic=log)
            deadline = time.monotonic()+90
            while True:
                try:
                    session.require_foreground()
                    camera, hud, binding = runtime.camera()
                    break
                except (ForegroundLost, RuntimeChanged) as error:
                    if time.monotonic() >= deadline:
                        raise RuntimeError('No stable foreground APB camera within 90 seconds') from error
                    time.sleep(.2)
            world = u64(hud, 0x2E0)
            print('Scanning current level registry for nearby emitter/prefab pairs', flush=True)
            candidates = survey.refresh(world, camera, radius=args.radius)
            log('capture_complete', identity=session.identity, centre=camera['position'],
                candidate_count=len(candidates), radius=args.radius)
            if not candidates:
                raise RuntimeError('No nearby typed emitter/prefab pairs found; inspect scan.jsonl')
            (args.output/'association-capture.json').write_text(json.dumps(candidates, indent=2), encoding='utf-8')
            deadline = time.monotonic()+30
            while True:
                try:
                    session.require_foreground()
                    fresh_camera, fresh_hud, _ = runtime.camera()
                    break
                except (RuntimeChanged, ForegroundLost) as error:
                    log('camera_retry', message=str(error))
                    if time.monotonic() >= deadline:
                        raise RuntimeError('Camera remained unstable after capture; inspect association-capture.json') from error
                    time.sleep(.1)
            if u64(fresh_hud, 0x2E0) != world:
                raise RuntimeError('World changed during scan; repeat at the current location')
            vtable, marker = kind(fresh_hud, runtime.base)
            fresh_camera.update(vtable_rva=hex(vtable), marker=marker)
            profile = {'format': 'apb-build-candidates-v1', 'status': 'unverified_build_candidates',
                       'identity': session.identity, 'module_base': hex(runtime.base),
                       'camera': fresh_camera, 'candidates': candidates,
                       'selection': {'centre': camera['position'], 'radius_world_units': args.radius,
                                     'kind': 'typed emitter-to-prefab associations',
                                     'captured_levels': [{'level': hex(level)} for level in sorted(survey.levels)]}}
            profile_path = args.output/'profile.json'
            profile_path.write_text(json.dumps(profile, indent=2, allow_nan=False), encoding='utf-8')
            print(f'Opening overlay: {len(candidates)} nearby association markers; F8 starts', flush=True)
        run(SimpleNamespace(profile=profile_path, output=args.output/'annotations.jsonl', tag='pumpkin'), session)
    finally:
        session.close()


if __name__ == '__main__':
    main()
