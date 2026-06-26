use crate::config::{AimActivation, TriggerActivation, Vec2i};

pub const AIM_TOGGLE_COOLDOWN_S: f64 = 1.0;
pub const AIM_TIMED_RELEASE_S: f64 = 0.5;
pub const ON_AIM_RELEASE_S: f64 = 0.3;
pub const TRIGGER_TOGGLE_COOLDOWN_S: f64 = 1.0;
pub const DOUBLE_CROUCH_WINDOW_S: f64 = 0.5;
pub const DOUBLE_CROUCH_ACTIVE_S: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetPoint {
    pub x: i32,
    pub y: i32,
}

impl TargetPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn seen(self) -> bool {
        self.x != 0 || self.y != 0
    }

    pub fn adjusted(self, adjust: Vec2i, x_only: bool) -> Self {
        Self {
            x: self.x + adjust.x,
            y: if x_only { 0 } else { self.y + adjust.y },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AimActivationState {
    pub enabled: bool,
    pub master_enabled: bool,
    pub base_activation: AimActivation,
    pub on_aim: bool,
    pub stay_locked: bool,
    pub on_aim_locked: bool,
    aim_toggle_ready_at_s: f64,
    stay_locked_release_at_s: Option<f64>,
    on_aim_release_at_s: Option<f64>,
}

impl AimActivationState {
    pub fn new(enabled: bool, base_activation: AimActivation, on_aim: bool) -> Self {
        Self {
            enabled,
            master_enabled: enabled,
            base_activation,
            on_aim,
            stay_locked: false,
            on_aim_locked: false,
            aim_toggle_ready_at_s: 0.0,
            stay_locked_release_at_s: None,
            on_aim_release_at_s: None,
        }
    }

    pub fn update(
        &mut self,
        aim_key_down: bool,
        target: Option<TargetPoint>,
        adjust: Vec2i,
        aim_distance_threshold: i32,
        now_s: f64,
    ) -> bool {
        self.expire_timers(now_s);

        if self.master_enabled {
            match self.base_activation {
                AimActivation::Toggle => {
                    if aim_key_down && now_s >= self.aim_toggle_ready_at_s {
                        self.enabled = !self.enabled;
                        self.aim_toggle_ready_at_s = now_s + AIM_TOGGLE_COOLDOWN_S;
                    }
                }
                AimActivation::Timed => {
                    if aim_key_down {
                        self.stay_locked = true;
                        self.stay_locked_release_at_s = None;
                    } else if self.stay_locked && self.stay_locked_release_at_s.is_none() {
                        self.stay_locked_release_at_s = Some(now_s + AIM_TIMED_RELEASE_S);
                    }
                }
                AimActivation::Hold => {}
            }

            if self.on_aim {
                self.update_on_aim(target, adjust, aim_distance_threshold, now_s);
            }
        }

        let base_active = match self.base_activation {
            AimActivation::Hold => aim_key_down && self.enabled,
            AimActivation::Toggle => self.enabled,
            AimActivation::Timed => self.stay_locked,
        };
        let condition_active = !self.on_aim || self.on_aim_locked;
        base_active && condition_active
    }

    fn update_on_aim(
        &mut self,
        target: Option<TargetPoint>,
        adjust: Vec2i,
        aim_distance_threshold: i32,
        now_s: f64,
    ) {
        if let Some(target) = target.filter(|target| target.seen()) {
            let adjusted = target.adjusted(adjust, false);
            if adjusted.x.abs() <= aim_distance_threshold {
                self.on_aim_locked = true;
                self.on_aim_release_at_s = None;
            } else if self.on_aim_locked && self.on_aim_release_at_s.is_none() {
                self.on_aim_release_at_s = Some(now_s + ON_AIM_RELEASE_S);
            }
        } else if self.on_aim_locked && self.on_aim_release_at_s.is_none() {
            self.on_aim_release_at_s = Some(now_s + ON_AIM_RELEASE_S);
        }
    }

    fn expire_timers(&mut self, now_s: f64) {
        if self
            .stay_locked_release_at_s
            .is_some_and(|release_at_s| now_s >= release_at_s)
        {
            self.stay_locked = false;
            self.stay_locked_release_at_s = None;
        }
        if self
            .on_aim_release_at_s
            .is_some_and(|release_at_s| now_s >= release_at_s)
        {
            self.on_aim_locked = false;
            self.on_aim_release_at_s = None;
        }
    }
}

impl Default for AimActivationState {
    fn default() -> Self {
        Self::new(false, AimActivation::Toggle, false)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TriggerActivationState {
    pub enabled: bool,
    pub master_enabled: bool,
    pub activation: TriggerActivation,
    pub double_crouch_active: bool,
    trigger_toggle_ready_at_s: f64,
    last_crouch_time_s: f64,
    crouch_key_down_last: bool,
    double_crouch_active_until_s: f64,
}

impl TriggerActivationState {
    pub fn new(enabled: bool, activation: TriggerActivation) -> Self {
        Self {
            enabled,
            master_enabled: enabled,
            activation,
            double_crouch_active: false,
            trigger_toggle_ready_at_s: 0.0,
            last_crouch_time_s: 0.0,
            crouch_key_down_last: false,
            double_crouch_active_until_s: 0.0,
        }
    }

    pub fn update(&mut self, trigger_key_down: bool, crouch_key_down: bool, now_s: f64) -> bool {
        if self.double_crouch_active && now_s >= self.double_crouch_active_until_s {
            self.double_crouch_active = false;
        }

        if self.master_enabled {
            match self.activation {
                TriggerActivation::Toggle => {
                    if trigger_key_down && now_s >= self.trigger_toggle_ready_at_s {
                        self.enabled = !self.enabled;
                        self.trigger_toggle_ready_at_s = now_s + TRIGGER_TOGGLE_COOLDOWN_S;
                    }
                }
                TriggerActivation::DoubleCrouch => {
                    self.update_double_crouch(crouch_key_down, now_s);
                }
                TriggerActivation::Hold => {}
            }
        }

        self.active(trigger_key_down)
    }

    pub fn active(&self, trigger_key_down: bool) -> bool {
        match self.activation {
            TriggerActivation::Toggle => self.enabled,
            TriggerActivation::Hold => trigger_key_down && self.enabled,
            TriggerActivation::DoubleCrouch => self.double_crouch_active,
        }
    }

    fn update_double_crouch(&mut self, crouch_key_down: bool, now_s: f64) {
        if crouch_key_down {
            if !self.crouch_key_down_last {
                self.crouch_key_down_last = true;
                if now_s - self.last_crouch_time_s <= DOUBLE_CROUCH_WINDOW_S {
                    self.double_crouch_active = true;
                    self.double_crouch_active_until_s = now_s + DOUBLE_CROUCH_ACTIVE_S;
                }
                self.last_crouch_time_s = now_s;
            }
        } else if self.crouch_key_down_last {
            self.crouch_key_down_last = false;
        }
    }
}

impl Default for TriggerActivationState {
    fn default() -> Self {
        Self::new(false, TriggerActivation::Hold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_aim_honors_one_second_cooldown() {
        let mut state = AimActivationState::new(true, AimActivation::Toggle, false);
        assert!(!state.update(true, None, Vec2i::new(0, 0), 50, 0.0));
        assert!(!state.enabled);

        assert!(!state.update(true, None, Vec2i::new(0, 0), 0, 0.5));
        assert!(!state.enabled);

        assert!(state.update(true, None, Vec2i::new(0, 0), 0, 1.0));
        assert!(state.enabled);
    }

    #[test]
    fn hold_aim_requires_key_and_enabled_flag() {
        let mut state = AimActivationState::new(true, AimActivation::Hold, false);
        assert!(!state.update(false, None, Vec2i::new(0, 0), 50, 0.0));
        assert!(state.update(true, None, Vec2i::new(0, 0), 50, 0.1));
    }

    #[test]
    fn timed_aim_releases_after_delay() {
        let mut state = AimActivationState::new(true, AimActivation::Timed, false);
        assert!(state.update(true, None, Vec2i::new(0, 0), 50, 1.0));
        assert!(state.update(false, None, Vec2i::new(0, 0), 50, 1.2));
        assert!(state.update(false, None, Vec2i::new(0, 0), 50, 1.69));
        assert!(!state.update(false, None, Vec2i::new(0, 0), 50, 1.70));
    }

    #[test]
    fn on_aim_condition_latches_and_delays_release() {
        let mut state = AimActivationState::new(true, AimActivation::Toggle, true);
        let adjust = Vec2i::new(0, 0);

        assert!(state.update(false, Some(TargetPoint::new(40, 20)), adjust, 50, 2.0));
        assert!(state.on_aim_locked);
        assert!(state.update(false, Some(TargetPoint::new(80, 20)), adjust, 50, 2.1));
        assert!(state.on_aim_locked);
        assert!(!state.update(false, Some(TargetPoint::new(80, 20)), adjust, 50, 2.4));
        assert!(!state.on_aim_locked);
    }

    #[test]
    fn trigger_toggle_honors_cooldown() {
        let mut state = TriggerActivationState::new(true, TriggerActivation::Toggle);
        assert!(!state.update(true, false, 0.0));
        assert!(!state.enabled);
        assert!(!state.update(true, false, 0.5));
        assert!(state.update(true, false, 1.0));
        assert!(state.enabled);
    }

    #[test]
    fn trigger_hold_requires_key_and_enabled_flag() {
        let mut state = TriggerActivationState::new(true, TriggerActivation::Hold);
        assert!(!state.update(false, false, 0.0));
        assert!(state.update(true, false, 0.1));
    }

    #[test]
    fn double_crouch_activates_for_five_seconds() {
        let mut state = TriggerActivationState::new(true, TriggerActivation::DoubleCrouch);
        assert!(!state.update(false, true, 10.0));
        assert!(!state.update(false, false, 10.1));
        assert!(state.update(false, true, 10.4));
        assert!(state.double_crouch_active);
        assert!(state.update(false, false, 14.9));
        assert!(!state.update(false, false, 15.4));
    }
}
