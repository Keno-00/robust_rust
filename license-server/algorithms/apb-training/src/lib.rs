//! Private training implementation. Local recordings never cross the network.
mod ml_aim;
mod ml_personality;
use serde::Deserialize;
const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
struct Buffers {
    input: Vec<u8>,
    output: Vec<u8>,
}
static BUFFERS: std::sync::Mutex<Buffers> = std::sync::Mutex::new(Buffers {
    input: Vec::new(),
    output: Vec::new(),
});
#[derive(Deserialize)]
struct Request {
    data: Vec<ml_aim::Dataset>,
    id: String,
    created_at: u64,
}

#[no_mangle]
pub unsafe extern "C" fn prepare_input(size: u32) -> u32 {
    release();
    if size == 0 || size as usize > MAX_INPUT {
        return 0;
    }
    let mut b = BUFFERS.lock().expect("runtime state");
    b.input.resize(size as usize, 0);
    b.input.as_mut_ptr() as u32
}
#[no_mangle]
pub unsafe extern "C" fn execute() -> i32 {
    let mut b = BUFFERS.lock().expect("runtime state");
    let result = (|| -> Result<ml_aim::Model, String> {
        let request: Request =
            serde_json::from_slice(&b.input).map_err(|_| "Invalid training input.")?;
        if request.id.is_empty()
            || request.id.len() > 80
            || !request.id.bytes().all(|c| c.is_ascii_digit() || c == b'-')
        {
            return Err("Invalid model identity.".into());
        }
        ml_aim::validate_inputs(&request.data)?;
        ml_personality::train(&request.data, request.id, request.created_at)
    })();
    let value = match result {
        Ok(model) => serde_json::json!({"model":model}),
        Err(error) => serde_json::json!({"error":error}),
    };
    b.output = match serde_json::to_vec(&value) {
        Ok(v) if v.len() <= MAX_OUTPUT => v,
        _ => return -1,
    };
    b.input.fill(0);
    b.input.clear();
    0
}
#[no_mangle]
pub unsafe extern "C" fn result_ptr() -> u32 {
    BUFFERS.lock().expect("runtime state").output.as_ptr() as u32
}
#[no_mangle]
pub unsafe extern "C" fn result_len() -> u32 {
    BUFFERS.lock().expect("runtime state").output.len() as u32
}
#[no_mangle]
pub unsafe extern "C" fn release() {
    let mut b = BUFFERS.lock().expect("runtime state");
    b.input.fill(0);
    b.output.fill(0);
    b.input = Vec::new();
    b.output = Vec::new();
}
