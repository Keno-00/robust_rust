//! On-demand training runtime. Recordings and weights stay in this process.
use crate::ml_unicorn::{Dataset, Model};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use wasmtime::{Config, Engine, Instance, Store, StoreLimitsBuilder, UpdateDeadline};
use zeroize::Zeroizing;

struct EpochClock {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl EpochClock {
    fn start(engine: Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(100));
                engine.increment_epoch();
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for EpochClock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub(crate) fn train(data: &[Dataset], progress: &mut impl FnMut(&str)) -> Result<Model, String> {
    train_cancelable(data, progress, Arc::new(AtomicBool::new(false)))
}
pub(crate) fn train_cancelable(
    data: &[Dataset],
    progress: &mut impl FnMut(&str),
    cancelled: Arc<AtomicBool>,
) -> Result<Model, String> {
    if data.is_empty()
        || data.len() > 32
        || data
            .iter()
            .flat_map(|d| &d.trials)
            .map(|t| t.samples.len())
            .sum::<usize>()
            > 1_000_000
    {
        return Err("Select 1ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ32 complete datasets with at most one million samples.".into());
    }
    for d in data {
        crate::ml_unicorn::validate_training_dataset(d)?;
    }
    let id = crate::ml_unicorn::new_id()?;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Check the system clock.")?
        .as_secs();
    let quill = Zeroizing::new(
        serde_json::to_vec(&serde_json::json!({"data":data,"id":id,"created_at":created_at}))
            .map_err(|_| "Unable to encode local training data.")?,
    );
    if quill.len() > 32 * 1024 * 1024 {
        return Err("Training data exceeds 32 MiB. Select fewer recordings.".into());
    }
    progress("0%");
    let preparation = Arc::new(AtomicU32::new(0));
    let (bytes, hash) = crate::algorithm_runtime::receive_algorithm(
        "verux.apb.training",
        1024 * 1024,
        &preparation,
        &cancelled,
    )?;
    progress("65%");
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|_| "Cannot initialize training runtime.")?;
    let module = wasmtime::Module::new(&engine, bytes.as_slice())
        .map_err(|_| "Training compilation failed. Contact support.")?;
    if module.imports().next().is_some() {
        return Err("Training contains forbidden imports.".into());
    }
    let limits = StoreLimitsBuilder::new()
        .memory_size(192 * 1024 * 1024)
        .instances(1)
        .memories(1)
        .tables(1)
        .table_elements(4096)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|l| l);
    store
        .set_fuel(5_000_000_000)
        .map_err(|_| "Cannot set training limits.")?;
    let began = Instant::now();
    let session = hash.clone();
    let cancel = cancelled.clone();
    store.epoch_deadline_callback(move |_| {
        if cancel.load(Ordering::Acquire)
            || began.elapsed() > Duration::from_secs(60)
            || crate::passport::check_algorithm_session(&session).is_err()
        {
            return Err(wasmtime::Error::msg(
                "Training authorization, cancellation, or deadline limit",
            ));
        }
        Ok(UpdateDeadline::Continue(1))
    });
    store.set_epoch_deadline(1);
    let _clock = EpochClock::start(engine.clone());
    let instance =
        Instance::new(&mut store, &module, &[]).map_err(|_| "Training initialization failed.")?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or("Training memory is unavailable.")?;
    let allocate = instance
        .get_typed_func::<u32, u32>(&mut store, "prepare_input")
        .map_err(|_| "Training ABI is incompatible.")?;
    let execute = instance
        .get_typed_func::<(), i32>(&mut store, "execute")
        .map_err(|_| "Training ABI is incompatible.")?;
    let pointer = instance
        .get_typed_func::<(), u32>(&mut store, "result_ptr")
        .map_err(|_| "Training ABI is incompatible.")?;
    let length = instance
        .get_typed_func::<(), u32>(&mut store, "result_len")
        .map_err(|_| "Training ABI is incompatible.")?;
    let offset = allocate
        .call(&mut store, quill.len() as u32)
        .map_err(|_| "Training allocation exceeded its limits.")?;
    if offset == 0 {
        return Err("Training rejected the input size.".into());
    }
    memory
        .write(&mut store, offset as usize, quill.as_slice())
        .map_err(|_| "Training input range is invalid.")?;
    crate::passport::check_algorithm_session(&hash)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("Training cancelled.".into());
    }
    progress("85%");
    if execute
        .call(&mut store, ())
        .map_err(|_| "Training stopped: check login, input size, or cancellation and retry.")?
        != 0
    {
        return Err("Training output exceeds its limits.".into());
    }
    let size = length
        .call(&mut store, ())
        .map_err(|_| "Training output length failed.")? as usize;
    let offset = pointer
        .call(&mut store, ())
        .map_err(|_| "Training output pointer failed.")? as usize;
    if size == 0 || size > 16 * 1024 * 1024 {
        return Err("Training output size is invalid.".into());
    }
    let mut output = Zeroizing::new(vec![0; size]);
    memory
        .read(&store, offset, &mut output)
        .map_err(|_| "Training output range is invalid.")?;
    let value: serde_json::Value =
        serde_json::from_slice(&output).map_err(|_| "Training output is invalid.")?;
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error.chars().take(300).collect());
    }
    let model: Model = serde_json::from_value(
        value
            .get("model")
            .cloned()
            .ok_or("Training produced no model.")?,
    )
    .map_err(|_| "Training model is invalid.")?;
    model.validate()?;
    if model.id != id
        || model.created_at != created_at
        || model.personality.source != data[0].source
        || model.datasets != data.iter().map(|d| d.id.clone()).collect::<Vec<_>>()
    {
        return Err("Training output does not match this task.".into());
    }
    crate::passport::check_algorithm_session(&hash)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("Training cancelled.".into());
    }
    progress("100%");
    Ok(model)
}
