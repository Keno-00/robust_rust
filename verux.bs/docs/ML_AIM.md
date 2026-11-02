# ML Aim personality

ML Aim now uses one personality implementation. The unreleased experimental
controller and recordings are retired; there is no legacy runtime or migration
that invents personality from the old averaged coefficients. Internal schema 2
identifies compatible files, but the UI offers no model-version choices.

## Recording and training

Dataset uses the profile's selected coordinate source (color or memory), stops
live input, and opens a separate native recording window. Train uses complete
recordings for that source. The course source and scenario are visible while
recording. A mismatched model cannot be enabled or started. Color and memory
models remain separate because their vertical goals differ.

The course contains 60 directional acquisitions and ten 15-second continuous
runs. Acquisitions vary distance, target size, left/right direction, and memory
quadrants. Click the initial target to start. Each successful acquisition click
marks its boundary, then the target moves for 1.2 seconds before the next
acquisition. Keep following during this continuation. Reaction periods before
movement remain recorded and are retained by the flick repertoire.

Continuous runs cover sustained following, reversals, changing speed, simulated
approach with growing target size, brief disappearance/reacquisition, discrete
target changes, moving acquisition, and elevation or horizontal stop/start.
Click each run's starting target to begin. Memory adds measured jumps and
approved simulated falls; color uses slow and fast horizontal runs instead.
Each course takes roughly four minutes plus the acquisition/reaction periods.
Scenario scheduling is reproducible; its frequency is a design assumption,
not a measured distribution of APB gameplay.

Color instructions request horizontal intent while retaining both measured
cursor axes. The saved correction goal has zero vertical error, while recorded
vertical cursor velocity is preserved. Memory continuous runs display body
bounds and use the runtime entry/release rule: activate vertical correction
outside the body, correct into the inner half, and release. Visible target
coordinates are stored separately from the correction goal. Samples include
visibility, target identity, raw counts, cursor coordinates, and monotonic time.
Hidden intervals and identity boundaries are excluded from fitting continuous
motion across a discontinuity. They remain in the stored recording.

Jump timing and rise/return shape are embedded in src/ml_jump.rs from the
position trace witness-targeted-second-jump-20260930-1/jump.tsv, samples
0.812 through 2.015 seconds at position offset +0xD0. The normalized first
jump rises approximately 183.48 game units and returns toward baseline.
The full trace's reported range was 203.46217 game units (see APB_F8_SCANS.md).
Screen amplitude and the repeat interval are simulated; measured world height
is not a calibrated screen displacement. Falls and approach are explicitly
labelled simulations.

## Learned movement

src/ml_talisman.rs fits each acquisition independently as a 12-basis DMP
for its along/perpendicular motion. Stored examples retain signed direction,
distance, duration, reaction interval, and initial observed velocity. Selection
ranks examples by direction, normalized distance, and initial velocity; cycles
among up to three nearby demonstrated alternatives (score within 0.25 of the
nearest) across acquisitions. It never synthesizes random jitter. Each chosen
example retains its own curvature and braking rather than a pooled shape.
DMP alpha=25, beta=6.25, phase decay=4, ridge=0.01.

Tracking fits separate horizontal and vertical regressions with 16 inputs:
error on both axes, error derivative on both axes, four recent two-axis velocity
samples, signed squared errors, cross-axis error interaction, and an intercept.
Feature RMS scaling has a 1e-4 floor so intentionally absent vertical errors
in color recordings do not make training fail. This is feature normalization,
not a substitute model. Demonstrations are resampled at 20 ms.

Six-sample (120 ms) residual sequences preserve observed corrections that the
regression does not explain. Runtime selects sequences by the current scaled
context, follows their temporal residuals, and reselects on a sequence boundary
or error sign reversal. Learned residuals continue throughout tracking; there
is no timed fade to an ordinary smoother. Integration and velocity-history
updates use at most 20 ms substeps. Histories include acquisition output before
tracking begins. Training metadata retains source recording IDs, counts,
coefficients, sequences, scales, and training RMSE. RMSE is not an unseen-course
performance score.

