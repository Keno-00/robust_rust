//! Vertical correction enters 25% inside live body bounds, then releases.
use crate::apb_orchid::PawnRef;
use crate::detection::Target;

#[derive(Debug, Clone, Copy)]
pub struct AtlasUnicornTarget {
    pub point: Target,
    pub pawn: PawnRef,
    pub top: f64,
    pub bottom: f64,
    pub body_bounds: [f64; 4],
    pub distance_units: Option<f64>,
}

#[derive(Default)]
pub struct VerticalCorrection {
    active: Option<PawnRef>,
}
impl VerticalCorrection {
    pub fn reset(&mut self) {
        self.active = None;
    }
    pub fn delta(&mut self, target: AtlasUnicornTarget) -> i32 {
        if self.active.is_some_and(|pawn| pawn != target.pawn) {
            self.reset();
        }
        if self.active.is_none() {
            if target.top > 0.0 {
                self.active = Some(target.pawn);
            } else if target.bottom < 0.0 {
                self.active = Some(target.pawn);
            } else {
                return 0;
            }
        }
        let upper = target.top + (target.bottom - target.top) * 0.25;
        let lower = target.bottom - (target.bottom - target.top) * 0.25;
        // Keep correcting into the inner band even after crossing the body edge.
        if upper <= 0.0 && lower >= 0.0 {
            self.reset();
            return 0;
        }
        if upper > 0.0 {
            upper.ceil() as i32
        } else if lower < 0.0 {
            lower.floor() as i32
        } else {
            self.reset();
            0
        }
    }
}

/// Approximate screen-height anchors; these are not skeletal bones or hitboxes.
pub fn approximate_delta(target: AtlasUnicornTarget, mode: crate::config::VerticalTracking, enabled: [bool; 4]) -> i32 {
    use crate::config::VerticalTracking;
    let height = target.bottom - target.top;
    let points = [target.top, target.top + height * 0.18, target.top + height * 0.45, target.bottom];
    let value = match mode {
        VerticalTracking::Head => points[0],
        VerticalTracking::Neck => points[1],
        VerticalTracking::Torso => points[2],
        VerticalTracking::Feet => points[3],
        VerticalTracking::ClosestPoint => points.into_iter().zip(enabled).filter(|(_, use_point)| *use_point).map(|(point, _)| point).min_by(|a,b| a.abs().total_cmp(&b.abs())).unwrap_or(0.0),
        _ => 0.0,
    };
    value.round() as i32
}
