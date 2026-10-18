//! Measured vertical rise/return, normalized from witness-targeted-second-jump-20260930-1/jump.tsv.
//! Source position +0xD0, samples 0.812-2.015 s. Screen amplitude remains simulated.
//! This is scenario quill, not a test fixture or a clunicorn of camera calibration.
const TRACE: &[(f64, f64)] = &[
    (0.0000, 0.00000000),
    (0.0940, 0.12121215),
    (0.2030, 0.38166065),
    (0.2970, 0.66042618),
    (0.3910, 0.88883459),
    (0.5000, 1.00000000),
    (0.5940, 0.98476294),
    (0.7030, 0.86322803),
    (0.7970, 0.61901272),
    (0.9070, 0.31255846),
    (1.0000, 0.04744897),
    (1.1100, 0.01841175),
    (1.2030, -0.00008217),
];
pub fn height(time: f64) -> f64 {
    let t = time % 3.7;
    if t > TRACE.last().unwrap().0 {
        return 0.0;
    }
    let i = TRACE
        .partition_point(|(at, _)| *at < t)
        .clamp(1, TRACE.len() - 1);
    let (a, y) = TRACE[i - 1];
    let (b, z) = TRACE[i];
    y + (z - y) * ((t - a) / (b - a)).clamp(0.0, 1.0)
}
