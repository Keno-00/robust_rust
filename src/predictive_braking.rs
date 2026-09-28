use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::config::SmoothType;
use crate::log_info;

pub(crate) const MAX_FRAME_AGE: Duration = Duration::from_millis(100);
const WRITE_HISTORY: Duration = Duration::from_millis(500);
const MICRO_GRAB_FRACTION: f64 = 0.01;

#[derive(Clone, Copy)]
struct SentWrite {
    at: Instant,
    total: i64,
}

/// Successful Arduino writes only. This is not a future movement queue: it
/// tells the controller which already-sent counts cannot yet be in an image.
pub(crate) struct SentMotionHistory {
    writes: VecDeque<SentWrite>,
    origin: Instant,
    base_total: i64,
    total: i64,
    generation: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct MotionSnapshot {
    visible_total: i64,
    pending: i64,
    generation: u64,
}

impl SentMotionHistory {
    pub fn new() -> Self {
        Self {
            writes: VecDeque::new(),
            origin: Instant::now(),
            base_total: 0,
            total: 0,
            generation: 0,
        }
    }

    pub fn reset(&mut self) {
        self.writes.clear();
        self.origin = Instant::now();
        self.base_total = 0;
        self.total = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn record(&mut self, dx: i32, at: Instant) {
        self.total += i64::from(dx);
        self.writes.push_back(SentWrite {
            at,
            total: self.total,
        });
        while self.writes.len() > 1
            && at.saturating_duration_since(self.writes[1].at) > WRITE_HISTORY
        {
            let old = self.writes.pop_front().unwrap();
            self.origin = old.at;
            self.base_total = old.total;
        }
    }

    pub fn snapshot(&self, presented_at: Instant) -> Option<MotionSnapshot> {
        let visible_total = if presented_at < self.origin {
            (self.base_total == 0).then_some(0)?
        } else {
            let index = self
                .writes
                .partition_point(|write| write.at <= presented_at);
            if index == 0 {
                self.base_total
            } else {
                self.writes[index - 1].total
            }
        };
        Some(MotionSnapshot {
            visible_total,
            pending: self.total - visible_total,
            generation: self.generation,
        })
    }
}

#[derive(Clone, Copy)]
struct Observation {
    at: Instant,
    error: f64,
    y: i32,
    width: i32,
    height: i32,
    visible_total: i64,
}

/// Computes a future landing from the selected smoother, observed response,
/// and writes newer than the current image. No probe or hold is required.
pub(crate) struct PredictiveBrake {
    previous: Option<Observation>,
    pixels_per_count: Option<f64>,
    closing_speed: f64,
    generation: Option<u64>,
    micro_jitter: f64,
    pending_turn: Option<f64>,
    reverse_limit: Option<(f64, f64)>,
    jump_candidate: bool,
    last_turn_log: Option<Instant>,
    remainder: f64,
    direction: f64,
}

impl PredictiveBrake {
    pub fn new() -> Self {
        Self {
            previous: None,
            pixels_per_count: None,
            closing_speed: 0.0,
            generation: None,
            micro_jitter: 0.0,
            pending_turn: None,
            reverse_limit: None,
            jump_candidate: false,
            last_turn_log: None,
            remainder: 0.0,
            direction: 0.0,
        }
    }

    pub fn reset_tracking(&mut self) {
        self.previous = None;
        self.closing_speed = 0.0;
        self.micro_jitter = 0.0;
        self.pending_turn = None;
        self.reverse_limit = None;
        self.jump_candidate = false;
        self.remainder = 0.0;
        self.direction = 0.0;
    }

    fn observe_gain(&mut self, travel: f64, sent_toward: i64, crossed_center: bool) {
        if travel <= 0.0 || sent_toward <= 0 {
            return;
        }
        let sample = travel / sent_toward as f64 * 1.1;
        if !(0.05..=4.0).contains(&sample) {
            return;
        }
        self.pixels_per_count = Some(match self.pixels_per_count {
            None => {
                log_info!("Predictive braking: first ordinary response sample {:.3} px/count; adapting the provisional 0.5 px/count estimate.", sample);
                0.65 * 0.5 + 0.35 * sample
            }
            // Crossing center proves the prior command was stronger than the
            // model allowed. Raise the estimate promptly to prevent repetition.
            Some(old) if crossed_center => old.max(0.2 * old + 0.8 * sample),
            Some(old) if sample > old => 0.4 * old + 0.6 * sample,
            Some(old) => 0.8 * old + 0.2 * sample,
        });
    }