## Runtime and preview

Influence blends ordinary and learned output before braking and integer count
conversion. Ramp and Sticky modify the request before ML. Predictive Braking
can limit horizontal movement afterward. Color personality allows its learned
incidental vertical output despite a zero vertical correction request; ordinary
color aim still remains horizontal. Fractional vertical counts are carried by
the shared quantizer, including with braking. Memory still suppresses vertical
output after its body-entry correction releases.

Memory identity changes, target loss, activation release/block, stale braking
frames, and unavailable Arduino input reset the controller. Unsent ML movement
is replaced. Missing/invalid artifacts, invalid coefficients, and stalled or
invalid motion produce visible errors and logs; no other model is substituted.

Preview runs the same personality controller with a one-pixel-per-count virtual
response. Color error remains horizontal while the drawn cursor shows vertical
nuance; memory preview approximates body entry with 36-pixel outer and 18-pixel
inner vertical half-bands. It uses configured FPS, source-aware paths, restart,
pause/resume, and custom targets. Preview is not an APB camera simulator and
does not include all runtime modifiers, capture/serial latency, or real manual
input. Actual game sensitivity, FOV and camera response remain uncalibrated.

Physical hand input is not separately measured at runtime by this repository.
Target error/derivative respond to combined manual camera movement, automated
movement, and target movement; the model cannot identify each contribution.
The recording preserves raw counts for later calibration, but this implementation
learns cursor displacement. There is no claim of measured desktop-to-game
transfer or successful live tracking without a later gameplay evaluation.

## Storage and retirement

New recordings and models live under Documents/VeruxAPB/ml-aim/personality/
(datasets and models subfolders). Old experimental artifacts remain untouched
in the previous directory and are not offered as compatible models. Profiles
referencing them fail visibly until the user selects a new model. The UI explains
retirement and the need for a new course. JSON has explicit verux-apb identity
and internal schema validation; stable IDs, atomic synced writes, rename/delete,
and foreground/display-scale validation are retained.

## Named smoothing curves

The old nonstandard implementations deliberately change. Their profile names
remain readable, but their feel can change. For signed offset `e`, smoothing `s`,
and span `h` equal to half the corresponding grab dimension:

| Name | Precise displacement |
| --- | --- |
| Linear | `e / s` |
| Logarithmic | `sign(e) * ln(1 + abs(e)) / s` |
| Hyperbolic | `h * tanh(e / h) / s` |
| Bezier | Quadratic Bezier with P0=0, P1=0, P2=e, evaluated at t=1/s: `e * t²` |

These are stateless curves. Bezier has no target-persistence or switch delay,
focus factor, acceleration multiplier, adaptive damping, stability multiplier,
inertia, or speed cap. Hyperbolic has no sine-based branches, near/far thresholds,
or piecewise output replacement. Logarithmic no longer multiplies its log curve
by distance. The smoothing-disabled path retains its existing clamp.

Shared transport quantization carries fractional counts across frames so small
logarithmic movements can still reach a target. It resets on cancellation and
target changes; it is not a curve-specific modifier. Existing memory body-entry
correction, explicit Ramp/Sticky, and explicit Predictive Braking remain separate
product behavior. The braking envelopes now use matching log/tanh/Bezier curves.

## Validation

```powershell
cargo check --bin verux_apb
cargo build --release --bin verux_apb
```

No new automated tests, fixtures, or test dependencies were added. Compilation
validates integration, not learned quality. No APB process needs to be launched
for these commands. When convenient, manual acceptance should record each
source course, train and preview its model, verify source mismatch errors,
compare directional acquisitions and sustained sequences, inspect color
vertical nuance and memory vertical release, and exercise interruption/reset.
Live camera response and personality fidelity require later user evaluation.
