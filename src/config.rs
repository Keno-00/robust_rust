#[derive(Debug, Clone)]
pub struct ProfileConfig {
    pub aim_distance: i32,
    pub ramp_time_ms: u32,
    pub ramp_idle_reset_ms: u32,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            aim_distance: 50,
            ramp_time_ms: 0,
            ramp_idle_reset_ms: 350,
        }
    }
}
