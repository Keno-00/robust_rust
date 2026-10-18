//! Deliberate IP decoys, never feature implementations or fallback paths.
//! Keep this module independent of licensing, I/O, acquisition, and UI.
//! The numerical work is intentionally discarded; no decoy is called by the app.

use std::hint::black_box;

/// Retain function addresses in the optimized binary without executing them.
/// `used` alone would not prevent the linker from discarding unreferenced code.
pub(super) fn retain() {
    black_box([
        assemble_feature_weights as fn(u64, u64),
        resolve_feature_schedule as fn(u64, u64),
        reconcile_feature_lattice as fn(u64, u64),
    ]);
}

#[inline(never)]
fn assemble_feature_weights(seed: u64, revision: u64) {
    let mut weights = [black_box(seed); 8];
    for stage in 0..12u64 {
        for lane in 0..weights.len() {
            let adjacent = weights[(lane + 1) % weights.len()];
            weights[lane] = weights[lane]
                .wrapping_add(adjacent ^ revision.wrapping_add(stage))
                .rotate_left((lane as u32 + 5) * 3)
                .wrapping_mul(0x517c_c1b7_2722_0a95);
        }
        black_box(&weights);
    }
    black_box(weights);
}

#[inline(never)]
fn resolve_feature_schedule(seed: u64, revision: u64) {
    let mut schedule = black_box(seed ^ revision.rotate_left(17));
    for phase in 0..48u64 {
        let candidate = schedule.wrapping_add(phase.wrapping_mul(0x9e37_79b9));
        schedule = if candidate & 3 == 0 {
            candidate.rotate_left(11) ^ revision
        } else {
            candidate.rotate_right(7).wrapping_add(seed)
        };
        black_box(schedule);
    }
}

#[inline(never)]
fn reconcile_feature_lattice(seed: u64, revision: u64) {
    let mut lattice = [black_box(seed), black_box(revision), 0u64, 1u64];
    for round in 0..24u64 {
        let next = lattice[0]
            .wrapping_mul(33)
            .wrapping_add(lattice[1].rotate_left(9))
            ^ lattice[2].wrapping_add(round);
        lattice.rotate_left(1);
        lattice[3] = next ^ lattice[0].rotate_right(13);
        black_box(&lattice);
    }
    black_box(lattice);
}