    pub fn command(
        &mut self,
        error: i32,
        y: i32,
        width: i32,
        height: i32,
        desired: f64,
        kind: Option<SmoothType>,
        smoothing: f32,
        bezier_control: f64,
        grab_width: i32,
        presented_at: Instant,
        now: Instant,
        motion: MotionSnapshot,
    ) -> Result<i32, String> {
        if !desired.is_finite() || !smoothing.is_finite() || smoothing <= 0.0 || grab_width <= 0 {
            return Err("Predictive braking stopped: movement, smoothing, or horizontal grab size is invalid. Check the profile and restart.".into());
        }
        if now.saturating_duration_since(presented_at) > MAX_FRAME_AGE {
            self.reset_tracking();
            return Ok(0);
        }
        if self.generation != Some(motion.generation) {
            self.reset_tracking();
            self.pixels_per_count = None;
            self.generation = Some(motion.generation);
            log_info!("Predictive braking: serial transport changed; response estimate reset while movement remains immediate.");
        }

        let current = Observation {
            at: presented_at,
            error: f64::from(error),
            y,
            width,
            height,
            visible_total: motion.visible_total,
        };
        let half_grab = f64::from(grab_width) * 0.5;
        let micro_extent = half_grab * MICRO_GRAB_FRACTION;
        if let Some(previous) = self.previous {
            let target_changed = (y - previous.y).abs() > height.max(previous.height).max(4)
                || width > previous.width.saturating_mul(2)
                || previous.width > width.saturating_mul(2)
                || height > previous.height.saturating_mul(2)
                || previous.height > height.saturating_mul(2);
            if target_changed {
                self.reset_tracking();
            } else if current.error.signum() != previous.error.signum() {
                // Repeated tiny side-to-side corrections reveal a local
                // uncertainty in the observed center. Keep that uncertainty
                // until the target clearly leaves this percentage-based zone.
                let crossed_center = previous.error != 0.0 && current.error != 0.0;
                if crossed_center {
                    let sent_toward = (motion.visible_total - previous.visible_total)
                        * previous.error.signum() as i64;
                    self.observe_gain(
                        previous.error.abs() + current.error.abs(),
                        sent_toward,
                        true,
                    );
                }
                let crossed_near_center = crossed_center
                    && previous.error.abs() <= micro_extent
                    && current.error.abs() <= micro_extent;
                let jitter = if crossed_near_center {
                    self.micro_jitter
                        .max((previous.error.abs() + current.error.abs()) * 0.5)
                } else {
                    0.0
                };
                self.reset_tracking();
                self.micro_jitter = jitter;
                if crossed_center {
                    self.pending_turn = Some(current.error.signum());
                    self.previous = Some(current);
                    if self.last_turn_log.is_none_or(|last| {
                        now.saturating_duration_since(last) >= Duration::from_secs(1)
                    }) {
                        log_info!("Predictive braking: observed direction change from {:.0} to {:.0} px ({:.2}% of horizontal half-grab); gain estimate {:.3} px/count, {} already-sent counts outside this image. Awaiting a fresh image before correcting.", previous.error, current.error, current.error.abs() / half_grab * 100.0, self.pixels_per_count.unwrap_or(0.5), motion.pending);
                        self.last_turn_log = Some(now);
                    }
                    return Ok(0);
                }
            } else if presented_at > previous.at {
                let dt = presented_at.duration_since(previous.at).as_secs_f64();
                if (0.002..=0.08).contains(&dt) {
                    let closed = previous.error.abs() - current.error.abs();
                    self.closing_speed = if closed > 0.0 { closed / dt } else { 0.0 };
                    let sent_toward =
                        (motion.visible_total - previous.visible_total) * i64::from(error.signum());
                    let jumped_away = -closed > half_grab * 0.025
                        && -closed > previous.error.abs() * 0.25
                        && sent_toward > 0;
                    if jumped_away && !self.jump_candidate {
                        // A target that suddenly jumps away despite successful
                        // movement toward it may be a different name selected
                        // by the detector. Do not chase that one image.
                        if self.last_turn_log.is_none_or(|last| {
                            now.saturating_duration_since(last) >= Duration::from_secs(1)
                        }) {
                            log_info!("Predictive braking: deferred contradictory target jump from {:.0} to {:.0} px while {} successful counts moved toward center; confirming the next image.", previous.error, current.error, sent_toward);
                            self.last_turn_log = Some(now);
                        }
                        self.reset_tracking();
                        self.previous = Some(current);
                        self.jump_candidate = true;
                        self.remainder = 0.0;
                        return Ok(0);
                    }
                    self.jump_candidate = false;
                    self.observe_gain(closed, sent_toward, false);
                } else {
                    self.closing_speed = 0.0;
                }
            }
        }
        self.previous = Some(current);

        let mut confirmed_turn = false;
        if let Some(turn) = self.pending_turn {
            if current.error == 0.0 || current.error.signum() != turn {
                self.pending_turn = None;
            } else if motion.pending as f64 * turn < 0.0 {
                // The old-direction writes are already sent but are not yet in
                // this image. Wait for their effect before correcting backward.
                self.remainder = 0.0;
                return Ok(0);
            } else {
                self.pending_turn = None;
                confirmed_turn = true;
            }
        }

        let direction = desired.signum();
        if error == 0 || direction == 0.0 || direction != current.error.signum() {
            self.remainder = 0.0;
            return Ok(0);
        }
        if direction != self.direction {
            self.remainder = 0.0;
            self.direction = direction;
        }

        // The count conversion starts provisionally, then comes from ordinary
        // successful writes observed in later images. This is not DPI input.
        let gain = self.pixels_per_count.unwrap_or(0.5);
        let distance = current.error.abs();
        if confirmed_turn {
            self.reverse_limit = Some((direction, distance));
        }
        if distance > micro_extent * 2.0 {
            self.micro_jitter = 0.0;
        }
        let pending_toward = motion.pending as f64 * direction;
        let image_age = now.saturating_duration_since(presented_at).as_secs_f64();
        let velocity_reserve = (self.closing_speed * image_age * 0.25).min(distance * 0.2);
        let future_error =
            (distance - gain * pending_toward - velocity_reserve - self.micro_jitter)
                .clamp(0.0, distance * 2.0);
        if future_error <= gain * 0.5 {
            self.remainder = 0.0;
            return Ok(0);
        }

        let position = (distance / half_grab).clamp(0.0, 1.0);
        let position_curve = brake_envelope(kind, position, half_grab, smoothing, bezier_control);
        // All thresholds are fractions of the predicted remaining error. The
        // chosen curve and horizontal grab size shape the final reserve.
        let reserve = 0.04 + 0.14 * position_curve;
        let native_travel = desired.abs() * gain;
        let risk = native_travel / future_error;
        let fraction = if risk <= 0.5 {
            1.0
        } else {
            let remaining = ((1.0 - risk) * 2.0).clamp(0.0, 1.0);
            0.8 + 0.2 * brake_envelope(kind, remaining, half_grab, smoothing, bezier_control)
        };
        let safe_counts = future_error * (1.0 - reserve) / gain;
        // In the last one percent of horizontal grab, the same selected curve
        // increasingly damps sub-count corrections. This is a percentage of
        // the configured view, never a fixed number of screen pixels.
        let micro_remaining = (future_error / micro_extent).clamp(0.0, 1.0);
        let micro_curve = brake_envelope(
            kind,
            micro_remaining,
            micro_extent,
            smoothing,
            bezier_control,
        );
        let micro_factor = 0.25 + 0.75 * micro_curve;
        let mut limited = (desired.abs() * fraction * micro_factor).min(safe_counts);
        if let Some((turn_direction, start_error)) = self.reverse_limit {
            if direction != turn_direction
                || distance < start_error * 0.5
                || distance > start_error * 1.5
            {
                self.reverse_limit = None;
            } else {
                // After a confirmed crossing, approach the new side by a
                // curve-shaped percentage of its error. This prevents the
                // correction itself from becoming a large backward snap.
                let reverse_fraction = 0.2 + 0.3 * position_curve;
                limited = limited.min(distance * reverse_fraction / gain);
            }
        }
        let count_limit = (safe_counts + 0.5).floor().min(i32::MAX as f64);
        let amount = limited + self.remainder;
        let command = amount.floor().min(count_limit);
        self.remainder = if command < count_limit {
            amount - command
        } else {
            0.0
        };
        Ok((command as i32) * direction as i32)
    }
}

fn brake_envelope(
    kind: Option<SmoothType>,
    remaining: f64,
    span_px: f64,
    smoothing: f32,
    bezier_control: f64,
) -> f64 {
    let r = remaining.clamp(0.0, 1.0);
    match kind {
        None | Some(SmoothType::Linear) => r,
        Some(SmoothType::Logarithmic) => r * (span_px * r).ln_1p() / span_px.ln_1p(),
        Some(SmoothType::Hyperbolic) => {
            let angle = (span_px / (f64::from(smoothing) * 100.0)).min(std::f64::consts::FRAC_PI_2);
            if angle < 1e-6 {
                r * r
            } else {
                (angle * r).sin().powi(2) / angle.sin().powi(2)
            }
        }
        Some(SmoothType::Bezier) => {
            let t = ((11.0 - f64::from(smoothing)) / 10.0).clamp(0.0, 1.0);
            let control = bezier_control.clamp(0.0, 1.0);
            let curve = |p: f64| 2.0 * (1.0 - p) * p * control + p * p;
            if t < 1e-6 {
                r * r
            } else {
                curve(r * t) / curve(t)
            }
        }
    }
    .clamp(0.0, 1.0)
}
