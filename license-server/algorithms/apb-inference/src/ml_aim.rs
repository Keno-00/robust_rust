use serde::{Deserialize, Serialize};
pub const SCHEMA: u32 = 2;
#[cfg(not(feature = "color-only"))]
pub const PRODUCT: &str = "verux-apb";
#[cfg(feature = "color-only" )]
pub const PRODUCT: &str = "verux-apb-cb";
pub(crate) fn source_allowed(source: &str) -> bool {
    source == "color" || (!cfg!(feature = "color-only") && source == "memory")
}
pub const FLICK_TRIALS: usize = 60;
pub const TRACK_TRIALS: usize = 10;
pub(crate) const BASIS: usize = 12;
pub(crate) const ALPHA: f64 = 25.0;
pub(crate) const BETA: f64 = ALPHA / 4.0;
pub(crate) const PHASE_DECAY: f64 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub schema: u32,
    pub product: String,
    pub id: String,
    pub name: String,
    pub created_at: u64,
    pub datasets: Vec<String>,
    pub flick_count: usize,
    pub tracking_count: usize,
    pub tracking_rmse: f64,
    pub personality: crate::ml_personality::Personality,
}

fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 80 || !id.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
        return Err(
            "Invalid ML artifact ID. Collect a new dataset or select a valid model.".into(),
        );
    }
    Ok(())
}

impl Model {
    pub fn validate(&self) -> Result<(), String> {
        valid_id(&self.id)?;
        if self.name.trim().is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
            || self.name.contains("##")
        {
            return Err("Model names must contain 1–64 characters, without control characters or ##. Enter a valid name.".into());
        }
        if self.schema != SCHEMA
            || self.product != PRODUCT
            || self.flick_count < FLICK_TRIALS
            || self.tracking_count < TRACK_TRIALS
        {
            return Err("Incompatible ML model. Old models are retired; record the personality course and train again.".into());
        }
        self.personality.validate()?;
        if !self.tracking_rmse.is_finite() || self.tracking_rmse < 0.0 {
            return Err("Invalid tracking fit diagnostics.".into());
        }
        Ok(())
    }
}

pub(crate) fn basis(phase: f64) -> [f64; BASIS] {
    let mut values = [0.0; BASIS];
    let mut sum = 0.0;
    for (i, value) in values.iter_mut().enumerate() {
        let center = (-PHASE_DECAY * i as f64 / (BASIS - 1) as f64).exp();
        let width = 1.0 / (center * 0.5).powi(2);
        *value = (-width * (phase - center).powi(2)).exp();
        sum += *value;
    }
    if sum > 0.0 {
        for value in &mut values {
            *value *= phase / sum;
        }
    }
    values
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

#[derive(Debug)]
pub struct Controller {
    model: Model,
    width: f64,
    initial_dt: f64,
    previous_time: Option<f64>,
    previous_error: [f64; 2],
    velocity: [f64; 2],
    direction: [f64; 2],
    distance: f64,
    duration: f64,
    elapsed: f64,
    reaction: f64,
    flick_weights: [Vec<f64>; 2],
    acquisition: usize,
    tracking: crate::ml_personality::Tracking,
}

impl Controller {
    pub fn new(model: Model, width: i32, fps: u32) -> Result<Self, String> {
        model.validate()?;
        if width <= 0 || fps == 0 {
            return Err("ML Aim requires a valid monitor width and FPS.".into());
        }
        Ok(Self {
            model,
            width: f64::from(width),
            initial_dt: 1.0 / f64::from(fps),
            previous_time: None,
            previous_error: [0.0; 2],
            velocity: [0.0; 2],
            direction: [0.0; 2],
            distance: 0.0,
            duration: 1.0,
            elapsed: 0.0,
            reaction: 0.0,
            flick_weights: [Vec::new(), Vec::new()],
            acquisition: 0,
            tracking: Default::default(),
        })
    }

    pub fn reset(&mut self) {
        self.previous_time = None;
        self.velocity = [0.0; 2];
        self.tracking = Default::default();
    }

    /// Returns a learned displacement in the same screen-error units as the
    /// ordinary smoothers. The existing input pipeline interprets these as counts.
    pub fn step(&mut self, error: [f64; 2], now: f64) -> Result<[f64; 2], String> {
        if !now.is_finite() || error.iter().any(|v| !v.is_finite()) {
            return Err("ML Aim received invalid target coordinates or time.".into());
        }
        let fresh = self.previous_time.is_none();
        let dt = self
            .previous_time
            .map_or(self.initial_dt, |last| now - last);
        if dt <= 0.0 || dt > 0.25 {
            self.reset();
            return Err("ML Aim movement timing is invalid or stalled. Restart and check the capture/input timing.".into());
        }
        if fresh {
            self.distance = error[0].hypot(error[1]);
            if self.distance == 0.0 {
                return Ok([0.0; 2]);
            }
            self.direction = [error[0] / self.distance, error[1] / self.distance];
            let f =
                self.model
                    .personality
                    .flick(error, self.width, self.velocity, self.acquisition);
            self.duration = f.duration;
            self.reaction = f.reaction;
            self.flick_weights = f.weights.clone();
            self.acquisition += 1;
            self.elapsed = -self.reaction;
            self.previous_error = error;
        }
        let mut result = [0.0; 2];
        if self.elapsed < 0.0 {
            self.elapsed += dt;
        } else if self.elapsed < self.duration {
            // Integrate the fitted second-order system in small time steps.
            // This is numerical integration, not an extra movement modifier.
            let steps = (dt / (self.duration / 200.0)).ceil().max(1.0) as usize;
            let h = dt / steps as f64;
            let weights = &self.flick_weights;
            let mut predicted_error = error;
            for _ in 0..steps {
                let features = basis((-PHASE_DECAY * self.elapsed / self.duration).exp());
                let forcing = [dot(&weights[0], &features), dot(&weights[1], &features)];
                let global_forcing = [
                    forcing[0] * self.direction[0] - forcing[1] * self.direction[1],
                    forcing[0] * self.direction[1] + forcing[1] * self.direction[0],
                ];
                for axis in 0..2 {
                    let acceleration = (ALPHA
                        * (BETA * predicted_error[axis] - self.duration * self.velocity[axis])
                        + self.distance * global_forcing[axis])
                        / self.duration.powi(2);
                    self.velocity[axis] += acceleration * h;
                    let delta = self.velocity[axis] * h;
                    result[axis] += delta;
                    predicted_error[axis] -= delta;
                }
                self.elapsed += h;
            }
        } else {
            result = self
                .tracking
                .step(&self.model.personality, error, self.width, dt);
            self.velocity = result.map(|v| v / dt);
        }
        if self.elapsed < self.duration {
            self.tracking.observe(self.velocity, self.width, dt);
        }
        self.previous_time = Some(now);
        self.previous_error = error;
        if result
            .iter()
            .chain(&self.velocity)
            .any(|v| !v.is_finite() || v.abs() > i32::MAX as f64)
        {
            self.reset();
            return Err("ML Aim produced an invalid movement. Disable this model and collect a new dataset.".into());
        }
        Ok(result)
    }
}
