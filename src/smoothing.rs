use crate::config::SmoothType;

const INTERPOLATION_FACTOR: f64 = 0.1;
const DAMPING_FACTOR: f64 = 0.9;
const MAX_SPEED: f64 = 50.0;
const PERSISTENCE_THRESHOLD_S: f64 = 0.05;
const TARGET_SWITCH_COOLDOWN_S: f64 = 0.1;
const FOCUS_GAIN_RATE: f64 = 0.05;
const STABILITY_FACTOR: f64 = 0.85;
const ACCELERATION_FACTOR: f64 = 1.5;
const INERTIA_DECAY_BASE: f64 = 0.5;

pub fn clamp_delta(x: i32, y: i32, smoothing: f32) -> (i32, i32) {
    let limit = 50.0 / smoothing.max(f32::MIN_POSITIVE);
    (clamp_one(x, limit), clamp_one(y, limit))
}

pub fn linear(x: i32, y: i32, smoothing: f32) -> (i32, i32) {
    let smoothing = smoothing.max(f32::MIN_POSITIVE);
    ((x as f32 / smoothing) as i32, (y as f32 / smoothing) as i32)
}

pub fn logarithmic(x: i32, y: i32, smoothing: f32) -> (i32, i32) {
    let smoothing = smoothing.max(f32::MIN_POSITIVE) as f64;
    let smooth_one = |v: i32| -> i32 {
        let abs = v.abs() as f64;
        let sign = v.signum() as f64;
        (sign * (abs.ln_1p() / smoothing) * abs) as i32
    };

    (smooth_one(x), smooth_one(y))
}

pub fn hyperbolic(
    x: i32,
    y: i32,
    smoothing: f32,
    width_threshold: i32,
    height_threshold: i32,
) -> (i32, i32) {
    let smoothing_based = smoothing as f64 * 100.0;
    let sinusoid_check = (0.75 * std::f64::consts::PI) * smoothing_based + 5.0;
    let x_farcheck = width_threshold as f64 - (width_threshold as f64 / 3.0);
    let y_farcheck = height_threshold as f64 - (height_threshold as f64 / 3.0);

    (
        hyperbolic_one(
            x,
            smoothing_based,
            sinusoid_check,
            x_farcheck,
            width_threshold as f64,
        ),
        hyperbolic_one(
            y,
            smoothing_based,
            sinusoid_check,
            y_farcheck,
            height_threshold as f64,
        ),
    )
}

pub fn smooth_delta(
    smooth_type: Option<SmoothType>,
    x: i32,
    y: i32,
    smoothing: f32,
    grab_width: i32,
    grab_height: i32,
    bezier: &mut BezierSmoother,
    now_s: f64,
) -> (i32, i32) {
    match smooth_type {
        None => clamp_delta(x, y, smoothing),
        Some(SmoothType::Linear) => linear(x, y, smoothing),
        Some(SmoothType::Logarithmic) => logarithmic(x, y, smoothing),
        Some(SmoothType::Hyperbolic) => hyperbolic(x, y, smoothing, grab_width, grab_height),
        Some(SmoothType::Bezier) => bezier.step(x, y, smoothing, now_s),
    }
}

