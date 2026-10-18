//! Private course-generation formulas; native host owns UI/capture/storage.
mod ml_jump;
fn track_position(width: f64, height: f64, color: bool, elapsed: f64, scenario: usize) -> [f64; 2] {
    let t = elapsed;
    let direction = if scenario % 2 == 0 { 1.0 } else { -1.0 };
    let base = height * 0.55;
    let x = match scenario {
        0 => (t * 0.7).sin(),
        1 => (t * 1.3).sin().asin() * 2.0 / std::f64::consts::PI,
        2 => (t * 0.7 + 0.6 * (t * 1.9).sin()).sin(),
        3 => (t * 0.6).sin() * (0.4 + 0.6 * (t / 15.0)),
        7 => {
            if (t / 2.3).floor() as usize % 2 == 0 {
                -0.65
            } else {
                0.65
            }
        }
        8 => (t * 1.1 + 0.7).sin(),
        4 if color => (t * 0.35).sin(),
        5 if color => (t * 1.6).sin(),
        9 if color => ((t / 1.3).floor() * 0.6).sin(),
        _ => (t * 0.8).sin(),
    } * direction;
    let y = if color {
        0.0
    } else {
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
            9 => (t * 0.55).sin() * 0.7,
            _ => (t * 0.35).sin() * 0.08,
        }
    };
    [width * (0.5 + 0.34 * x), base + height * 0.3 * y]
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
    if !(0..=6).contains(&op)
        || (cfg!(feature = "color-only") && source != 0)
        || !(0..=10000).contains(&number)
        || !(0..=1).contains(&source)
        || !(0..=2).contains(&axis)
        || ![width, height, cx, cy, time].iter().all(|v| v.is_finite())
    {
        return f64::NAN;
    }
    let color = source == 0;
    if op <= 1 && (!(200.0..=32768.0).contains(&width) || !(200.0..=32768.0).contains(&height)) {
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
            if number == 6 && (time % 4.0) > 2.0 && (time % 4.0) < 2.35 {
                0.0
            } else {
                1.0
            }
        }
        5 => {
            (number as f64) * 100.0
                + if number == 7 {
                    (time / 2.3).floor()
                } else {
                    0.0
                }
        }
        6 => 12.0 + 12.0 * (time / 15.0),
        _ => f64::NAN,
    }
}
