# Fixed APB name detector

The active runner uses `src/apb_detection.rs`. Capture still supplies packed BGR pixels
through the existing DXGI implementation. No capture backend, frame-timeout behavior,
profiling, or automatic fallback is added by this change.

## Color rules

Aim requires R >= 240. Trigger requires R >= 204 (15% lower).
Both require R - G >= 96 and R - B >= 96. These are integer comparisons against
the correct BGR channels. White and yellow pixels with green close to red fail the
same redness condition in both detectors. Thresholds are explicit constants in the
APB module; they are starting values for manual APB validation.

These conditions deliberately replace the old HSV filters. They are not an exact
conversion of the old ranges. The historical channel reversal is absent because
red is read directly from BGR byte 2. Dim red names below the aim brightness floor
will not be aim candidates, even if a previous HSV shade range accepted them.

## Names and coordinates

The aim search retains the previous upper-half region and five-column crosshair
exclusion. Excluded pixels remain empty in the mask; the image is not copied or
squeezed across the missing center columns.

The mask pools two horizontal pixels into one cell and retains vertical resolution.
Each nonzero cell stores which source pixels qualified, allowing exact original
red-pixel X bounds to be recovered after connected-component labeling. No image
upsampling, HSV image, dilation, or erosion is performed in the active path.

Connected components are sorted horizontally. Aligned neighboring letters join
only if their original horizontal gap is at most half the larger component height,
with a minimum allowed gap of two pixels and a maximum of six pixels. Their vertical
overlap must cover at least half the smaller height, and their lower edges must be
within half the larger height. Tiny detached strokes can join an overlapping X span
across a vertical gap of at most two pixels. The comparison uses component bounds,
not an ever-expanding merged rectangle. Buffers are retained between frames.

Groups need at least four qualifying source pixels and three pixels of width and
height. Each group has separate source bounds. The bounding-box midpoint determines
its center. Closest Aim selection minimizes squared screen distance to the original
capture center. Computer Vision movement sends Y=0; Memory uses its separate
body-bound Y correction. Closest Distance is available with the Memory aimbot
source and measures 3D separation from the owned local character.

When a detection frame loses a target, all unsent tracking moves are cancelled.
If aim remains active, one motion tail starts from the last tracking delta and
decreases using the configured ramp duration and easing curve. It is armed only
on the tracking-to-lost transition; subsequent missing frames cannot restart it.
Each missing frame replaces pending tail output with the current decayed output,
and every tail packet carries the same expiration deadline. The sender discards
expired tail packets even if the detection loop stalls. A zero ramp duration
disables the tail. Reacquiring a target cancels the old tail before tracking resumes.
Stateful smoothing and proximity charge reset when tracking stops, so reacquisition
cannot reuse the old target's velocity. Stopping aim activation, blocking, or
shutting down the runner cancels movement and the tail immediately.
The sender holds the movement lock through its serial write to order cancellation
against an extracted mouse step. Commands already written to the serial transport
cannot be recalled; cancellation begins when the detection loop observes loss.
Capture freshness and timeout behavior remain unchanged.

### Ramp curves and short durations

The `S-curve` choice under `Ramp Ease` uses quintic smootherstep:
`6t^5 - 15t^4 + 10t^3` for normalized ramp time `t` in [0, 1]. Its first and second
derivatives are zero at both ends, easing the change in speed into and out of the
ramp. Ramp-down uses one minus that same curve. The existing `smoothstep` option
also has an S shape, but only its first derivative is zero at the endpoints.
New profiles default to `S-curve`; saved profiles retain their selected curve.
The persisted value for the new choice is `smootherstep`.

The runner refreshes the ramp's last-target timestamp while the target is visible.
Previously it never refreshed that timestamp, so the idle reset could restart a
ramp repeatedly even during continuous detection. Idle reset now measures target
absence rather than elapsed ramp time.

Very short ramps still have few sampled updates, and HID movement uses whole-pixel
counts. For example, at a configured 250 detection updates per second a 20 ms ramp
spans roughly five frame intervals. An S curve changes the envelope; it cannot
create more updates or remove integer quantization in the existing movement path.

Color and geometry cannot guarantee separation of two visually adjacent names.
Names on the same line separated by six pixels or less can still join. Distinct
objects already connected in the pooled mask also cannot be split by grouping.
These are the important cases to check manually and use to tune the fixed gap rules.

### Independent aimbot and triggerbot sources

Triggerbot offers Also Activate Aim only when Aimbot Base Activation is Hold
or Timed and Trigger Activation is Hold. The persisted opt-in flag adds the
held trigger key to the aim activation input for either source. It does not
depend on a firing observation: aiming can acquire the target before the
cursor becomes red/green. The aim key remains usable independently. Existing
Timed release, On Aim conditions, disabled-feature checks and Block behavior
still apply; unsupported mode combinations do not activate this link.

Aimbot and Triggerbot each have Computer Vision / Memory source radios. They can be combined in any pairing. Only an enabled feature opens
its selected source; enabled ESP also opens the pawn source independently of
the two input source choices. Color Detection needs DXGI capture; Memory needs EchoDriver.
A selected source that cannot start or stops reading fails visibly rather than
switching to the other source. A Memory/Memory pairing needs DXGI capture only
when predictive braking is enabled.

