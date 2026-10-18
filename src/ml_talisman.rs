//! Version 2: directional flick repertoire and context-conditioned tracking sequences.
//! All variation is measured demonstration residual, never synthetic noise.
use crate::ml_unicorn::BASIS;
use serde::{Deserialize, Serialize};

pub const SCHEMA: u32 = crate::ml_unicorn::SCHEMA;
const FEATURES: usize = 16;
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

impl Personality {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::ml_unicorn::source_allowed(&self.source)
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
