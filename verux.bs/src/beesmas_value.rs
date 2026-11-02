//! Stateless named curves. Ramp, Sticky, ML and Predictive Braking are separate.
#[cfg(test)]
use crate::config::BeesmasType;

#[cfg(test)]
pub fn clamp_delta(x: i32, y: i32, beesmas_value: f32) -> (i32, i32) {
    let limit = 50.0 / f64::from(beesmas_value.max(f32::MIN_POSITIVE));
    (
        f64::from(x).clamp(-limit, limit) as i32,
        f64::from(y).clamp(-limit, limit) as i32,
    )
}

#[cfg(test)]
pub fn linear(x: i32, y: i32, beesmas_value: f32) -> (i32, i32) {
    let s = f64::from(beesmas_value.max(f32::MIN_POSITIVE));
    ((f64::from(x) / s) as i32, (f64::from(y) / s) as i32)
}

#[cfg(test)]
pub fn logarithmic(x: i32, y: i32, beesmas_value: f32) -> (i32, i32) {
    let s = f64::from(beesmas_value.max(f32::MIN_POSITIVE));
    let curve = |v: i32| f64::from(v).signum() * f64::from(v).abs().ln_1p() / s;
    (curve(x) as i32, curve(y) as i32)
}

#[cfg(test)]
pub fn hyperbolic(x: i32, y: i32, beesmas_value: f32, width: i32, height: i32) -> (i32, i32) {
    let curve = |v: i32, span: i32| {
        let span = f64::from(span.max(1)) * 0.5;
        span * (f64::from(v) / span).tanh() / f64::from(beesmas_value.max(f32::MIN_POSITIVE))
    };
    (curve(x, width) as i32, curve(y, height) as i32)
}

#[cfg(test)]
pub fn beesmas_delta(
    kind: Option<BeesmasType>,
    x: i32,
    y: i32,
    beesmas_value: f32,
    width: i32,
    height: i32,
    bezier: &mut BezierBeesmaser,
    now: f64,
) -> (i32, i32) {
    let (x, y) = beesmas_delta_precise(kind, x, y, beesmas_value, width, height, bezier, now);
    (x as i32, y as i32)
}

#[cfg(test)]
pub fn beesmas_delta_precise(
    kind: Option<BeesmasType>,
    x: i32,
    y: i32,
    beesmas_value: f32,
    width: i32,
    height: i32,
    bezier: &mut BezierBeesmaser,
    now: f64,
) -> (f64, f64) {
    let s = f64::from(beesmas_value.max(f32::MIN_POSITIVE));
    let axis = |v: i32, span: i32| {
        let v = f64::from(v);
        match kind {
            None => v.clamp(-50.0 / s, 50.0 / s),
            Some(BeesmasType::Linear) => v / s,
            Some(BeesmasType::Logarithmic) => v.signum() * v.abs().ln_1p() / s,
            Some(BeesmasType::Hyperbolic) => {
                let span = f64::from(span.max(1)) * 0.5;
                span * (v / span).tanh() / s
            }
            Some(BeesmasType::Bezier) => {
                unreachable!("Bezier is evaluated by the pure quadratic curve below")
            }
        }
    };
    if kind == Some(BeesmasType::Bezier) {
        bezier.step_precise(x, y, beesmas_value, now)
    } else {
        (axis(x, width), axis(y, height))
    }
}

/// A quadratic Bezier segment: P0=0, P1=0, P2=target offset; t=1/beesmas_value.
/// This intentionally holds no velocity, timers, direction, damping, or caps.
#[derive(Debug, Clone, Default)]
pub struct BezierBeesmaser;

impl BezierBeesmaser {
    pub fn new(_now: f64) -> Self {
        Self
    }

    #[cfg(test)]
    pub fn step(&mut self, x: i32, y: i32, beesmas_value: f32, now: f64) -> (i32, i32) {
        let (x, y) = self.step_precise(x, y, beesmas_value, now);
        (x as i32, y as i32)
    }

    pub fn horizontal_control_ratio(&self, _error: i32, _beesmas_value: f32) -> f64 {
        0.0
    }

    #[cfg(test)]
    pub fn step_precise(&mut self, x: i32, y: i32, beesmas_value: f32, _now: f64) -> (f64, f64) {
        let t = 1.0 / f64::from(beesmas_value.max(1.0));
        (t * t * f64::from(x), t * t * f64::from(y))
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
    fn linear_matches_python_move_mouse_beesmas() {
        assert_eq!(linear(100, -55, 2.5), (40, -22));
    }

    #[test]
    fn logarithmic_is_only_the_signed_log_curve() {
        assert_eq!(logarithmic(100, -50, 2.5), (1, -1));
    }

    #[test]
    fn hyperbolic_is_only_the_scaled_tanh_curve() {
        assert_eq!(hyperbolic(250, -20, 2.5, 500, 500), (76, -7));
    }

    #[test]
    fn bezier_is_immediate_and_has_no_direction_switch_delay() {
        let mut beesmaser = BezierBeesmaser::new(10.0);
        assert_eq!(beesmaser.step(100, 0, 5.0, 10.01), (4, 0));
        assert_eq!(beesmaser.step(-100, 0, 5.0, 10.02), (-4, 0));
    }
}
