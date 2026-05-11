// storage/logger.rs
//
// Persistence layer for raw events and normalized sessions.
// Writes JSONL files into the active session directory.

use chrono::Local;

use crate::models::activity::ActivityGroup;
use crate::models::event::Event;

use std::fs::{self, OpenOptions};
use std::io::Write;

/// Base path for session storage.
fn session_dir() -> String {
    "../sessions/active_session".to_string()
}

/// Appends a raw Event as JSONL to `events.jsonl`.
pub fn log_event(
    event_type: &str,
    source: &str,
    app: Option<String>,
    title: Option<String>,
    workspace: Option<i64>,
    duration_sec: Option<u64>,
    data: serde_json::Value,
) {
    let dir = session_dir();
    fs::create_dir_all(&dir).unwrap();

    let path = format!("{}/events.jsonl", dir);

    let event = Event {
        timestamp: Local::now().to_rfc3339(),
        event_type: event_type.to_string(),
        source: source.to_string(),
        app,
        title,
        workspace,
        duration_sec,
        data,
    };

    let json = serde_json::to_string(&event)
        .expect("serialization failed");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();

    writeln!(file, "{}", json).unwrap();
}

/// Appends a normalized ActivityGroup as JSONL to
/// `normalized_sessions.jsonl`.
pub fn log_normalized_session(
    group: &ActivityGroup,
) {
    let dir = session_dir();
    fs::create_dir_all(&dir).unwrap();

    let path = format!(
        "{}/normalized_sessions.jsonl",
        dir,
    );

    let json = serde_json::to_string(group)
        .expect("serialization failed");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();

    writeln!(file, "{}", json).unwrap();
}

/// Reads all normalized ActivityGroups from `normalized_sessions.jsonl`.
pub fn read_normalized_sessions() -> Vec<ActivityGroup> {
    let dir = session_dir();
    let path = format!("{}/normalized_sessions.jsonl", dir);

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
