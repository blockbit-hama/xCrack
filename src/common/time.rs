use chrono::Utc;

/// Get current timestamp
pub fn current_timestamp() -> u64 {
    Utc::now().timestamp() as u64
}

/// Calculate time difference in seconds
pub fn time_diff(timestamp1: u64, timestamp2: u64) -> u64 {
    if timestamp1 > timestamp2 {
        timestamp1 - timestamp2
    } else {
        timestamp2 - timestamp1
    }
} 