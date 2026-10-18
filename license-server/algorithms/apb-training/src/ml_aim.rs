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
pub const TRACK_SECONDS: f64 = 15.0;
pub(crate) const BASIS: usize = 12;
pub(crate) const ALPHA: f64 = 25.0;
pub(crate) const BETA: f64 = ALPHA / 4.0;
pub(crate) const PHASE_DECAY: f64 = 4.0;
const RIDGE: f64 = 0.01;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub t: f64,
    pub cursor: [f64; 2],
    pub target: [f64; 2],
    pub raw_counts: [f64; 2],
    #[serde(default)]
    pub displayed_target: Option<[f64; 2]>,
    #[serde(default = "visible_default")]
    pub visible: bool,
    #[serde(default)]
    pub target_id: usize,
}

fn visible_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub tracking: bool,
    pub scenario: usize,
    pub radius: f64,
    pub misses: usize,
    #[serde(default)]
    pub acquisition_end: Option<f64>,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub schema: u32,
    pub product: String,
    pub id: String,
    pub width: f64,
    pub height: f64,
    pub source: String,
    pub trials: Vec<Trial>,
}

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

fn validate_dataset(data: &Dataset) -> Result<(), String> {
    valid_id(&data.id)?;
    if !source_allowed(&data.source)
        || data.schema != SCHEMA
        || data.product != PRODUCT
        || !data.width.is_finite()
        || data.width < 1.0
        || !data.height.is_finite()
        || data.height < 1.0
    {
        return Err("Incompatible ML recording. Record the new personality course; old recordings are retired.".into());
    }
    let flicks = data.trials.iter().filter(|t| !t.tracking).count();
    let tracks = data.trials.iter().filter(|t| t.tracking).count();
    if flicks != FLICK_TRIALS || tracks != TRACK_TRIALS {
        return Err("ML dataset is incomplete. Finish all scenarios before training.".into());
    }
    for trial in &data.trials {
        if trial.samples.len() < 3 || !trial.radius.is_finite() || trial.radius <= 0.0 {
            return Err("ML trial has insufficient samples or invalid target size.".into());
        }
        if trial
            .acquisition_end
            .is_some_and(|t| !t.is_finite() || t < 0.03 || t > trial.samples.last().unwrap().t)
        {
            return Err("Invalid acquisition boundary.".into());
        }
        let mut previous = -1.0;
        for sample in &trial.samples {
            if !sample.t.is_finite()
                || sample.t < 0.0
                || sample.t <= previous
                || !sample
                    .cursor
                    .iter()
                    .chain(&sample.target)
                    .chain(&sample.raw_counts)
                    .chain(sample.displayed_target.iter().flatten())
                    .all(|v| v.is_finite())
            {
                return Err("ML trial has invalid positions or non-monotonic timestamps.".into());
            }
            previous = sample.t;
        }
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

pub(crate) fn fit<const N: usize>(rows: &[([f64; N], f64)]) -> Result<[f64; N], String> {
    if rows.len() < N * 2 {
        return Err("Too few usable movement samples. Collect another dataset.".into());
    }
    let mut matrix = [[0.0; N]; N];
    let mut rhs = [0.0; N];
    for (x, y) in rows {
        for i in 0..N {
            rhs[i] += x[i] * y;
            for j in 0..N {
                matrix[i][j] += x[i] * x[j];
            }
        }
    }
    for (i, row) in matrix.iter_mut().enumerate() {
        row[i] += RIDGE;
    }
    for i in 0..N {
        let pivot = (i..N)
            .max_by(|a, b| matrix[*a][i].abs().total_cmp(&matrix[*b][i].abs()))
            .unwrap();
        matrix.swap(i, pivot);
        rhs.swap(i, pivot);
        let value = matrix[i][i];
        if !value.is_finite() || value.abs() < 1e-12 {
            return Err("ML fit is singular. Collect more varied movement scenarios.".into());
        }
        for j in i..N {
            matrix[i][j] /= value;
        }
        rhs[i] /= value;
        for k in 0..N {
            if k == i {
                continue;
            }
            let factor = matrix[k][i];
            for j in i..N {
                matrix[k][j] -= factor * matrix[i][j];
            }
            rhs[k] -= factor * rhs[i];
        }
    }
    if rhs.iter().any(|v| !v.is_finite()) {
        return Err("ML fitting produced non-finite coefficients.".into());
    }
    Ok(rhs)
}

pub(crate) fn position(samples: &[Sample], time: f64, target: bool) -> [f64; 2] {
    let i = samples
        .partition_point(|s| s.t < time)
        .clamp(1, samples.len() - 1);
    let a = &samples[i - 1];
    let b = &samples[i];
    let u = ((time - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
    let pa = if target { a.target } else { a.cursor };
    let pb = if target { b.target } else { b.cursor };
    [pa[0] + u * (pb[0] - pa[0]), pa[1] + u * (pb[1] - pa[1])]
}

pub fn validate_inputs(data: &[Dataset]) -> Result<(), String> {
    if data.is_empty() || data.len() > 32 {
        return Err("Select 1–32 complete datasets.".into());
    }
    let count: usize = data
        .iter()
        .flat_map(|d| &d.trials)
        .map(|t| t.samples.len())
        .sum();
    if count > 1_000_000 {
        return Err("Training exceeds the sample limit.".into());
    }
    for d in data {
        validate_dataset(d)?;
    }
    Ok(())
}
