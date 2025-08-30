use std::fmt::Display;

use chrono::Local;

enum Level {
    Info(&'static str),
    Error(&'static str),
    Debug(&'static str),
}
impl Level {
    pub fn info() -> Self {
        Level::Info("INFO")
    }
    pub fn debug() -> Self {
        Level::Debug("DEBUG")
    }
    pub fn error() -> Self {
        Level::Error("ERROR")
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Info(s) => s,
            Level::Debug(s) => s,
            Level::Error(s) => s,
        }
    }
}

pub struct Logger {
    service: &'static str,
    compact: bool,
}
impl Logger {
    pub const fn verbose(service: &'static str) -> Self {
        Self {
            service,
            compact: false,
        }
    }
    pub const fn compact(service: &'static str) -> Self {
        Self {
            service,
            compact: true,
        }
    }

    fn create_message(&self, level: Level, msg: impl Display) -> String {
        if self.compact {
            let prefix = match level {
                // We need to match as templates because of the value inside
                Level::Debug(_) => "d",
                Level::Error(_) => "e",
                _ => "",
            };

            format!("{}[{}] {}", prefix, self.service, msg)
        } else {
            format!(
                "[{}] {} {}: {}",
                self.service,
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                level.as_str(),
                msg
            )
        }
    }
    pub fn info(&self, msg: impl Display) {
        println!("{}", self.create_message(Level::info(), msg));
    }
    pub fn debug(&self, msg: impl Display) {
        println!("{}", self.create_message(Level::debug(), msg));
    }
    pub fn error(&self, error: impl Display) {
        println!("{}", self.create_message(Level::error(), error));
    }
}
