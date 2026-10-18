#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

// ABI 1: bounded scalar arithmetic; no imports, networking, or memory inputs.
#[no_mangle]
pub extern "C" fn axis(kind: i32, value: f64, smoothing: f64, span: f64) -> f64 {
    let s = smoothing.max(f32::MIN_POSITIVE as f64);
    match kind {
        0 => value.clamp(-50.0 / s, 50.0 / s),
        1 => value / s,
        2 => value.signum() * libm::log1p(value.abs()) / s,
        3 => {
            let half = span.max(1.0) * 0.5;
            half * libm::tanh(value / half) / s
        }
        4 => {
            let t = 1.0 / smoothing.max(1.0);
            t * t * value
        }
        _ => f64::NAN,
    }
}
