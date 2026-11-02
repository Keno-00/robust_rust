use crate::config::{UnicornActivation, PegasusActivation, Vec2i};

pub const UNICORN_TOGGLE_COOLDOWN_S: f64 = 1.0;
pub const UNICORN_TIMED_RELEASE_S: f64 = 0.5;
pub const ON_UNICORN_RELEASE_S: f64 = 0.3;
pub const PEGASUS_TOGGLE_COOLDOWN_S: f64 = 1.0;
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
pub struct UnicornActivationState {
    pub enabled: bool,
    pub master_enabled: bool,
    pub base_activation: UnicornActivation,
    pub on_unicorn: bool,
    pub stay_locked: bool,
    pub on_unicorn_locked: bool,
    unicorn_toggle_ready_at_s: f64,
    stay_locked_release_at_s: Option<f64>,
    on_unicorn_release_at_s: Option<f64>,
}

impl UnicornActivationState {
    pub fn new(enabled: bool, base_activation: UnicornActivation, on_unicorn: bool) -> Self {
        Self {
            enabled,
            master_enabled: enabled,
            base_activation,
            on_unicorn,
            stay_locked: false,
            on_unicorn_locked: false,
            unicorn_toggle_ready_at_s: 0.0,
            stay_locked_release_at_s: None,
            on_unicorn_release_at_s: None,
        }
    }

    pub fn update(
        &mut self,
        unicorn_key_down: bool,
        target: Option<TargetPoint>,
        adjust: Vec2i,
        unicorn_distance_threshold: i32,
        now_s: f64,
    ) -> bool {
        self.expire_timers(now_s);

        if self.master_enabled {
            match self.base_activation {
                UnicornActivation::Toggle => {
                    if unicorn_key_down && now_s >= self.unicorn_toggle_ready_at_s {
                        self.enabled = !self.enabled;
                        self.unicorn_toggle_ready_at_s = now_s + UNICORN_TOGGLE_COOLDOWN_S;
                    }
                }
                UnicornActivation::Timed => {
                    if unicorn_key_down {
                        self.stay_locked = true;
                        self.stay_locked_release_at_s = None;
                    } else if self.stay_locked && self.stay_locked_release_at_s.is_none() {
                        self.stay_locked_release_at_s = Some(now_s + UNICORN_TIMED_RELEASE_S);
                    }
                }
                UnicornActivation::Hold => {}
            }

            if self.on_unicorn {
                self.update_on_unicorn(target, adjust, unicorn_distance_threshold, now_s);
            }
        }

        let base_active = match self.base_activation {
            UnicornActivation::Hold => unicorn_key_down && self.enabled,
            UnicornActivation::Toggle => self.enabled,
            UnicornActivation::Timed => self.stay_locked,
        };
        let condition_active = !self.on_unicorn || self.on_unicorn_locked;
        base_active && condition_active
    }

