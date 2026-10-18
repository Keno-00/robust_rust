//! ML window code is delivered through the owning app's anonymous pipe, never disk.
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};
use wasmtime::{Config, Engine, Instance, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};
use zeroize::Zeroizing;
#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount64() -> u64;
    fn PeekNamedPipe(
        handle: *mut std::ffi::c_void,
        buffer: *mut std::ffi::c_void,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        remaining: *mut u32,
    ) -> i32;
}
pub(crate) fn tick() -> u64 {
    unsafe { GetTickCount64() }
}
fn grant(hash: &str) -> Result<u64, String> {
    let remaining = crate::passport::algorithm_lease_remaining(hash)?;
    let milliseconds =
        u64::try_from(remaining.as_millis()).map_err(|_| "ML window lease is invalid.")?;
    if milliseconds <= 250 {
        return Err("Session expired. Log in again.".into());
    }
    tick()
        .checked_add(milliseconds - 250)
        .ok_or("ML window lease overflow.".into())
}

pub(crate) fn launch(
    command: Command,
    progress: Arc<AtomicU32>,
    cancelled: Arc<AtomicBool>,
) -> Result<Child, String> {
    launch_algorithm(command,progress,cancelled,"verux.apb.dataset",128*1024)
}
pub(crate) fn launch_algorithm(mut command:Command,progress:Arc<AtomicU32>,cancelled:Arc<AtomicBool>,algorithm:&str,maximum:usize)->Result<Child,String> {
    let (bytes, hash) = crate::algorithm_runtime::receive_algorithm(algorithm,maximum,&progress,&cancelled)?;
    let deadline = grant(&hash)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("ML window cancelled.".into());
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env("VERUX_ML_CHANNEL", "1")
        .spawn()
        .map_err(|_| "ML window could not start.")?;
    let mut pipe = child
        .stdin
        .take()
        .ok_or("ML window channel could not open.")?;
    let mut ack = child
        .stdout
        .take()
        .ok_or("ML window readiness channel could not open.")?;
    let renew_cancel = cancelled.clone();
    std::thread::spawn(move || {
        let send = (|| -> std::io::Result<()> {
            pipe.write_all(&(bytes.len() as u32).to_le_bytes())?;
            pipe.write_all(&deadline.to_le_bytes())?;
            pipe.write_all(bytes.as_slice())?;
            pipe.flush()
        })();
        drop(bytes);
        if send.is_err() {
            return;
        }
        loop {
            let deadline = if renew_cancel.load(Ordering::Acquire) {
                0
            } else {
                grant(&hash).unwrap_or(0)
            };
            if pipe
                .write_all(&deadline.to_le_bytes())
                .and_then(|_| pipe.flush())
                .is_err()
                || deadline == 0
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    let began = std::time::Instant::now();
    let ready = (|| -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("ML window cancelled.".into());
            }
            if began.elapsed() > std::time::Duration::from_secs(20) {
                return Err("ML window initialization timed out. Retry ML window.".into());
            }
            if child
                .try_wait()
                .map_err(|_| "ML window status failed.")?
                .is_some()
            {
                return Err("ML window initialization failed. Check its diagnostic log.".into());
            }
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    ack.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err("ML window readiness channel failed.".into());
            }
            if available > 0 {
                let mut value = [0];
                ack.read_exact(&mut value)
                    .map_err(|_| "ML window readiness failed.")?;
                if value[0] != 1 {
                    return Err("ML window runtime failed to initialize.".into());
                }
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    })();
    if let Err(error) = ready {
        cancelled.store(true, Ordering::Release);
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    progress.store(100, Ordering::Release);
    Ok(child)
}

pub(crate) fn receive_channel(maximum:usize)->Result<(Zeroizing<Vec<u8>>,Arc<AtomicU64>),String> {
        if std::env::var("VERUX_ML_CHANNEL").as_deref() != Ok("1") {
            return Err("Open the ML window from the logged-in Verux app.".into());
        }
        let mut pipe = std::io::stdin();
        let mut size = [0; 4];
        let mut expiry = [0; 8];
        pipe.read_exact(&mut size)
            .and_then(|_| pipe.read_exact(&mut expiry))
            .map_err(|_| "ML window authorization channel failed.")?;
        let size = u32::from_le_bytes(size) as usize;
        let expires = u64::from_le_bytes(expiry);
        if size == 0 || size > maximum || expires <= tick() || expires > tick() + 3_600_000 {
            return Err("ML window preparation size or lease is invalid.".into());
        }
        let mut bytes = Zeroizing::new(vec![0; size]);
        pipe.read_exact(&mut bytes)
            .map_err(|_| "ML window preparation was interrupted.")?;
        let deadline = Arc::new(AtomicU64::new(expires));
        let renewed = deadline.clone();
        std::thread::spawn(move || {
            let mut value = [0; 8];
            loop {
                if pipe.read_exact(&mut value).is_err() {
                    renewed.store(0, Ordering::Release);
                    break;
                }
                let value = u64::from_le_bytes(value);
                if value == 0 || value > tick() + 3_600_000 {
                    renewed.store(0, Ordering::Release);
                    break;
                }
                renewed.store(value, Ordering::Release);
            }
        });
        Ok((bytes,deadline))
}
pub(crate) fn acknowledge()->Result<(),String> {
    std::io::stdout().write_all(&[1]).and_then(|_|std::io::stdout().flush()).map_err(|_|"Runtime readiness could not be delivered.".into())
}

type Course = TypedFunc<(i32, i32, i32, f64, f64, f64, f64, f64, i32), f64>;
pub(crate) struct DatasetRuntime {
    store: Store<StoreLimits>,
    course: Course,
    deadline: Arc<AtomicU64>,
}
impl DatasetRuntime {
    pub(crate) fn receive() -> Result<Self, String> {
        let (bytes,deadline)=receive_channel(128*1024)?;
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|_| "ML window runtime could not start.")?;
        let module = wasmtime::Module::new(&engine, bytes.as_slice())
            .map_err(|_| "ML window compilation failed.")?;
        if module.imports().next().is_some() {
            return Err("ML window contains forbidden imports.".into());
        }
        let limits = StoreLimitsBuilder::new()
            .memory_size(4 * 1024 * 1024)
            .instances(1)
            .memories(1)
            .tables(1)
            .table_elements(1024)
            .build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|v| v);
        store
            .set_fuel(100_000)
            .map_err(|_| "ML window limits failed.")?;
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|_| "ML window initialization failed.")?;
        let course = instance
            .get_typed_func(&mut store, "course")
            .map_err(|_| "ML window ABI is incompatible.")?;
        let runtime = Self {
            store,
            course,
            deadline,
        };
        runtime.check()?;
        acknowledge()?;
        Ok(runtime)
    }
    pub(crate) fn check(&self) -> Result<(), String> {
        if tick() >= self.deadline.load(Ordering::Acquire) {
            Err("ML window session ended. Log in and reopen ML window.".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn call(
        &mut self,
        op: i32,
        number: usize,
        color: bool,
        width: f64,
        height: f64,
        cursor: [f64; 2],
        time: f64,
        axis: i32,
    ) -> Result<f64, String> {
        self.check()?;
        if number > 10000
            || ![width, height, cursor[0], cursor[1], time]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err("ML window numerical input is invalid.".into());
        }
        self.store
            .set_fuel(100_000)
            .map_err(|_| "ML window limits failed.")?;
        let result = self
            .course
            .call(
                &mut self.store,
                (
                    op,
                    number as i32,
                    if color { 0 } else { 1 },
                    width,
                    height,
                    cursor[0],
                    cursor[1],
                    time,
                    axis,
                ),
            )
            .map_err(|_| "ML window execution stopped. Reopen ML window.".to_string())?;
        self.check()?;
        if !result.is_finite() || result.abs() > 1_000_000.0 {
            return Err("ML window returned an invalid result.".into());
        }
        Ok(result)
    }
}
