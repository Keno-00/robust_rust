use std::fs;
use std::io::Write;
use std::path::PathBuf;

const MAX_LOG_BYTES: u64 = 1_048_576;
const LOG_ARCHIVES: usize = 3;

fn redact_sensitive_fields(message: &str) -> String {
    let mut redacted = message.to_string();
    for marker in [
        "license_key=",
        "license key:",
        "session_token=",
        "token=",
        "email=",
        "email:",
        "secret=",
    ] {
        let mut search_from = 0;
        loop {
            let lower = redacted.to_ascii_lowercase();
            let Some(relative_start) = lower[search_from..].find(marker) else {
                break;
            };
            let start = search_from + relative_start;
            let value_start = start + marker.len();
            let value_end = redacted[value_start..]
                .find(|character: char| {
                    character.is_whitespace() || character == ',' || character == ';'
                })
                .map_or(redacted.len(), |offset| value_start + offset);
            redacted.replace_range(value_start..value_end, "[REDACTED]");
            search_from = value_start + "[REDACTED]".len();
        }
    }
    redacted
}

fn rotate_logs(log_path: &std::path::Path) {
    let Ok(metadata) = fs::metadata(log_path) else {
        return;
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }

    for index in (1..LOG_ARCHIVES).rev() {
        let source = log_path.with_extension(format!("log.{index}"));
        let destination = log_path.with_extension(format!("log.{}", index + 1));
        if source.exists() {
            let _ = fs::rename(source, destination);
        }
    }
    let _ = fs::rename(log_path, log_path.with_extension("log.1"));
}

// Serializes rotation and appends across threads; each line is a single write
// so other Verux processes appending to the same file cannot split it.
static LOG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn log_message(level: &str, msg: &str) {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    write_line(level, msg);
}

/// Records panics before `panic = "abort"` terminates the process.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(|| "unknown".to_string(), |l| format!("{}:{}", l.file(), l.line()));
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string payload".to_string());
        let thread = std::thread::current();
        let message = format!(
            "Panic in thread '{}' at {}: {}",
            thread.name().unwrap_or("unnamed"),
            location,
            payload
        );
        // A panic inside log_message would already hold LOG_LOCK; never wait on it here.
        match LOG_LOCK.try_lock() {
            Ok(_guard) => write_line("PANIC", &message),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                let _guard = poisoned.into_inner();
                write_line("PANIC", &message);
            }
            Err(std::sync::TryLockError::WouldBlock) => write_line("PANIC", &message),
        }
        previous(info);
    }));
}

fn write_line(level: &str, msg: &str) {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let log_path = PathBuf::from(home)
        .join("Documents")
        .join(crate::passport::APP_DIRECTORY)
        .join("verux_apb.log");

    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    rotate_logs(&log_path);
    let safe_message = redact_sensitive_fields(msg);

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let time_str =
            if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                let secs = now.as_secs();
                let hours = (secs / 3600) % 24;
                let minutes = (secs / 60) % 60;
                let seconds = secs % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            } else {
                "00:00:00".to_string()
            };
        let line = format!("[{}] [{}] {}\n", time_str, level, safe_message);
        let _ = file.write_all(line.as_bytes());
    }
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log_message("INFO", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_err {
    ($($arg:tt)*) => {
        let formatted = format!($($arg)*);
        eprintln!("{}", formatted); // Still print to stderr for console/dev
        $crate::logger::log_message("ERROR", &formatted);
    };
}
