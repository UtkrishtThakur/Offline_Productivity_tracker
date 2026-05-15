use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{StorageConfig, TrackerConfig};
use crate::models::activity::ActivityGroup;
use crate::processing::daily;

/// Metadata for a single day's summary directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayMetadata {
    pub date: String,
    pub status: String,
    pub total_groups: usize,
    pub total_duration_sec: u64,
    pub projects: Vec<String>,
    pub languages: Vec<String>,
    pub files: Vec<String>,
    pub apps: Vec<String>,
    pub deterministic_summary: String,
    pub semantic_summary: Option<String>,
    pub error: Option<String>,
    pub retry_count: u32,
    pub finalized_at: Option<String>,
}

/// Handles daily summary archival, metadata tracking, and log cleanup.
pub struct Archiver {
    pub summaries_dir: PathBuf,
    pub auto_cleanup: bool,
}

impl Archiver {
    pub fn new(config: &TrackerConfig) -> Self {
        Self {
            summaries_dir: PathBuf::from(&config.storage.summaries_dir),
            auto_cleanup: config.summary.auto_cleanup,
        }
    }

    /// Path to the summary directory for a given date.
    pub fn day_dir(&self, date: &str) -> PathBuf {
        self.summaries_dir.join(date)
    }

    /// Ensure the day directory exists and return its path.
    pub fn ensure_day_dir(&self, date: &str) -> std::io::Result<PathBuf> {
        let dir = self.day_dir(date);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Check whether a day already has a finalized summary.
    pub fn day_has_summary(&self, date: &str) -> bool {
        let meta_path = self.day_dir(date).join("metadata.json");
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<DayMetadata>(&content) {
                return meta.status == "finalized";
            }
        }
        false
    }

    /// Read existing metadata for a date, if any.
    pub fn read_metadata(&self, date: &str) -> std::io::Result<Option<DayMetadata>> {
        let path = self.day_dir(date).join("metadata.json");
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content)
                .map(Some)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write metadata.json for a date.
    pub fn write_metadata(&self, date: &str, metadata: &DayMetadata) -> std::io::Result<()> {
        let dir = self.ensure_day_dir(date)?;
        let json = serde_json::to_string_pretty(metadata)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(dir.join("metadata.json"), &json)
    }

    /// Write deterministic.txt for a date from the given groups.
    pub fn write_deterministic(&self, date: &str, groups: &[ActivityGroup]) -> std::io::Result<()> {
        let dir = self.ensure_day_dir(date)?;
        let content = daily::format_timeline_summary(groups, date);
        fs::write(dir.join("deterministic.txt"), &content)
    }

    /// Build metadata from a set of groups for a given day.
    pub fn build_metadata(
        &self,
        date: &str,
        groups: &[ActivityGroup],
        status: &str,
        semantic_path: Option<String>,
        error: Option<String>,
        retry_count: u32,
    ) -> DayMetadata {
        let total_sec: u64 = groups.iter().map(|g| g.total_duration_sec).sum();

        let mut projects: BTreeSet<String> = BTreeSet::new();
        let mut languages: BTreeSet<String> = BTreeSet::new();
        let mut files: BTreeSet<String> = BTreeSet::new();
        let mut apps: BTreeSet<String> = BTreeSet::new();

        for g in groups {
            if let Some(ref p) = g.project {
                projects.insert(p.clone());
            }
            for lang in &g.languages {
                languages.insert(lang.clone());
            }
            for f in &g.files_touched {
                files.insert(f.clone());
            }
            apps.insert(g.app.clone());
        }

        DayMetadata {
            date: date.to_string(),
            status: status.to_string(),
            total_groups: groups.len(),
            total_duration_sec: total_sec,
            projects: projects.into_iter().collect(),
            languages: languages.into_iter().collect(),
            files: files.into_iter().collect(),
            apps: apps.into_iter().collect(),
            deterministic_summary: "deterministic.txt".to_string(),
            semantic_summary: semantic_path,
            error,
            retry_count,
            finalized_at: Some(chrono::Local::now().to_rfc3339()),
        }
    }

    /// Finalize a day: write deterministic.txt, metadata.json, then optionally clean up logs.
    pub fn finalize_day(
        &self,
        date: &str,
        groups: &[ActivityGroup],
        storage: &StorageConfig,
    ) -> std::io::Result<DayMetadata> {
        self.write_deterministic(date, groups)?;

        let meta = self.build_metadata(date, groups, "finalized", None, None, 0);
        self.write_metadata(date, &meta)?;

        if self.auto_cleanup {
            self.cleanup_day_logs(date, storage)?;
        }

        Ok(meta)
    }

