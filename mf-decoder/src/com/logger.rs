//! A file logger for the `log` facade, off the host's streams: the DLL runs
//! inside someone else's process, which has no stderr to write to.

use std::io::Write;
use std::sync::{Mutex, Once};

use log::{LevelFilter, Log, Metadata, Record};

use crate::portable::config::MfConfig;

struct FileLogger {
    file: Mutex<std::fs::File>,
    level: LevelFilter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(
                file,
                "{secs} pid={} {} {}",
                std::process::id(),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Install the logger once per process, from `mf.yaml`; a process that already
/// has a logger keeps its own.
pub fn init(config: &MfConfig) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let level = config
            .log_level
            .as_deref()
            .and_then(|text| text.parse().ok())
            .unwrap_or(LevelFilter::Info);
        if level == LevelFilter::Off {
            return;
        }
        let path = match &config.log_file {
            Some(file) => std::path::PathBuf::from(file),
            None => match super::paths::user_dir() {
                Some(dir) => dir.join("harletty-mf.log"),
                None => return,
            },
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };
        let logger = Box::leak(Box::new(FileLogger {
            file: Mutex::new(file),
            level,
        }));
        if log::set_logger(logger).is_ok() {
            log::set_max_level(level);
        }
    });
}
