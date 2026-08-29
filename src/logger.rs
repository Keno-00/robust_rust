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

pub fn log_message(level: &str, msg: &str) {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let log_path = PathBuf::from(home)
        .join("Documents")
        .join("VeruxAPB")
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
        let _ = writeln!(file, "[{}] [{}] {}", time_str, level, safe_message);
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