/// Braking retains sub-count corrections until the final HID quantization. The
/// legacy integer entry points above remain unchanged when braking is disabled.
pub fn smooth_delta_precise(
    smooth_type: Option<SmoothType>,
    x: i32,
    y: i32,
    smoothing: f32,
    grab_width: i32,
    grab_height: i32,
    bezier: &mut BezierSmoother,
    now_s: f64,
) -> (f64, f64) {
    let scale = f64::from(smoothing.max(f32::MIN_POSITIVE));
    match smooth_type {
        None => {
            let limit = 50.0 / scale;
            (
                f64::from(x).clamp(-limit, limit),
                f64::from(y).clamp(-limit, limit),
            )
        }
        Some(SmoothType::Linear) => (f64::from(x) / scale, f64::from(y) / scale),
        Some(SmoothType::Logarithmic) => {
            let curve = |value: i32| {
                let value = f64::from(value);
                value * value.abs().ln_1p() / scale
            };
            (curve(x), curve(y))
        }
        Some(SmoothType::Hyperbolic) => {
            let base = f64::from(smoothing) * 100.0;
            let check = 0.75 * std::f64::consts::PI * base + 5.0;
            (
                hyperbolic_one_precise(
                    x,
                    base,
                    check,
                    f64::from(grab_width) * 2.0 / 3.0,
                    f64::from(grab_width),
                ),
                hyperbolic_one_precise(
                    y,
                    base,
                    check,
                    f64::from(grab_height) * 2.0 / 3.0,
                    f64::from(grab_height),
                ),
            )
        }
        Some(SmoothType::Bezier) => bezier.step_precise(x, y, smoothing, now_s),
    }
}

fn clamp_one(v: i32, limit: f32) -> i32 {
    let sign = v.signum() as f32;
    (sign * (v.abs() as f32).min(limit)) as i32
}

fn hyperbolic_one(
    value: i32,
    smoothing_based: f64,
    sinusoid_check: f64,
    farcheck: f64,
    threshold: f64,
) -> i32 {
    hyperbolic_one_precise(value, smoothing_based, sinusoid_check, farcheck, threshold) as i32
}

fn hyperbolic_one_precise(
    value: i32,
    smoothing_based: f64,
    sinusoid_check: f64,
    farcheck: f64,
    threshold: f64,
) -> f64 {
    let abs = value.abs() as f64;
    let sign = value.signum() as f64;
    let mut out = value as f64;

    if abs > farcheck {
        out = ((abs - farcheck) - (threshold / 3.0)).abs() * sign;
    }
    if farcheck > abs && abs > sinusoid_check {
        out = smoothing_based * sign;
    }
    if sinusoid_check >= abs && abs > 5.0 {
        out = (2.0 * smoothing_based * (abs / smoothing_based).sin().powi(2)) * sign;
    }

    out
}

fn clamp_speed(value: f64) -> f64 {
    value.clamp(-MAX_SPEED, MAX_SPEED)
}

fn lerp(current: f64, target: f64, factor: f64) -> f64 {
    current + factor * (target - current)
}

fn adaptive_damping(distance: f64) -> f64 {
    let distance = distance.min(100.0);
    0.95 + ((DAMPING_FACTOR - 0.95) * (distance / 100.0))
}

fn interp_smoothing(smoothing: f32) -> f64 {
    let t = ((smoothing as f64 - 1.0) / 9.0).clamp(0.0, 1.0);
    0.1 + ((INERTIA_DECAY_BASE - 0.1) * t)
}

#[derive(Debug, Clone)]
pub struct BezierSmoother {
    velocity: (f64, f64),
    last_direction: (i32, i32),
    target_start_time_s: f64,
    last_target_switch_time_s: f64,
    focus_factor: f64,
}

impl BezierSmoother {
    pub fn new(now_s: f64) -> Self {
        Self {
            velocity: (0.0, 0.0),
            last_direction: (0, 0),
            target_start_time_s: now_s,
            last_target_switch_time_s: 0.0,
            focus_factor: 1.0,
        }
    }

    pub fn step(&mut self, rel_x: i32, rel_y: i32, smoothing: f32, now_s: f64) -> (i32, i32) {
        let (x, y) = self.step_precise(rel_x, rel_y, smoothing, now_s);
        (x as i32, y as i32)
    }

    pub fn horizontal_control_ratio(&self, error: i32, smoothing: f32) -> f64 {
        if error == 0 {
            return 0.0;
        }
        // step_precise stores velocity after decay; recover the control point
        // used to draw this frame's Bezier segment before normalizing its brake.
        (self.velocity.0 / interp_smoothing(smoothing) * 0.5 / f64::from(error)).clamp(0.0, 1.0)
    }

