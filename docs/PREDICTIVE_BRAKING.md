# Predictive braking

The Aimbot Behavior section has one control: **Predictive Braking**. Profiles
persist `PredictiveBraking = True/False`; missing values default to False.

With braking enabled, the selected smoothing curve produces the movement request
immediately. There is no probe, hold, or calibration phase. Each successful
Arduino movement write is recorded with its time. For a fresh DXGI image, the
controller reserves movement sent after that image was presented, since it
cannot yet be visible there. It then estimates where the native smoothing
request would land using screen movement observed from ordinary writes.

Braking begins when the proposed movement exceeds half of the predicted
remaining distance. Its strength rises as the predicted landing approaches or
crosses center. The matching curve shapes that increase:

- Linear: linear remaining-distance segment.
- Logarithmic: normalized `d * ln(1 + d)` segment.
- Hyperbolic: monotonic sine-squared approach segment.
- Bezier: quadratic segment with the smoother's control point.

The curve's position is the current error as a percentage of half the horizontal
grab width, so changing X Grab changes the brake shape without a fixed pixel
threshold. A predicted landing reserve is 4–18% of remaining distance, and the
requested count is capped to stay within it. This reserve and the curve replace
the former fixed-pixel zone and count-space midpoint cap. The first correction
uses a provisional 0.5 pixels/count value; successful ordinary movement refines
it. No DPI or game-sensitivity setting is needed. Target motion or response
delay can distort the estimate, so first-frame overshoot remains possible.
Fractional counts carry into later frames.

Within the final 1% of the horizontal half-grab, the selected curve also damps
fractional corrections before they accumulate into mouse counts. If the
observed error crosses center from one side to the other within that zone, the
controller treats the crossing size as local center uncertainty and reserves
it in subsequent landing predictions. That uncertainty clears once the target
moves clearly outside the zone. This suppresses repeated micro reversals
without a fixed-pixel deadband or another GUI setting.

Any sign change waits for the next fresh image and for already-sent
old-direction counts to appear before allowing a confirmed correction. That
correction is initially limited to a curve-shaped 20–50% of the new error,
depending on its percentage of horizontal grab. The crossing itself also
updates the observed movement response, so a genuine overshoot increases the
next brake rather than repeating the same oversized approach. A single large
same-side jump that contradicts successful movement toward center is ignored
while the detector confirms its newly selected target on the next image.

Each fresh frame replaces unsent tracking movement. On target loss, the old
movement is canceled instead of running the usual tail. A stale image also
cancels movement until fresh frames return. A recoverable DXGI access loss
reinitializes the same DXGI capture path and logs the error. Invalid smoothing,
failed capture reinitialization, or unusable sender state stop visibly with a
log entry.

The unchecked path retains the existing smoothing behavior. Live APB/Arduino
feel still requires manual validation; no automated tests or test dependencies
were added for this change.
