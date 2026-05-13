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
}
