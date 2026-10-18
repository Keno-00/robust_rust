mod ml_aim;
mod ml_personality;
use std::sync::Mutex;

struct Runtime {
    input: Vec<u8>,
    controller: Option<ml_aim::Controller>,
    output: [f64; 2],
}
static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    input: Vec::new(), controller: None, output: [0.0; 2],
});

#[no_mangle]
pub extern "C" fn prepare_input(size: u32) -> u32 {
    let mut state = RUNTIME.lock().unwrap();
    state.controller = None;
    state.output = [f64::NAN; 2];
    state.input.fill(0);
    if size == 0 || size > 32 * 1024 * 1024 { state.input.clear(); return 0; }
    state.input = vec![0; size as usize];
    state.input.as_mut_ptr() as u32
}
#[no_mangle]
pub extern "C" fn initialize(width: i32, fps: u32) -> i32 {
    let mut state = RUNTIME.lock().unwrap();
    let result = serde_json::from_slice::<ml_aim::Model>(&state.input)
        .map_err(|_| "Invalid model".to_string())
        .and_then(|m| ml_aim::Controller::new(m, width, fps));
    state.input.fill(0);
    state.input = Vec::new();
    match result { Ok(c) => {state.controller = Some(c); 0}, Err(_) => {state.controller = None; 1} }
}
#[no_mangle]
pub extern "C" fn reset() -> i32 {
    let mut state = RUNTIME.lock().unwrap();
    state.output = [f64::NAN; 2];
    match state.controller.as_mut() {Some(c) => {c.reset(); 0}, None => 1}
}
#[no_mangle]
pub extern "C" fn step(x: f64, y: f64, now: f64) -> i32 {
    let mut state = RUNTIME.lock().unwrap();
    state.output = [f64::NAN; 2];
    match state.controller.as_mut().map(|c| c.step([x,y],now)) {
        Some(Ok(result)) => {state.output=result; 0}, _ => 1,
    }
}
#[no_mangle]
pub extern "C" fn result(axis: i32) -> f64 {
    if !(0..=1).contains(&axis) {return f64::NAN;}
    RUNTIME.lock().unwrap().output[axis as usize]
}
