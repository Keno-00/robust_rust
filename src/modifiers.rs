use crate::config::{EaseCurve, ProximityEffect, RampApply};

pub fn ease_factor(t: f64, curve: EaseCurve) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let t = if (1.0 - t).abs() < f64::EPSILON * 32.0 {
        1.0
    } else {
        t
    };
    match curve {
        EaseCurve::Linear => t,
        EaseCurve::Smoothstep => t * t * (3.0 - (2.0 * t)),
        // Quintic S curve: first and second derivatives vanish at both endpoints.
        EaseCurve::Smootherstep => t * t * t * (t * (6.0 * t - 15.0) + 10.0),
        EaseCurve::Quad => t * t,
        EaseCurve::Cubic => t * t * t,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RampController {
    pub ramp_time_ms: u32,
    pub idle_reset_ms: u32,
    pub mode: RampApply,
    pub ease: EaseCurve,
    armed: bool,
    started_at_s: f64,
    last_target_at_s: f64,
}

impl RampController {
    pub fn new(ramp_time_ms: u32, idle_reset_ms: u32, mode: RampApply, ease: EaseCurve) -> Self {
        Self {
            ramp_time_ms,
            idle_reset_ms,
            mode,
            ease,
            armed: false,
            started_at_s: 0.0,
            last_target_at_s: 0.0,
        }
    }

    pub fn arm(&mut self, now_s: f64) {
        self.armed = true;
        self.started_at_s = now_s;
        self.last_target_at_s = now_s;
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn note_target_seen(&mut self, now_s: f64) {
        self.last_target_at_s = now_s;
    }

    pub fn check_idle_reset(&mut self, now_s: f64) {
        if !self.armed || self.idle_reset_ms == 0 {
            return;
        }

        let idle_ms = (now_s - self.last_target_at_s) * 1000.0;
        if idle_ms >= f64::from(self.idle_reset_ms) {
            self.disarm();
        }
    }

    pub fn factor(&self, now_s: f64) -> f64 {
        if !self.armed {
            return 0.0;
        }
        if self.ramp_time_ms == 0 {
            return 1.0;
        }

        let t = ((now_s - self.started_at_s) * 1000.0) / f64::from(self.ramp_time_ms);
        ease_factor(t, self.ease)
    }

    pub fn apply(&self, dx: i32, dy: i32, smoothing: f32, now_s: f64) -> (i32, i32, f32) {
        let factor = self.factor(now_s);
        match self.mode {
            RampApply::Delta => (
                (f64::from(dx) * factor).round() as i32,
                (f64::from(dy) * factor).round() as i32,
                smoothing,
            ),
            RampApply::Smoothing => {
                let smoothing = f64::from(smoothing);
                let smoothing_start = smoothing.max(0.0) * 4.0;
                let effective = smoothing_start + (smoothing - smoothing_start) * factor;
                (dx, dy, effective as f32)
            }
        }
    }
}

impl Default for RampController {
    fn default() -> Self {
        Self::new(0, 350, RampApply::Delta, EaseCurve::Linear)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecayController {
    pub ramp_time_ms: u32,
    pub ease: EaseCurve,
    armed: bool,
    started_at_s: f64,
    dx0: f64,
    dy0: f64,
}

impl DecayController {
    pub fn new(ramp_time_ms: u32, ease: EaseCurve) -> Self {
        Self {
            ramp_time_ms,
            ease,
            armed: false,
            started_at_s: 0.0,
            dx0: 0.0,
            dy0: 0.0,
        }
    }

    pub fn arm(&mut self, dx0: i32, dy0: i32, now_s: f64) {
        if dx0 == 0 && dy0 == 0 {
            self.armed = false;
            return;
        }

        self.dx0 = f64::from(dx0);
        self.dy0 = f64::from(dy0);
        self.started_at_s = now_s;
        self.armed = true;
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }

    pub fn is_active(&self) -> bool {
        self.armed
    }

    pub fn delta(&mut self, now_s: f64) -> (i32, i32, bool) {
        if !self.armed || self.ramp_time_ms == 0 {
            self.armed = false;
            return (0, 0, false);
        }

        let t = ((now_s - self.started_at_s) * 1000.0) / f64::from(self.ramp_time_ms);
        if t >= 1.0 {
            self.armed = false;
            return (0, 0, false);
        }

        let factor = 1.0 - ease_factor(t, self.ease);
        let dx = (self.dx0 * factor).round() as i32;
        let dy = (self.dy0 * factor).round() as i32;
        if dx == 0 && dy == 0 {
            self.armed = false;
            return (0, 0, false);
        }

        (dx, dy, true)
    }
}

impl Default for DecayController {
    fn default() -> Self {
        Self::new(0, EaseCurve::Linear)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProximityCharge {
    pub enabled: bool,
    pub distance: i32,
    pub build_ms: u32,
    pub effect: ProximityEffect,
    pub ease: EaseCurve,
}

impl Default for ProximityCharge {
    fn default() -> Self {
        Self {
            enabled: false,
            distance: 50,
            build_ms: 700,
            effect: ProximityEffect::Strength,
            ease: EaseCurve::Smoothstep,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProximityChargeState {
    pub started_at_s: f64,
    pub factor: f64,
}

impl ProximityChargeState {
    pub fn update(
        &mut self,
        config: ProximityCharge,
        aim_active: bool,
        dx: i32,
        dy: i32,
        now_s: f64,
    ) {
        if !config.enabled || !aim_active {
            self.reset();
            return;
        }

        let in_range = dx.abs() <= config.distance && dy.abs() <= config.distance;
        if !in_range {
            self.reset();
            return;
        }

        if self.started_at_s <= 0.0 {
            self.started_at_s = now_s;
        }

        self.factor = if config.build_ms == 0 {
            1.0
        } else {
            let elapsed = ((now_s - self.started_at_s) * 1000.0) / f64::from(config.build_ms);
            ease_factor(elapsed, config.ease)
        };
    }

    pub fn apply(
        &self,
        dx: i32,
        dy: i32,
        smoothing: f32,
        effect: ProximityEffect,
    ) -> (i32, i32, f32) {
        if self.factor <= 0.0 {
            return (dx, dy, smoothing);
        }

        match effect {
            ProximityEffect::Strength => {
                let charge_strength = 1.0 + (0.35 * self.factor);
                (
                    (f64::from(dx) * charge_strength).round() as i32,
                    (f64::from(dy) * charge_strength).round() as i32,
                    smoothing,
                )
            }
            ProximityEffect::Smoothing => {
                let effective = f64::from(smoothing) * (1.0 - (0.35 * self.factor));
                (dx, dy, effective.max(1.0) as f32)
            }
        }
    }

    pub fn reset(&mut self) {
        self.started_at_s = 0.0;
        self.factor = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_curves_match_python_shapes() {
        assert_eq!(ease_factor(0.5, EaseCurve::Linear), 0.5);
        assert_eq!(ease_factor(0.5, EaseCurve::Smoothstep), 0.5);
        assert_eq!(ease_factor(0.5, EaseCurve::Quad), 0.25);
        assert_eq!(ease_factor(0.5, EaseCurve::Cubic), 0.125);
        assert_eq!(ease_factor(-1.0, EaseCurve::Linear), 0.0);
        assert_eq!(ease_factor(2.0, EaseCurve::Linear), 1.0);
    }

    #[test]
    fn ramp_scales_delta_over_time() {
        let mut ramp = RampController::new(100, 350, RampApply::Delta, EaseCurve::Linear);
        ramp.arm(10.0);

        assert_eq!(ramp.apply(100, -50, 2.5, 10.0), (0, 0, 2.5));
        assert_eq!(ramp.apply(100, -50, 2.5, 10.05), (50, -25, 2.5));
        assert_eq!(ramp.apply(100, -50, 2.5, 10.10), (100, -50, 2.5));
    }

    #[test]
    fn ramp_can_fade_smoothing() {
        let mut ramp = RampController::new(100, 350, RampApply::Smoothing, EaseCurve::Linear);
        ramp.arm(0.0);

        assert_eq!(ramp.apply(10, 20, 2.5, 0.0), (10, 20, 10.0));
        assert_eq!(ramp.apply(10, 20, 2.5, 0.10), (10, 20, 2.5));
    }

    #[test]
    fn ramp_idle_reset_disarms_after_configured_gap() {
        let mut ramp = RampController::new(100, 30, RampApply::Delta, EaseCurve::Linear);
        ramp.arm(1.0);
        ramp.note_target_seen(1.01);
        ramp.check_idle_reset(1.039);
        assert!(ramp.is_armed());

        ramp.check_idle_reset(1.040);
        assert!(!ramp.is_armed());
    }

    #[test]
    fn decay_eases_last_delta_to_zero() {
        let mut decay = DecayController::new(100, EaseCurve::Linear);
        decay.arm(100, -50, 2.0);

        assert_eq!(decay.delta(2.0), (100, -50, true));
        assert_eq!(decay.delta(2.05), (50, -25, true));
        assert_eq!(decay.delta(2.10), (0, 0, false));
        assert!(!decay.is_active());
    }

    #[test]
    fn proximity_charge_builds_and_applies_strength() {
        let config = ProximityCharge {
            enabled: true,
            distance: 50,
            build_ms: 100,
            effect: ProximityEffect::Strength,
            ease: EaseCurve::Linear,
        };
        let mut state = ProximityChargeState::default();

        state.update(config, true, 10, 20, 5.0);
        assert_eq!(state.factor, 0.0);

        state.update(config, true, 10, 20, 5.1);
        assert!((state.factor - 1.0).abs() < f64::EPSILON * 32.0);
        assert_eq!(state.apply(10, 20, 2.5, config.effect), (14, 27, 2.5));
    }

    #[test]
    fn proximity_charge_resets_out_of_range() {
        let config = ProximityCharge {
            enabled: true,
            distance: 50,
            build_ms: 100,
            effect: ProximityEffect::Smoothing,
            ease: EaseCurve::Linear,
        };
        let mut state = ProximityChargeState::default();

        state.update(config, true, 10, 20, 1.0);
        state.update(config, true, 60, 20, 1.1);

        assert_eq!(state.factor, 0.0);
        assert_eq!(state.started_at_s, 0.0);
    }
}
