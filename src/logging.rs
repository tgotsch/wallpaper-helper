use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

struct DualLogger {
    file: Mutex<File>,
}

impl Log for DualLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("[{}] [{}] {}", timestamp, record.level(), record.args());

        println!("{}", line);

        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

pub fn init() {
    let file = File::create("wallpaper_helper.log")
        .expect("Failed to create wallpaper_helper.log");

    let logger = DualLogger {
        file: Mutex::new(file),
    };

    log::set_boxed_logger(Box::new(logger))
        .expect("Failed to set logger");
    log::set_max_level(LevelFilter::Info);
}
