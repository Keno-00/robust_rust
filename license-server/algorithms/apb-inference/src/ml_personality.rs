//! Version 2: directional flick repertoire and context-conditioned tracking sequences.
//! All variation is measured demonstration residual, never synthetic noise.
use crate::ml_aim::BASIS;
use serde::{Deserialize, Serialize};

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

impl Personality {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::ml_aim::source_allowed(&self.source)
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
    pub fn flick(
        &self,
        error: [f64; 2],
        width: f64,
        velocity: [f64; 2],
        acquisition: usize,
    ) -> &Flick {
        let distance = error[0].hypot(error[1]);
        let score = |f: &Flick| {
            let alignment = (error[0] * f.direction[0] + error[1] * f.direction[1]) / distance;
            4.0 * (1.0 - alignment)
                + (distance / width / f.distance).ln().powi(2)
                + (velocity[0] / width - f.initial_velocity[0]).powi(2)
                + (velocity[1] / width - f.initial_velocity[1]).powi(2)
        };
        let mut candidates: Vec<_> = self.flicks.iter().map(|f| (score(f), f)).collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let close = candidates
            .iter()
            .take(3)
            .take_while(|(s, _)| *s <= candidates[0].0 + 0.25)
            .count();
        candidates[acquisition % close].1
    }
}

#[derive(Debug, Default)]
pub struct Tracking {
    history: [[f64; 2]; HISTORY],
    previous_error: Option<[f64; 2]>,
    sequence: Option<usize>,
    elapsed: f64,
    history_elapsed: f64,
}
impl Tracking {
    pub fn observe(&mut self, velocity: [f64; 2], width: f64, dt: f64) {
        self.history_elapsed += dt;
        while self.history_elapsed >= DT {
            self.history.rotate_right(1);
            self.history[0] = velocity.map(|v| v / width);
            self.history_elapsed -= DT;
        }
    }

    pub fn step(&mut self, p: &Personality, error: [f64; 2], width: f64, dt: f64) -> [f64; 2] {
        let e = error.map(|v| v / width);
        let old = self.previous_error.unwrap_or(e);
        let derivative = std::array::from_fn(|a| (e[a] - old[a]) / dt);
        let steps = (dt / DT).ceil().max(1.0) as usize;
        let h = dt / steps as f64;
        let mut result = [0.0; 2];
        for step in 0..steps {
            let current = std::array::from_fn(|a| {
                old[a] + (e[a] - old[a]) * (step + 1) as f64 / steps as f64
            });
            let x = features(current, derivative, &self.history);
            let reversed = step == 0 && (0..2).any(|a| old[a] * e[a] < 0.0);
            if self.sequence.is_none() || self.elapsed >= DT * SEQUENCE as f64 || reversed {
                self.sequence = p
                    .sequences
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let score = |s: &Sequence| {
                            (0..FEATURES - 1)
                                .map(|i| ((x[i] - s.context[i]) / p.scales[i]).powi(2))
                                .sum::<f64>()
                        };
                        score(a).total_cmp(&score(b))
                    })
                    .map(|(i, _)| i);
                self.elapsed = 0.0;
            }
            let seq = &p.sequences[self.sequence.expect("validated nonempty sequences")];
            let index = ((self.elapsed / DT).floor() as usize).min(SEQUENCE - 1);
            let fraction = (self.elapsed / DT - index as f64).clamp(0.0, 1.0);
            let base = predict(p, &x);
            let velocity = std::array::from_fn(|a| {
                base[a]
                    + seq.residual[index][a] * (1.0 - fraction)
                    + seq.residual[(index + 1).min(SEQUENCE - 1)][a] * fraction
            });
            for a in 0..2 {
                result[a] += velocity[a] * width * h;
            }
            self.elapsed += h;
            self.observe(velocity.map(|v| v * width), width, h);
        }
        self.previous_error = Some(e);
        result
    }
}
