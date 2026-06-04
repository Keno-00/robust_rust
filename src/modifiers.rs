#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeEffect {
    Strength,
    Smoothing,
}

#[derive(Debug, Clone, Copy)]
pub struct ProximityCharge {
    pub enabled: bool,
    pub distance: i32,
    pub build_ms: u32,
    pub effect: ChargeEffect,
}

impl Default for ProximityCharge {
    fn default() -> Self {
        Self {
            enabled: false,
            distance: 50,
            build_ms: 700,
            effect: ChargeEffect::Strength,
        }
    }
}