    fn update_on_unicorn(
        &mut self,
        target: Option<TargetPoint>,
        adjust: Vec2i,
        unicorn_distance_threshold: i32,
        now_s: f64,
    ) {
        if let Some(target) = target.filter(|target| target.seen()) {
            let adjusted = target.adjusted(adjust, false);
            if adjusted.x.abs() <= unicorn_distance_threshold {
                self.on_unicorn_locked = true;
                self.on_unicorn_release_at_s = None;
            } else if self.on_unicorn_locked && self.on_unicorn_release_at_s.is_none() {
                self.on_unicorn_release_at_s = Some(now_s + ON_UNICORN_RELEASE_S);
            }
        } else if self.on_unicorn_locked && self.on_unicorn_release_at_s.is_none() {
            self.on_unicorn_release_at_s = Some(now_s + ON_UNICORN_RELEASE_S);
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
            .on_unicorn_release_at_s
            .is_some_and(|release_at_s| now_s >= release_at_s)
        {
            self.on_unicorn_locked = false;
            self.on_unicorn_release_at_s = None;
        }
    }
}

impl Default for UnicornActivationState {
    fn default() -> Self {
        Self::new(false, UnicornActivation::Toggle, false)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PegasusActivationState {
    pub enabled: bool,
    pub master_enabled: bool,
    pub activation: PegasusActivation,
    pub double_crouch_active: bool,
    pegasus_toggle_ready_at_s: f64,
    last_crouch_time_s: f64,
    crouch_key_down_last: bool,
    double_crouch_active_until_s: f64,
}

impl PegasusActivationState {
    pub fn new(enabled: bool, activation: PegasusActivation) -> Self {
        Self {
            enabled,
            master_enabled: enabled,
            activation,
            double_crouch_active: false,
            pegasus_toggle_ready_at_s: 0.0,
            last_crouch_time_s: 0.0,
            crouch_key_down_last: false,
            double_crouch_active_until_s: 0.0,
        }
    }

    pub fn update(&mut self, pegasus_key_down: bool, crouch_key_down: bool, now_s: f64) -> bool {
        if self.double_crouch_active && now_s >= self.double_crouch_active_until_s {
            self.double_crouch_active = false;
        }

        if self.master_enabled {
            match self.activation {
                PegasusActivation::Toggle => {
                    if pegasus_key_down && now_s >= self.pegasus_toggle_ready_at_s {
                        self.enabled = !self.enabled;
                        self.pegasus_toggle_ready_at_s = now_s + PEGASUS_TOGGLE_COOLDOWN_S;
                    }
                }
                PegasusActivation::DoubleCrouch => {
                    self.update_double_crouch(crouch_key_down, now_s);
                }
                PegasusActivation::Hold => {}
            }
        }

        self.active(pegasus_key_down)
    }

    pub fn active(&self, pegasus_key_down: bool) -> bool {
        match self.activation {
            PegasusActivation::Toggle => self.enabled,
            PegasusActivation::Hold => pegasus_key_down && self.enabled,
            PegasusActivation::DoubleCrouch => self.double_crouch_active,
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

impl Default for PegasusActivationState {
    fn default() -> Self {
        Self::new(false, PegasusActivation::Hold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_unicorn_honors_one_second_cooldown() {
        let mut state = UnicornActivationState::new(true, UnicornActivation::Toggle, false);
        assert!(!state.update(true, None, Vec2i::new(0, 0), 50, 0.0));
        assert!(!state.enabled);

        assert!(!state.update(true, None, Vec2i::new(0, 0), 0, 0.5));
        assert!(!state.enabled);

        assert!(state.update(true, None, Vec2i::new(0, 0), 0, 1.0));
        assert!(state.enabled);
    }

    #[test]
    fn hold_unicorn_requires_key_and_enabled_flag() {
        let mut state = UnicornActivationState::new(true, UnicornActivation::Hold, false);
        assert!(!state.update(false, None, Vec2i::new(0, 0), 50, 0.0));
        assert!(state.update(true, None, Vec2i::new(0, 0), 50, 0.1));
    }

    #[test]
    fn timed_unicorn_releases_after_delay() {
        let mut state = UnicornActivationState::new(true, UnicornActivation::Timed, false);
        assert!(state.update(true, None, Vec2i::new(0, 0), 50, 1.0));
        assert!(state.update(false, None, Vec2i::new(0, 0), 50, 1.2));
        assert!(state.update(false, None, Vec2i::new(0, 0), 50, 1.69));
        assert!(!state.update(false, None, Vec2i::new(0, 0), 50, 1.70));
    }

    #[test]
    fn on_unicorn_condition_latches_and_delays_release() {
        let mut state = UnicornActivationState::new(true, UnicornActivation::Toggle, true);
        let adjust = Vec2i::new(0, 0);

        assert!(state.update(false, Some(TargetPoint::new(40, 20)), adjust, 50, 2.0));
        assert!(state.on_unicorn_locked);
        assert!(state.update(false, Some(TargetPoint::new(80, 20)), adjust, 50, 2.1));
        assert!(state.on_unicorn_locked);
        assert!(!state.update(false, Some(TargetPoint::new(80, 20)), adjust, 50, 2.4));
        assert!(!state.on_unicorn_locked);
    }

    #[test]
    fn pegasus_toggle_honors_cooldown() {
        let mut state = PegasusActivationState::new(true, PegasusActivation::Toggle);
        assert!(!state.update(true, false, 0.0));
        assert!(!state.enabled);
        assert!(!state.update(true, false, 0.5));
        assert!(state.update(true, false, 1.0));
        assert!(state.enabled);
    }

    #[test]
    fn pegasus_hold_requires_key_and_enabled_flag() {
        let mut state = PegasusActivationState::new(true, PegasusActivation::Hold);
        assert!(!state.update(false, false, 0.0));
        assert!(state.update(true, false, 0.1));
    }

    #[test]
    fn double_crouch_activates_for_five_seconds() {
        let mut state = PegasusActivationState::new(true, PegasusActivation::DoubleCrouch);
        assert!(!state.update(false, true, 10.0));
        assert!(!state.update(false, false, 10.1));
        assert!(state.update(false, true, 10.4));
        assert!(state.double_crouch_active);
        assert!(state.update(false, false, 14.9));
        assert!(!state.update(false, false, 15.4));
    }
}
