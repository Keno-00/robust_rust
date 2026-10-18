use crate::ml_aim::{basis, fit, position, Dataset, Model, ALPHA, BASIS, BETA, PHASE_DECAY};
use serde::{Deserialize, Serialize};

pub const SCHEMA: u32 = crate::ml_aim::SCHEMA;
const FEATURES: usize = 16;
const HISTORY: usize = 4;
const DT: f64 = 0.02;
const SEQUENCE: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flick {
    pub direction: [f64; 2],
    pub distance: f64,
    pub duration: f64,
    pub reaction: f64,
    pub initial_velocity: [f64; 2],
    pub weights: [Vec<f64>; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub context: [f64; FEATURES],
    pub residual: [[f64; 2]; SEQUENCE],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    pub source: String,
    pub flicks: Vec<Flick>,
    pub weights: [Vec<f64>; 2],
    pub scales: [f64; FEATURES],
    pub sequences: Vec<Sequence>,
}

fn features(
    error: [f64; 2],
    derivative: [f64; 2],
    history: &[[f64; 2]; HISTORY],
) -> [f64; FEATURES] {
    let mut x = [0.0; FEATURES];
    x[..2].copy_from_slice(&error);
    x[2..4].copy_from_slice(&derivative);
    for (i, v) in history.iter().enumerate() {
        x[4 + 2 * i..6 + 2 * i].copy_from_slice(v);
    }
    // Smooth directional conditioning, including cross-axis dependencies.
    x[12] = error[0] * error[0].abs();
    x[13] = error[1] * error[1].abs();
    x[14] = error[0] * error[1];
    x[15] = 1.0;
    x
}

fn predict(p: &Personality, x: &[f64; FEATURES]) -> [f64; 2] {
    std::array::from_fn(|a| {
        p.weights[a]
            .iter()
            .zip(x)
            .zip(p.scales)
            .map(|((w, v), s)| w * v / s)
            .sum()
    })
}

pub fn train(data: &[Dataset], id: String, created_at: u64) -> Result<Model, String> {
    let source = data[0].source.clone();
    if source != "color" && source != "memory" {
        return Err("Personality dataset needs an explicit color or memory source.".into());
    }
    if data
        .iter()
        .any(|d| d.schema != SCHEMA || d.source != source)
    {
        return Err("Train one source at a time. Retired recordings cannot train a personality model; record the new course.".into());
    }
    let mut flicks = Vec::new();
    let mut runs = Vec::new();
    for d in data {
        for trial in &d.trials {
            let s = &trial.samples;
            if !trial.tracking {
                let end = trial.acquisition_end.ok_or(
                    "Acquisition recording is missing its click boundary. Record the new course.",
                )?;
                let count = s.partition_point(|v| v.t <= end);
                let s = &s[..count];
                if s.len() < 3 {
                    return Err("Acquisition has insufficient samples before its click.".into());
                }
                let first = s
                    .iter()
                    .position(|v| {
                        (v.cursor[0] - s[0].cursor[0]).hypot(v.cursor[1] - s[0].cursor[1]) > 0.5
                    })
                    .ok_or("Flick contains no movement. Record the course again.")?
                    .saturating_sub(1);
                let start = s[first].t;
                let duration = s.last().unwrap().t - start;
                let offset = [
                    s.last().unwrap().target[0] - s[first].cursor[0],
                    s.last().unwrap().target[1] - s[first].cursor[1],
                ];
                let distance = offset[0].hypot(offset[1]);
                if duration < 0.03 || distance <= trial.radius {
                    return Err(
                        "Acquisition has too little movement to fit. Record the course again."
                            .into(),
                    );
                }
                let direction = [offset[0] / distance, offset[1] / distance];
                let project = |t| {
                    let p = position(s, t, false);
                    let delta = [
                        (p[0] - s[first].cursor[0]) / distance,
                        (p[1] - s[first].cursor[1]) / distance,
                    ];
                    [
                        delta[0] * direction[0] + delta[1] * direction[1],
                        -delta[0] * direction[1] + delta[1] * direction[0],
                    ]
                };
                let mut rows: [Vec<([f64; BASIS], f64)>; 2] = [Vec::new(), Vec::new()];
                for i in 1..100 {
                    let u = i as f64 / 100.0;
                    let a = project(start + (u - 0.01) * duration);
                    let b = project(start + u * duration);
                    let c = project(start + (u + 0.01) * duration);
                    for axis in 0..2 {
                        let v = (c[axis] - a[axis]) / 0.02;
                        let acc = (c[axis] - 2.0 * b[axis] + a[axis]) / 0.0001;
                        rows[axis].push((
                            basis((-PHASE_DECAY * u).exp()),
                            acc - ALPHA
                                * (BETA * ((if axis == 0 { 1.0 } else { 0.0 }) - b[axis]) - v),
                        ));
                    }
                }
                let initial_velocity = if first > 0 {
                    let dt = s[first].t - s[first - 1].t;
                    std::array::from_fn(|a| {
                        (s[first].cursor[a] - s[first - 1].cursor[a]) / (dt * d.width)
                    })
                } else {
                    [0.0; 2]
                };
                flicks.push(Flick {
                    direction,
                    distance: distance / d.width,
                    duration,
                    reaction: start - s[0].t,
                    initial_velocity,
                    weights: [fit(&rows[0])?.to_vec(), fit(&rows[1])?.to_vec()],
                });
            }
            {
                if trial.tracking
                    && s.last().unwrap().t - s[0].t < crate::ml_aim::TRACK_SECONDS - 0.1
                {
                    return Err("Tracking course is incomplete. Record the course again.".into());
                }
                let mut run = Vec::new();
                let mut history = [[0.0; 2]; HISTORY];
                let mut last_error = [0.0; 2];
                let mut t = s[0].t;
                while t + DT <= s.last().unwrap().t {
                    let i = s.partition_point(|v| v.t < t).min(s.len() - 1);
                    let j = s.partition_point(|v| v.t < t + DT).min(s.len() - 1);
                    if !s[i].visible || !s[j].visible || s[i].target_id != s[j].target_id {
                        if !run.is_empty() {
                            runs.push(std::mem::take(&mut run));
                        }
                        history = [[0.0; 2]; HISTORY];
                        last_error = [0.0; 2];
                        t += DT;
                        continue;
                    }
                    let cursor = position(s, t, false);
                    let target = position(s, t, true);
                    let next = position(s, t + DT, false);
                    let error = std::array::from_fn(|a| (target[a] - cursor[a]) / d.width);
                    let derivative = if run.is_empty() {
                        [0.0; 2]
                    } else {
                        std::array::from_fn(|a| (error[a] - last_error[a]) / DT)
                    };
                    let velocity = std::array::from_fn(|a| (next[a] - cursor[a]) / (DT * d.width));
                    run.push((features(error, derivative, &history), velocity));
                    history.rotate_right(1);
                    history[0] = velocity;
                    last_error = error;
                    t += DT;
                }
                runs.push(run);
            }
        }
    }
    let mut scales = [0.0; FEATURES];
    let n: usize = runs.iter().map(Vec::len).sum();
    if n < FEATURES * 2 {
        return Err("Insufficient usable tracking history. Record the course again.".into());
    }
    for run in &runs {
        for (x, _) in run {
            for i in 0..FEATURES {
                scales[i] += x[i] * x[i];
            }
        }
    }
    for s in &mut scales {
        *s = (*s / n as f64).sqrt().max(1e-4);
    }
    let rows: [Vec<([f64; FEATURES], f64)>; 2] = std::array::from_fn(|a| {
        runs.iter()
            .flatten()
            .map(|(x, y)| (std::array::from_fn(|i| x[i] / scales[i]), y[a]))
            .collect()
    });
    let mut p = Personality {
        source,
        flicks,
        weights: [fit(&rows[0])?.to_vec(), fit(&rows[1])?.to_vec()],
        scales,
        sequences: Vec::new(),
    };
    let mut squared_error = 0.0;
    for run in runs {
        for (x, y) in &run {
            let v = predict(&p, x);
            squared_error += (v[0] - y[0]).powi(2) + (v[1] - y[1]).powi(2);
        }
        for start in (0..run.len().saturating_sub(SEQUENCE)).step_by(SEQUENCE) {
            let residual = std::array::from_fn(|j| {
                let (x, y) = &run[start + j];
                let v = predict(&p, x);
                [y[0] - v[0], y[1] - v[1]]
            });
            p.sequences.push(Sequence {
                context: run[start].0,
                residual,
            });
        }
    }
    let model = Model {
        schema: SCHEMA,
        product: crate::ml_aim::PRODUCT.into(),
        name: format!(
            "Personality {} ({})",
            id.split('-').next().unwrap(),
            p.source
        ),
        id,
        created_at,
        datasets: data.iter().map(|d| d.id.clone()).collect(),
        flick_count: p.flicks.len(),
        tracking_count: data
            .iter()
            .map(|d| d.trials.iter().filter(|t| t.tracking).count())
            .sum(),
        tracking_rmse: (squared_error / (2 * n) as f64).sqrt(),
        personality: p,
    };
    model.validate()?;
    Ok(model)
}

impl Personality {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.source.as_str(), "color" | "memory")
            || self.flicks.is_empty()
            || self.sequences.is_empty()
            || self.flicks.len() > 10000
            || self.sequences.len() > 50000
            || self
                .weights
                .iter()
                .any(|w| w.len() != FEATURES || w.iter().any(|v| !v.is_finite()))
            || self.scales.iter().any(|s| !s.is_finite() || *s <= 0.0)
            || self.sequences.iter().any(|s| {
                s.context
                    .iter()
                    .chain(s.residual.iter().flatten())
                    .any(|v| !v.is_finite())
            })
            || self.flicks.iter().any(|f| {
                f.direction
                    .iter()
                    .chain(&f.initial_velocity)
                    .any(|v| !v.is_finite())
                    || !f.distance.is_finite()
                    || f.distance <= 0.0
                    || !f.duration.is_finite()
                    || f.duration < 0.03
                    || !f.reaction.is_finite()
                    || f.reaction < 0.0
                    || f.weights
                        .iter()
                        .any(|w| w.len() != BASIS || w.iter().any(|v| !v.is_finite()))
            })
        {
            return Err("Invalid personality data. Record and train a new model.".into());
        }
        Ok(())
    }
}
