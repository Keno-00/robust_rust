"""Temporary read-only confirmation of cursor-adjacent red hitmarker events."""
import argparse
import ctypes
import json
import math
import struct
import time
import winsound
from pathlib import Path
from ctypes import wintypes

import apb_cursor_scan
from apb_cursor_modules import module_base
from apb_validation_runtime import BUILD_SHA256
from apb_candidate_overlay import CandidateOverlay
from apb_f8_common import Point, Rect


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    sound = Path(__file__).resolve().parents[1] / 'assets/wav/[minecraft] hitsound.wav'
    if not sound.is_file():
        raise RuntimeError('Selected hitsound WAV is missing.')
    apb_cursor_scan.EXPECTED_SHA256 = BUILD_SHA256
    session = apb_cursor_scan.Session(args.pid)
    overlay = None
    try:
        base = module_base(session)
        u = session.user32
        u.GetClientRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(Rect)]
        u.GetClientRect.restype = wintypes.BOOL
        u.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(Point)]
        u.ClientToScreen.restype = wintypes.BOOL
        overlay = CandidateOverlay(session)
        previous = None
        binding = None
        red_count = white_count = 0
        refresh = 0
        with (args.output / 'events.jsonl').open('x', encoding='utf-8', buffering=1) as log:
            def record(event, **fields):
                log.write(json.dumps({'event': event, 'mono': time.monotonic(), **fields}) + '\n')
            record('ready', identity=session.identity, sound=str(sound), timer_offset='0x14c', color_offset='0x150')
            print('READY: memory-only red hitsound; white silent; F11 exits.', flush=True)
            deadline = time.monotonic() + 600
            while time.monotonic() < deadline:
                if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
                    raise RuntimeError('APB exited; sound monitor stopped.')
                if u.GetAsyncKeyState(0x7a) & 0x8000:
                    break
                try:
                    session.require_foreground()
                    pointer = struct.unpack('<Q', session.read(base + 0x3bd3340, 8))[0]
                    if not 0x10000 <= pointer < 0x800000000000 or pointer % 8:
                        raise RuntimeError('Cursor pointer unavailable; sound monitor stopped.')
                    header = session.read(pointer, 8)
                    values = session.read(pointer + 0x14c, 20)
                    if session.read(base + 0x3bd3340, 8) != struct.pack('<Q', pointer) or session.read(pointer, 8) != header:
                        previous = None
                        record('sample_rejected', reason='cursor binding changed during read')
                        continue
                    timer, *color = struct.unpack('<5f', values)
                    if not all(math.isfinite(v) for v in (timer, *color)) or not -1 <= timer <= 1.1:
                        raise RuntimeError('Hitmarker candidate values invalid; inspect diagnostic log.')
                    if binding != (pointer, header):
                        binding = pointer, header
                        previous = None
                        record('binding', address=hex(pointer), header=header.hex())
                    session.require_foreground()
                    if previous is not None and timer > 0 and timer > previous + .05:
                        red = all(abs(a-b) < .001 for a, b in zip(color, (1, 0, 0, 1)))
                        white = all(abs(a-b) < .001 for a, b in zip(color, (1, 1, 1, 1)))
                        if red:
                            winsound.PlaySound(str(sound), winsound.SND_FILENAME | winsound.SND_ASYNC | winsound.SND_NODEFAULT)
                            red_count += 1
                        elif white:
                            white_count += 1
                        record('timer_rise', timer=timer, previous=previous, color=color, played=red, white=white)
                    previous = timer
                    if time.monotonic() >= refresh:
                        overlay.place()
                        overlay.canvas.delete('all')
                        overlay.canvas.create_text(18, 24, anchor='nw', fill='#ffdc60', font=('Consolas', 13, 'bold'),
                            text=f'VERUX MEMORY HITSOUND TEST\nRed sounds: {red_count} | White silent: {white_count}\nShoot NPCs, then a white-hitmarker target\nF11: exit')
                        overlay.root.update()
                        refresh = time.monotonic() + .1
                except apb_cursor_scan.ForegroundLost:
                    previous = None
                    overlay.hide()
                time.sleep(.01)
            record('finished', red_sounds=red_count, white_silent=white_count)
    finally:
        if overlay is not None:
            overlay.close()
        session.close()


if __name__ == '__main__':
    main()
