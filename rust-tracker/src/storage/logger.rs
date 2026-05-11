// src/storage/logger.rs
use chrono::Local;
use crate::models::event::Event;
use std::fs::{self, OpenOptions};
use std::io::Write;

/// Appends an Event as JSONL to `sessions/active_session/events.jsonl`.
pub fn log_event(
    event_type: &str,
    source: &str,
    app: Option<String>,
    title: Option<String>,
    workspace: Option<i64>,
    duration_sec: Option<u64>,
    data: serde_json::Value,
) {
    let session_path = format!("../sessions/{}", "active_session");
    fs::create_dir_all(&session_path).unwrap();
    let log_file_path = format!("{}/events.jsonl", session_path);

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
    let json = serde_json::to_string(&event).expect("serialization failed");
    let mut file = OpenOptions::new()
        .create(true).append(true).open(log_file_path).unwrap();
    writeln!(file, "{}", json).unwrap();
}