For Color Detection, triggerbot uses the original five-row screen sampling
strip and fixed red predicate. For Memory, it reads APB's four-byte cursor state
through EchoDriver. For the October 18 update, `130` is white and `48` is green;
red `99` retains the previous build's mapping pending a new red capture.
The pointer path is `APB.exe + 0x3BD3340` to an object with the field
at offset `0x134`; restart validation of this new root remains pending.
See [core update](APB_CORE_UPDATE_20261018.md). The Memory source can
start while APB loads or is paused. A null pointer leaves the runner active,
logs a waiting state, and disables shots until the pointer appears. A driver
read failure or invalid nonzero pointer stops the runner visibly. An unclassified
value does not request a shot, is logged, and stops the runner if it persists
for 500 ms. See `docs/APB_CURSOR_MEMORY.md` for the discovery evidence.

The Triggerbot tab has Memory-only "Shoot on red" and "Shoot on green"
checkboxes. Existing profiles default to red only. Either color or both can be
selected; at least one must be selected. The selected cursor state can keep
firing at the configured delay while it remains visible. Memory mode clears a
pending shot when the cursor leaves the selected state. Color Detection keeps
its fixed red screen predicate and does not use these checkboxes.

The first eligible observation can request a shot immediately when the Arduino is
ready. In Color Detection, a red observation during cooldown retains one pending
shot even if the observation changes. In Memory, the pending shot is kept only
while a selected cursor state remains visible. Repeated eligible observations
coalesce into one pending flag; they do not create a list of clicks to replay.

The sender enforces the configured `Delay` as a minimum interval measured from each
successful serial shoot-command write. It sends at most one shot per polling cycle
and never emits a catch-up burst after a late wakeup. Normal frame, sender, USB, and
game scheduling still apply. The old random addition/subtraction of 40-80 ms is
removed so it cannot shorten or randomly lengthen the configured interval.

Trigger deactivation, blocking, shutdown, port changes, and serial failures clear
the pending shot. A detection is only queued when the Arduino is ready. The sender
holds the same lock through a shoot write so cancellation cannot leave an extracted
request waiting to be sent afterward. Activation/key requirements are unchanged.
The legacy `trigthresh` field remains readable but does not change cursor-state
triggering.
In Memory trigger mode, APB losing focus also clears the pending shot.

## Profile compatibility

Profile files continue to load and save using the existing schema. The existing
`coordinate_source` key remains the aimbot choice. The new `trigger_source`
key stores the triggerbot choice; profiles without it default to Memory, which
preserves the prior EchoDriver-trigger behavior. `trigger_on_red` and
`trigger_on_green` save the Memory trigger state choices and default to true
and false respectively when absent. Legacy HSV,
pixel-acquisition, downscale, highest-pixel, merge-distance, rectangle/area filter,
and X-only fields remain readable for compatibility but do not select or change
the fixed APB detector. Aim-shift Y is also inactive. The related controls have been
removed from the UI so they do not imply changes to the active detector. COM port,
keybinds, capture dimensions, FPS, activation, and licensing keep their existing
behavior. Ramp-up and other motion modifiers still apply during tracking; the
target-loss tail uses the same ramp duration and curve, with bounded output.

The old detector functions remain as legacy source; the runner never switches to
them automatically. Detector settings are reported in the startup log.

## Build and manual validation

Build in the existing checkout and target directory:

```powershell
Get-Process -Name verux_apb,robust_rust -ErrorAction SilentlyContinue | Stop-Process -Force
cargo build --release --bin verux_apb
```

Executable: `target/release/verux_apb.exe`.

No automated tests or timing measurements were added or run for this change. Manually
check bright red names, yellow and white exclusions, dim red trigger candidates,
separate names at different X/Y positions, gaps between letters, a name across the
crosshair exclusion, and center stability as names move by one pixel. Confirm old
profiles with X-only disabled still produce horizontal mouse reports only. While
tracking a name, remove it from view and check that motion ramps down once, reaches
zero, and stays stopped while no target exists. Repeat with Ramp disabled for an
immediate stop. Release aim activation or block the runner during the tail to check
immediate cancellation; reacquire a name on the opposite side to check that the old
tail is cancelled and smoothing starts without old velocity.
Check continuous tracking beyond several idle-reset periods and compare short ramps
with longer ramps using `Ramp Ease: S-curve`. Save/reload the profile to retain it.
Check all four aimbot/triggerbot source pairings and save/reload each choice.
Color/Color should not require EchoDriver; Memory/Memory without predictive
braking should not require DXGI capture. For Memory trigger, check the visible
EchoDriver cursor readout against white, green, and red after a fresh APB launch.
Check red-only, green-only, and both selections. A selected state held through
cooldown should produce one pending shot, with no accumulated burst. Moving to
an unselected state or white should cancel it. Verify that releasing trigger
activation or blocking also cancels it and that sustained selected states
respect the configured firing delay.
