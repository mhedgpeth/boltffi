use boltffi::*;

/// Task priority with explicit integer discriminants.
///
/// The `#[repr(i32)]` means these values are stable across
/// versions and safe to persist or send over the network.
#[data]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Priority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

#[export]
pub fn echo_priority(p: Priority) -> Priority {
    p
}

#[export]
pub fn priority_label(p: Priority) -> String {
    match p {
        Priority::Low => "low".to_string(),
        Priority::Medium => "medium".to_string(),
        Priority::High => "high".to_string(),
        Priority::Critical => "critical".to_string(),
    }
}

#[export]
pub fn is_high_priority(p: Priority) -> bool {
    matches!(p, Priority::High | Priority::Critical)
}

#[data]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

#[export]
pub fn echo_log_level(level: LogLevel) -> LogLevel {
    level
}

#[export]
pub fn should_log(level: LogLevel, min_level: LogLevel) -> bool {
    (level as u8) >= (min_level as u8)
}

#[export]
pub fn echo_vec_log_level(levels: Vec<LogLevel>) -> Vec<LogLevel> {
    levels
}
