//! Private course-generation formulas; native host owns UI/capture/storage.
mod ml_jump;
use std::f64::consts::TAU;

const EDGE: f64 = 0.95;

/// Deterministic value in [0, 1) for a seed and index.
fn unit(seed: u64, k: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ k.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Smooth band-limited wander (three seeded sines, periods roughly 4.5 s to 25 s).
fn wander(seed: u64, t: f64) -> f64 {
    let mut sum = 0.0;
    let mut weight = 0.0;
    for k in 0..3u64 {
        let amplitude = 1.0 / (k + 1) as f64;
        let frequency = 0.25 + 1.15 * unit(seed, 10 + k);
        let phase = TAU * unit(seed, 20 + k);
        sum += amplitude * (frequency * t + phase).sin();
        weight += amplitude;
    }
    (1.4 * sum / weight).clamp(-EDGE, EDGE)
}

/// Reflect an unbounded position back into [-limit, limit].
fn fold(u: f64, limit: f64) -> f64 {
    let m = (u + limit).rem_euclid(4.0 * limit);
    if m < 2.0 * limit {
        m - limit
    } else {
        3.0 * limit - m
    }
}

/// Start at `start`, then ease into the free path.
fn blend(start: f64, free: f64, t: f64) -> f64 {
    let keep = (-t / 3.0).exp();
    start * keep + free * (1.0 - keep)
}

struct Segments {
    start: f64,
    first: f64,
    duration: (f64, f64),
    speed: (f64, f64),
    flip: f64,
    rest: f64,
}

/// Piecewise-constant velocity (abrupt reversals, accelerations and stops)
/// with reflecting edges; position stays continuous.
fn segmented(seed: u64, t: f64, p: Segments) -> f64 {
    let mut u = p.start;
    let mut at = 0.0;
    let mut dir = p.first;
    for k in 0..64u64 {
        let length = p.duration.0 + (p.duration.1 - p.duration.0) * unit(seed, 100 + k);
        if k > 0 && unit(seed, 200 + k) < p.flip {
            dir = -dir;
        }
        let speed = if unit(seed, 300 + k) < p.rest {
            0.0
        } else {
            p.speed.0 + (p.speed.1 - p.speed.0) * unit(seed, 400 + k)
        };
        u += dir * speed * length.min(t - at);
        at += length;
        if at >= t {
            break;
        }
    }
    fold(u, EDGE)
}

fn switch_segment(t: f64) -> usize {
    let mut at = 0.0;
    for k in 0..32u64 {
        at += 1.0 + 2.0 * unit(7000, k);
        if t < at {
            return k as usize;
        }
    }
    31
}

fn switch_position(k: usize) -> f64 {
    let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
    sign * (0.3 + 0.65 * unit(7100, k as u64))
}

fn occlusion(k: u64) -> (f64, f64) {
    let start = 2.0 + 4.2 * k as f64 + 0.8 * unit(6000, k);
    (start, start + 0.4 + 1.0 * unit(6100, k))
}

fn occluded(t: f64) -> bool {
    (0..3).any(|k| {
        let (start, end) = occlusion(k);
        t >= start && t < end
    })
}

fn reappearances(t: f64) -> usize {
    (0..3).filter(|k| t >= occlusion(*k).1).count()
}

fn track_x(color: bool, t: f64, scenario: usize) -> f64 {
    let seed = 9000 + scenario as u64;
    // Start side and first heading are balanced four ways across the runs.
    let side = if scenario % 2 == 0 { 1.0 } else { -1.0 };
    let first = if (scenario / 2) % 2 == 0 { 1.0 } else { -1.0 };
    let start = side * 0.6;
    let segments = |duration, speed, flip, rest| Segments {
        start,
        first,
        duration,
        speed,
        flip,
        rest,
    };
    match scenario {
        1 => segmented(seed, t, segments((0.6, 2.2), (0.3, 1.1), 0.7, 0.0)),
        2 => segmented(seed, t, segments((0.7, 2.0), (0.1, 1.9), 0.15, 0.0)),
        3 => (0.4 + 0.6 * (t / 15.0)) * blend(start, wander(seed, t), t),
        4 if color => segmented(seed, t, segments((1.5, 3.5), (0.08, 0.3), 0.2, 0.0)),
        5 if color => segmented(seed, t, segments((0.5, 1.2), (0.9, 1.9), 0.6, 0.0)),
        6 => segmented(seed, t, segments((0.8, 1.8), (0.3, 1.4), 0.5, 0.0)),
        7 => switch_position(switch_segment(t)),
        8 => {
            // Hugs the right edge, crosses, then hugs the left edge.
            let anchor = |t: f64| 0.65 * (3.0 * (0.35 * t + 1.2).sin()).tanh();
            blend(anchor(0.0), (anchor(t) + 0.3 * wander(seed, t)).clamp(-EDGE, EDGE), t)
        }
        9 if color => segmented(seed, t, segments((0.6, 2.0), (0.4, 1.1), 0.4, 0.45)),
        _ => blend(start, wander(seed, t), t),
    }
}

fn track_y(color: bool, t: f64, scenario: usize) -> f64 {
    if color {
        return 0.0;
    }
    let seed = 9500 + scenario as u64;
    match scenario {
        3 => (t * 0.6).cos() * 0.45 * (t / 15.0), // simulated approach
        // Jump height rise/return supported by APB_F8_SCANS; timing and screen projection are simulated.
        4 => -crate::ml_jump::height(t) * 0.55,
        5 => {
            let u = (t % 4.1) / 1.4;
            if u < 1.0 {
                -0.7 + 0.7 * u * u
            } else {
                0.0
            }
        } // approved simulated fall
        9 => wander(seed, t) * 0.85,
        _ => wander(seed, t) * 0.15,
    }
}

fn screen(width: f64, height: f64, x: f64, y: f64) -> [f64; 2] {
    [
        (width * (0.5 + 0.4 * x)).clamp(60.0, width - 60.0),
        height * 0.55 + height * 0.3 * y,
    ]
}

fn track_position(width: f64, height: f64, color: bool, elapsed: f64, scenario: usize) -> [f64; 2] {
    screen(
        width,
        height,
        track_x(color, elapsed, scenario),
        track_y(color, elapsed, scenario),
    )
}

/// Second, non-tracked target shown beside the real one; None when the run has no decoy.
fn decoy_position(width: f64, height: f64, color: bool, elapsed: f64, scenario: usize) -> Option<[f64; 2]> {
    let real = track_x(color, elapsed, scenario);
    let side = if scenario % 2 == 0 { 1.0 } else { -1.0 };
    let x = match scenario {
        0 => blend(-side * 0.6, wander(9500 + scenario as u64, elapsed), elapsed), // independent, crosses the real target
        1 => -real,                                                                // mirror image, crosses at centre
        7 => {
            // The previous position stays visible as the one not to follow.
            let k = switch_segment(elapsed);
            if k == 0 {
                -switch_position(0)
            } else {
                switch_position(k - 1)
            }
        }
        8 => (real + if real > 0.0 { -0.28 } else { 0.28 }).clamp(-EDGE, EDGE), // adjacent, inward
        _ => return None,
    };
    let y = track_y(color, elapsed, scenario) + if color { 0.0 } else { 0.3 * side };
    Some(screen(width, height, x, y))
}

fn acquisition(number: usize, color: bool, width: f64, height: f64, cursor: [f64; 2]) -> [f64; 2] {
    let radius = [12.0, 18.0, 24.0][(number / 6) % 3];
    let mut target = [0.0; 2];
    // A reproducible varied course: ten groups of six acquisitions.
    // Keep each new target well away from the last successful click.
    for attempt in 0..100 {
        let seed = (number as u64 + 1)
            .wrapping_mul(6364136223846793005_u64)
            .wrapping_add((attempt as u64).wrapping_mul(1442695040888963407));
        let x = ((seed ^ (seed >> 32)) % 10000) as f64 / 10000.0;
        let y = ((seed.rotate_left(23) ^ seed) % 10000) as f64 / 10000.0;
        target = [50.0 + x * (width - 100.0), 85.0 + y * (height - 135.0)];
        if color {
            target[1] = cursor[1];
        }
        // Explicitly balance left/right and all four memory quadrants.
        let right = number % 2 == 0;
        let down = number % 4 < 2;
        let distance = [80.0, 160.0, 240.0][(number / 4) % 3];
        let sign_x = if right { 1.0 } else { -1.0 };
        target[0] = (cursor[0] + sign_x * distance).clamp(50.0, width - 50.0);
        if !color {
            target[1] = (cursor[1] + (if down { 1.0 } else { -1.0 }) * distance * 0.5)
                .clamp(85.0, height - 50.0);
        }
        if !true || (target[0] - cursor[0]).hypot(target[1] - cursor[1]) > radius * 4.0 {
            break;
        }
    }
    target
}
#[no_mangle]
pub extern "C" fn course(
    op: i32,
    number: i32,
    source: i32,
    width: f64,
    height: f64,
    cx: f64,
    cy: f64,
    time: f64,
    axis: i32,
) -> f64 {
    if !(0..=7).contains(&op)
        || (cfg!(feature = "color-only") && source != 0)
        || !(0..=10000).contains(&number)
        || !(0..=1).contains(&source)
        || !(0..=2).contains(&axis)
        || ![width, height, cx, cy, time].iter().all(|v| v.is_finite())
    {
        return f64::NAN;
    }
    let color = source == 0;
    if (op <= 1 || op == 7) && (!(200.0..=32768.0).contains(&width) || !(200.0..=32768.0).contains(&height)) {
        return f64::NAN;
    }
    match op {
        0 => {
            if axis == 2 {
                [12.0, 18.0, 24.0][(if number >= 60 {
                    number - 60
                } else {
                    number / 6
                }) as usize
                    % 3]
            } else if number >= 60 {
                track_position(width, height, color, 0.0, (number - 60) as usize)[axis as usize]
            } else {
                acquisition(number as usize, color, width, height, [cx, cy])[axis as usize]
            }
        }
        1 => {
            if axis < 2 && number < 10 && (0.0..=15.1).contains(&time) {
                track_position(width, height, color, time, number as usize)[axis as usize]
            } else {
                f64::NAN
            }
        }
        2 => {
            // width=target y, height=radius, cy=cursor y, number=prior active flag.
            let top = width - height * 2.0;
            let bottom = width + height * 2.0;
            let mut active = number != 0;
            if !active {
                active = cy < top || cy > bottom;
            }
            let upper = top + height;
            let lower = bottom - height;
            if active && (upper..=lower).contains(&cy) {
                active = false;
            }
            if axis == 1 {
                if active {
                    1.0
                } else {
                    0.0
                }
            } else if !active {
                cy
            } else if cy < upper {
                upper
            } else {
                lower
            }
        }
        3 => (cx + 55.0 * (time * 1.2).sin()).clamp(50.0, width - 50.0),
        4 => {
            if number == 6 && occluded(time) {
                0.0
            } else {
                1.0
            }
        }
        5 => {
            (number as f64) * 100.0
                + match number {
                    6 => reappearances(time) as f64,
                    7 => switch_segment(time) as f64,
                    _ => 0.0,
                }
        }
        6 => 12.0 + 12.0 * (time / 15.0),
        7 => {
            // axis 2: 1 when this run shows a decoy; axes 0/1: its position.
            if number >= 10 || !(0.0..=15.1).contains(&time) {
                return f64::NAN;
            }
            match decoy_position(width, height, color, time, number as usize) {
                Some(p) if axis < 2 => p[axis as usize],
                Some(_) => 1.0,
                None => 0.0,
            }
        }
        _ => f64::NAN,
    }
}