    /// Remove momentum that the brake has refused to send, including on crossing.
    pub fn limit_horizontal_velocity(&mut self, allowed: i32) {
        if allowed == 0 || self.velocity.0.signum() != f64::from(allowed).signum() {
            self.velocity.0 = 0.0;
        } else {
            self.velocity.0 = self
                .velocity
                .0
                .clamp(-f64::from(allowed).abs(), f64::from(allowed).abs());
        }
    }

    pub fn step_precise(
        &mut self,
        rel_x: i32,
        rel_y: i32,
        smoothing: f32,
        now_s: f64,
    ) -> (f64, f64) {
        let direction = (rel_x.signum(), rel_y.signum());
        if direction != self.last_direction && direction != (0, 0) {
            if now_s - self.last_target_switch_time_s < TARGET_SWITCH_COOLDOWN_S {
                return (0.0, 0.0);
            }
            if now_s - self.target_start_time_s < PERSISTENCE_THRESHOLD_S {
                return (0.0, 0.0);
            }
            self.target_start_time_s = now_s;
            self.last_direction = direction;
            self.last_target_switch_time_s = now_s;
            self.focus_factor = 1.0;
        }

        let mut rel_x = rel_x as f64;
        let mut rel_y = rel_y as f64;
        let distance = (rel_x.powi(2) + rel_y.powi(2)).sqrt();
        let damping = adaptive_damping(distance);

        self.focus_factor = (self.focus_factor + FOCUS_GAIN_RATE).min(1.0);
        rel_x *= self.focus_factor;
        rel_y *= self.focus_factor;

        self.velocity.0 = lerp(
            self.velocity.0 * ACCELERATION_FACTOR,
            rel_x,
            INTERPOLATION_FACTOR,
        );
        self.velocity.1 = lerp(
            self.velocity.1 * ACCELERATION_FACTOR,
            rel_y,
            INTERPOLATION_FACTOR,
        );

        let p1 = (self.velocity.0 * 0.5, self.velocity.1 * 0.5);
        let p2 = (rel_x, rel_y);
        let smoothing_bezier = 11.0 - smoothing as f64;
        let t = (smoothing_bezier / 10.0).min(1.0);
        let one_minus_t = 1.0 - t;
        let mut smooth_x = 2.0 * one_minus_t * t * p1.0 + t.powi(2) * p2.0;
        let mut smooth_y = 2.0 * one_minus_t * t * p1.1 + t.powi(2) * p2.1;

        smooth_x *= damping * STABILITY_FACTOR;
        smooth_y *= damping * STABILITY_FACTOR;

        let inertia_decay = interp_smoothing(smoothing);
        self.velocity.0 *= inertia_decay;
        self.velocity.1 *= inertia_decay;

        (clamp_speed(smooth_x), clamp_speed(smooth_y))
    }
}

impl Default for BezierSmoother {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_delta_matches_python_none_path() {
        assert_eq!(clamp_delta(100, -100, 2.5), (20, -20));
        assert_eq!(clamp_delta(12, -7, 2.5), (12, -7));
    }

    #[test]
    fn linear_matches_python_move_mouse_smooth() {
        assert_eq!(linear(100, -55, 2.5), (40, -22));
    }

    #[test]
    fn logarithmic_matches_python_vectorized_path() {
        assert_eq!(logarithmic(100, -50, 2.5), (184, -78));
    }

    #[test]
    fn hyperbolic_matches_python_vectorized_path() {
        assert_eq!(hyperbolic(250, -20, 2.5, 500, 500), (354, -3));
    }

    #[test]
    fn bezier_requires_target_persistence_before_switching_direction() {
        let mut smoother = BezierSmoother::new(10.0);
        assert_eq!(smoother.step(30, 0, 5.0, 10.01), (0, 0));
        assert_ne!(smoother.step(30, 0, 5.0, 10.06), (0, 0));
    }
}
