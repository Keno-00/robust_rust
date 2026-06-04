#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseActivation {
    Hold,
    Toggle,
    Timed,
    OnAim,
}

#[derive(Debug, Clone, Copy)]
pub struct ActivationState {
    pub enabled: bool,
    pub latched: bool,
}

impl Default for ActivationState {
    fn default() -> Self {
        Self {
            enabled: false,
            latched: false,
        }
    }
}
