"""Explicit diagnostic recording controls; does not send game input."""

import time

from apb_cursor_scan import ForegroundLost


F8, F10, F11 = 0x77, 0x79, 0x7A


def down(session, key):
    return bool(session.user32.GetAsyncKeyState(key) & 0x8000)


def wait_start(session, instruction):
    print(f"\nIDLE: {instruction}. F8 starts recording; F11 fully exits. "
          "During recording: F10 pause/resume, F8 stop/save, F9 screenshot.", flush=True)
    previous = down(session, F8)
    while True:
        if down(session, F11):
            return False
        if session.kernel32.WaitForSingleObject(session.process, 0) != 258:
            raise RuntimeError("APB exited while waiting to record")
        pressed = down(session, F8)
        if pressed and not previous:
            try:
                session.require_foreground()
            except ForegroundLost:
                print("F8 ignored: refocus APB before starting.", flush=True)
            else:
                while down(session, F8):
                    if down(session, F11):
                        return False
                    time.sleep(0.05)
                return True
        previous = pressed
        time.sleep(0.05)
