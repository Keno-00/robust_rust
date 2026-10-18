"""Operator-labelled, read-only red/white hitmarker capture; no production detector."""
import argparse, ctypes, json, struct, time
from pathlib import Path
from datetime import datetime, timezone
from ctypes import wintypes
from PIL import ImageGrab
from apb_cursor_scan import Session, ForegroundLost
from apb_cursor_modules import module_base
from apb_validation_runtime import ValidationRuntime, BUILD_SHA256, RuntimeChanged
from apb_runtime_audit import kind, u64
from apb_manual_labels import instance_signature
from apb_candidate_overlay import CandidateOverlay
from apb_f8_common import Point, Rect


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid',type=int);parser.add_argument('output',type=Path)
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
    session=Session(args.pid)
    overlay=None
    try:
        if session.identity['image_sha256'] != BUILD_SHA256: raise RuntimeError('Hitmarker capture requires the audited executable hash.')
        base=module_base(session)
        runtime=ValidationRuntime(session,{'format':'apb-build-candidates-v1','identity':session.identity,'module_base':hex(base)})
        camera,hud,binding=runtime.camera()
        world=u64(hud,0x2e0);control=u64(runtime.read(world,0x360),0x358)
        cb=runtime.typed(control,'controller',0x798);pawn=u64(cb,0x790)
        runtime.typed(pawn,'pawn')
        refs=[('hud',binding[3],0x1000),('controller',control,0x1400),('pawn',pawn,0x800)]
        subjects={label:{'address':hex(address),'probe_bytes':size,'signature':instance_signature(runtime.read(address,64))} for label,address,size in refs}
        u=session.user32
        u.GetClientRect.argtypes=[ctypes.c_void_p,ctypes.POINTER(Rect)];u.GetClientRect.restype=wintypes.BOOL
        u.ClientToScreen.argtypes=[ctypes.c_void_p,ctypes.POINTER(Point)];u.ClientToScreen.restype=wintypes.BOOL
        def down(key):return bool(u.GetAsyncKeyState(key)&0x8000)
        def roi():
            session.require_foreground();hwnd=u.GetForegroundWindow();rect=Rect();origin=Point()
            if not u.GetClientRect(hwnd,ctypes.byref(rect)) or not u.ClientToScreen(hwnd,ctypes.byref(origin)):raise ctypes.WinError(ctypes.get_last_error())
            width,height=rect.right-rect.left,rect.bottom-rect.top
            if width<256 or height<256:raise RuntimeError('APB client too small for centre capture.')
            cx,cy=origin.x+width//2,origin.y+height//2
            image=ImageGrab.grab(bbox=(cx-96,cy-96,cx+96,cy+96),all_screens=True).convert('RGB')
            session.require_foreground()
            return image,[width,height]
        overlay=CandidateOverlay(session)
        def guide(message):
            try:
                overlay.place();overlay.canvas.delete('all')
                overlay.canvas.create_text(18,24,text='VERUX HITMARKER CAPTURE\n'+message+'\nF11: exit',anchor='nw',fill='#ffdc60',font=('Consolas',13,'bold'))
                overlay.root.update()
            except ForegroundLost:
                overlay.hide()
        def next_step():
            if 'idle' not in completed:return 'STEP 1/3 - F8: start IDLE (do not shoot)'
            if 'red' not in completed:return 'STEP 2/3 - F6: start RED (shoot NPCs)'
            return 'STEP 3/3 - F7: start WHITE (shoot white-hitmarker target)'
        completed={};previous={key:down(key) for key in (0x75,0x76,0x77)}
        with (args.output/'events.jsonl').open('x',encoding='utf-8',buffering=1) as stream:
            def log(event,**fields):stream.write(json.dumps({'utc':datetime.now(timezone.utc).isoformat(),'mono':time.monotonic(),'event':event,**fields})+'\n')
            log('ready',identity=session.identity,subjects=subjects,world=hex(world),scope='Bounded object probes; sizes are not verified allocations. Centre ROI is assumed crosshair region; red/white counts are diagnostics, not accepted triggers.')
            (args.output/'ready.json').write_text(json.dumps({'identity':session.identity,'keys':{'F8':'idle no shooting','F6':'red NPC hitmarkers','F7':'white hitmarkers','F11':'exit'},'seconds_per_phase':8}),encoding='utf-8')
            print('READY F8 idle; F6 red NPC hits; F7 white hits. Each records 8 seconds. F11 exits.',flush=True)
            deadline=time.monotonic()+600
            while time.monotonic()<deadline:
                if session.kernel32.WaitForSingleObject(session.process,0)!=258:raise RuntimeError('APB exited.')
                if down(0x7a):break
                chosen=None
                for key,label in ((0x77,'idle'),(0x75,'red'),(0x76,'white')):
                    pressed=down(key)
                    if pressed and not previous[key]:chosen=label
                    previous[key]=pressed
                if chosen is None:
                    guide(next_step());time.sleep(.05);continue
                try:session.require_foreground()
                except ForegroundLost:continue
                if chosen in completed:
                    print('Already recorded '+chosen+'; use remaining phase keys or F11.',flush=True);continue
                print('RECORDING '+chosen,flush=True);log('phase_start',phase=chosen)
                phase_end=time.monotonic()+8;tick=0
                while time.monotonic()<phase_end:
                    if down(0x7a):raise RuntimeError('Operator aborted active phase; incomplete phase retained as diagnostic only.')
                    start=time.monotonic()
                    instructions={'idle':'DO NOT SHOOT','red':'SHOOT NPCs - RED HITMARKERS','white':'SHOOT WHITE-HITMARKER TARGET'}
                    guide(f'{chosen.upper()}: {instructions[chosen]} | {max(0,int(phase_end-start)+1)} seconds left')
                    try:
                        session.require_foreground();image,dimensions=roi();objects=[]
                        if u64(runtime.read(hud_address:=binding[3],0x2e8),0x2e0)!=world:raise RuntimeError('World changed; repeat capture.')
                        for label,address,size in refs:
                            body=runtime.read(address,size)
                            if instance_signature(body)!=subjects[label]['signature']:raise RuntimeError(label+' instance changed; repeat capture.')
                            objects.append({'label':label,'body_hex':body.hex()})
                        session.require_foreground()
                    except ForegroundLost:
                        log('phase_aborted',phase=chosen,reason='foreground lost');print('Phase aborted: refocus APB and press its phase key again.',flush=True);break
                    filename=f'{chosen}-{tick:04d}.png';image.save(args.output/filename)
                    pixels=list(image.getdata());red=sum(r>150 and r>g*1.6 and r>b*1.6 for r,g,b in pixels);white=sum(min(r,g,b)>205 and max(r,g,b)-min(r,g,b)<30 for r,g,b in pixels)
                    log('sample',phase=chosen,tick=tick,image=filename,dimensions=dimensions,left_mouse=down(1),red_pixels=red,white_pixels=white,world_time=struct.unpack('<d',runtime.read(world+0x6a0,8))[0],objects=objects)
                    tick+=1;time.sleep(max(0,1/20-(time.monotonic()-start)))
                else:
                    completed[chosen]=tick;log('phase_complete',phase=chosen,samples=tick);print('SAVED '+chosen+' '+str(tick)+' frames',flush=True)
                    if len(completed)==3:
                        final_deadline=time.monotonic()+8
                        while time.monotonic()<final_deadline and not down(0x7a):
                            guide('ALL 3 PHASES SAVED - return to chat');time.sleep(.1)
                        break
            log('finished',completed=completed)
            (args.output/'summary.json').write_text(json.dumps({'identity':session.identity,'completed':completed},indent=2),encoding='utf-8')
    finally:
        if overlay is not None:overlay.close()
        session.close()

if __name__=='__main__':main()
