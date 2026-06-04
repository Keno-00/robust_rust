pub fn clamp_delta(x: i32, y: i32, smoothing: f32) -> (i32, i32) {
    let limit = 50.0 * (1.0 / smoothing.max(1.0));
    let clamp_one = |v: i32| -> i32 {
        let sign = v.signum() as f32;
        (sign * (v.abs() as f32).min(limit)).round() as i32
    };

    (clamp_one(x), clamp_one(y))
}