    /// Rewrite JSONL files to remove entries matching a given date.
    pub fn cleanup_day_logs(&self, date: &str, storage: &StorageConfig) -> std::io::Result<()> {
        let session_dir = PathBuf::from(&storage.session_dir);

        let events_path = session_dir.join(&storage.events_file);
        let normalized_path = session_dir.join(&storage.normalized_file);

        if events_path.exists() {
            filter_jsonl_file(&events_path, |line| !line_matches_date(line, date))?;
        }
        if normalized_path.exists() {
            filter_jsonl_file(&normalized_path, |line| {
                !line_matches_date(line, date)
            })?;
        }

        Ok(())
    }

    /// Archive any unarchived previous days from the current set of groups.
    /// Returns the groups that belong to today.
    pub fn archive_pending_days<'a>(
        &self,
        groups: &'a [ActivityGroup],
        today: &str,
        storage: &StorageConfig,
    ) -> std::io::Result<Vec<&'a ActivityGroup>> {
        let mut by_date: BTreeSet<String> = BTreeSet::new();
        for g in groups {
            if let Some(d) = daily::extract_date(&g.start_time) {
                if d != today {
                    by_date.insert(d.to_string());
                }
            }
        }

        for date in &by_date {
            if self.day_has_summary(date) {
                continue;
            }
            let day_groups: Vec<ActivityGroup> = groups
                .iter()
                .filter(|g| {
                    daily::extract_date(&g.start_time) == Some(date.as_str())
                })
                .cloned()
                .collect();

            if !day_groups.is_empty() {
                self.finalize_day(date, &day_groups, storage)?;
            }
        }

        Ok(groups
            .iter()
            .filter(|g| daily::extract_date(&g.start_time) == Some(today))
            .collect())
    }
}

/// Check if a JSONL line contains a timestamp or start_time matching the date.
fn line_matches_date(line: &str, date: &str) -> bool {
    if let Ok(val) = serde_json::from_str::<Value>(line) {
        let date_prefix = &format!("\"{}", date);
        if !line.contains(date_prefix) {
            return false;
        }
        if let Some(ts) = val.get("timestamp").and_then(|v| v.as_str()) {
            if ts.starts_with(date) {
                return true;
            }
        }
        if let Some(ts) = val.get("start_time").and_then(|v| v.as_str()) {
            if ts.starts_with(date) {
                return true;
            }
        }
    }
    false
}

/// Read a JSONL file, filter lines, and atomically rewrite it via a temp file.
fn filter_jsonl_file(path: &Path, keep: impl Fn(&str) -> bool) -> std::io::Result<()> {
    let content = fs::read_to_string(path)?;
    let filtered: Vec<&str> = content.lines().filter(|l| keep(l)).collect();

    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let mut tmp = fs::File::create(&tmp_path)?;
        for line in &filtered {
            writeln!(tmp, "{}", line)?;
        }
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::activity::ActivityGroup;

    fn make_group(ts: &str) -> ActivityGroup {
        ActivityGroup {
            start_time: ts.to_string(),
            end_time: ts.to_string(),
            project: None,
            app: "test".to_string(),
            total_duration_sec: 60,
            files_touched: vec![],
            languages: vec![],
            terminal_workflows: vec![],
            git_summary: None,
        }
    }

    #[test]
    fn test_line_matches_date_events() {
        let line = r#"{"timestamp":"2026-05-14T09:00:00+00:00","event_type":"test"}"#;
        assert!(line_matches_date(line, "2026-05-14"));
        assert!(!line_matches_date(line, "2026-05-15"));
    }

    #[test]
    fn test_line_matches_date_groups() {
        let line = r#"{"start_time":"2026-05-14T09:00:00+00:00","app":"test"}"#;
        assert!(line_matches_date(line, "2026-05-14"));
        assert!(!line_matches_date(line, "2026-05-13"));
    }

    #[test]
    fn test_line_matches_date_no_match() {
        let line = r#"{"some_field":"hello"}"#;
        assert!(!line_matches_date(line, "2026-05-14"));
    }

    #[test]
    fn test_archive_pending_days_no_previous() {
        let config = TrackerConfig::default();
        let archiver = Archiver::new(&config);
        let groups = vec![make_group("2026-05-14T09:00:00+00:00")];
        let result = archiver.archive_pending_days(&groups, "2026-05-14", &config.storage);
        assert!(result.is_ok());
        let today_groups = result.unwrap();
        assert_eq!(today_groups.len(), 1);
    }

    #[test]
    fn test_extract_date_filtering() {
        assert!(daily::extract_date("2026-05-14T09:00:00+00:00") == Some("2026-05-14"));
    }
}
