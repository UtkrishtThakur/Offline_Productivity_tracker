use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::config::StorageConfig;
use crate::models::activity::ActivityGroup;
use crate::models::event::Event;

/// Persistence layer for raw events and normalized sessions.
/// Writes JSONL files into the configured session directory.
pub struct Logger {
    pub session_dir: PathBuf,
    pub events_path: PathBuf,
    pub normalized_path: PathBuf,
}

impl Logger {
    pub fn new(config: &StorageConfig) -> Self {
        let session_dir = PathBuf::from(&config.session_dir);
        let events_path = session_dir.join(&config.events_file);
        let normalized_path = session_dir.join(&config.normalized_file);

        Self {
            session_dir,
            events_path,
            normalized_path,
        }
    }

    pub fn log_event(
        &self,
        event_type: &str,
        source: &str,
        app: Option<String>,
        title: Option<String>,
        workspace: Option<i64>,
        duration_sec: Option<u64>,
        data: serde_json::Value,
    ) {
        fs::create_dir_all(&self.session_dir).unwrap();

        let event = Event {
            timestamp: chrono::Local::now().to_rfc3339(),
            event_type: event_type.to_string(),
            source: source.to_string(),
            app,
            title,
            workspace,
            duration_sec,
            data,
        };

        let json = serde_json::to_string(&event).expect("serialization failed");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .unwrap();

        writeln!(file, "{}", json).unwrap();
    }

    pub fn log_normalized_session(&self, group: &ActivityGroup) {
        fs::create_dir_all(&self.session_dir).unwrap();

        let json = serde_json::to_string(group).expect("serialization failed");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.normalized_path)
            .unwrap();

        writeln!(file, "{}", json).unwrap();
    }

    pub fn read_normalized_sessions(&self) -> Vec<ActivityGroup> {
        let contents = match fs::read_to_string(&self.normalized_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        contents
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// Read only sessions that start with the given date prefix (YYYY-MM-DD).
    /// Avoids loading the entire history when only one day is needed.
    pub fn read_normalized_sessions_for_date(&self, date: &str) -> Vec<ActivityGroup> {
        let contents = match fs::read_to_string(&self.normalized_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        contents
            .lines()
            .filter_map(|line| {
                let group: ActivityGroup = serde_json::from_str(line).ok()?;
                if group.start_time.starts_with(date) {
                    Some(group)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Scan the normalized file and return all unique dates (YYYY-MM-DD) found.
    /// Performs light string matching without full JSON deserialization.
    pub fn read_session_dates(&self) -> Vec<String> {
        let contents = match fs::read_to_string(&self.normalized_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut dates: Vec<String> = Vec::new();
        for line in contents.lines() {
            if let Some(date) = extract_date_prefix(line) {
                if !dates.contains(&date) {
                    dates.push(date);
                }
            }
        }
        dates
    }

    /// Read sessions matching any of the given dates.
    #[allow(dead_code)]
    pub fn read_sessions_for_dates(&self, dates: &[String]) -> Vec<ActivityGroup> {
        let contents = match fs::read_to_string(&self.normalized_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        contents
            .lines()
            .filter_map(|line| {
                let group: ActivityGroup = serde_json::from_str(line).ok()?;
                if dates.iter().any(|d| group.start_time.starts_with(d.as_str())) {
                    Some(group)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Extract YYYY-MM-DD from the start of a JSON string value field.
fn extract_date_prefix(line: &str) -> Option<String> {
    for prefix in &["\"start_time\":\"", "\"timestamp\":\""] {
        if let Some(pos) = line.find(prefix) {
            let start = pos + prefix.len();
            if start + 10 <= line.len() {
                let candidate = &line[start..start + 10];
                if candidate.chars().all(|c| c == '-' || c.is_ascii_digit()) {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}
