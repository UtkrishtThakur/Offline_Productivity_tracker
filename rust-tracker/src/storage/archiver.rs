use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{StorageConfig, TrackerConfig};
use crate::models::activity::ActivityGroup;
use crate::processing::daily;

/// Lifecycle states for a day's summary:
///
///   (no metadata) ──→ deterministic_complete ──→ semantic_pending ──→ semantic_complete ──→ finalized
///                          │                                                  │
///                          └── (AI disabled) ────────────────────────────────→ finalized
///                                                                   semantic_pending ──→ failed
///
/// - `pending`:                   no metadata written yet
/// - `deterministic_complete`:    deterministic.txt written, semantic pending
/// - `semantic_pending`:          AI job queued for retry
/// - `semantic_complete`:         semantic.txt written by AI analyzer
/// - `finalized`:                 all summaries written + logs cleaned up
/// - `failed`:                    retries exhausted, semantic failed permanently
pub const STATE_DETERMINISTIC_COMPLETE: &str = "deterministic_complete";
pub const STATE_SEMANTIC_PENDING: &str = "semantic_pending";
pub const STATE_SEMANTIC_COMPLETE: &str = "semantic_complete";
pub const STATE_FINALIZED: &str = "finalized";
pub const STATE_FAILED: &str = "failed";

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

    /// Check whether a day has any metadata (any lifecycle state).
    /// Used to avoid re-processing a day on restart.
    pub fn day_has_metadata(&self, date: &str) -> bool {
        let meta_path = self.day_dir(date).join("metadata.json");
        meta_path.exists()
    }

    /// Check whether a day is fully finalized.
    #[allow(dead_code)]
    pub fn day_is_finalized(&self, date: &str) -> bool {
        let meta_path = self.day_dir(date).join("metadata.json");
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<DayMetadata>(&content) {
                return meta.status == STATE_FINALIZED;
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

    // ── Lifecycle state machine ──────────────────────────────────────────

    /// Write deterministic.txt and create metadata with status `deterministic_complete`.
    pub fn write_deterministic_summary(
        &self,
        date: &str,
        groups: &[ActivityGroup],
    ) -> std::io::Result<DayMetadata> {
        self.write_deterministic(date, groups)?;
        let meta = self.build_metadata(
            date,
            groups,
            STATE_DETERMINISTIC_COMPLETE,
            None,
            None,
            0,
        );
        self.write_metadata(date, &meta)?;
        Ok(meta)
    }

    /// Transition metadata from `deterministic_complete` to `semantic_pending`.
    pub fn mark_semantic_pending(&self, date: &str) -> std::io::Result<()> {
        if let Ok(Some(mut meta)) = self.read_metadata(date) {
            meta.status = STATE_SEMANTIC_PENDING.to_string();
            self.write_metadata(date, &meta)
        } else {
            Ok(())
        }
    }

    /// Transition metadata to `semantic_complete` after successful AI generation.
    pub fn mark_semantic_complete(&self, date: &str) -> std::io::Result<()> {
        if let Ok(Some(mut meta)) = self.read_metadata(date) {
            meta.status = STATE_SEMANTIC_COMPLETE.to_string();
            meta.semantic_summary = Some("semantic.txt".to_string());
            self.write_metadata(date, &meta)
        } else {
            Ok(())
        }
    }

    /// Transition metadata to `finalized` after all summaries written and logs cleaned.
    pub fn mark_finalized(&self, date: &str) -> std::io::Result<()> {
        if let Ok(Some(mut meta)) = self.read_metadata(date) {
            meta.status = STATE_FINALIZED.to_string();
            meta.finalized_at = Some(chrono::Local::now().to_rfc3339());
            self.write_metadata(date, &meta)
        } else {
            Ok(())
        }
    }

    /// Transition metadata to `failed` after retries exhausted.
    pub fn mark_failed(&self, date: &str, error: &str) -> std::io::Result<()> {
        if let Ok(Some(mut meta)) = self.read_metadata(date) {
            meta.status = STATE_FAILED.to_string();
            meta.error = Some(error.to_string());
            self.write_metadata(date, &meta)
        } else {
            Ok(())
        }
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
            finalized_at: None,
        }
    }

    // ── Cleanup ──────────────────────────────────────────────────────────

    /// Rewrite JSONL files to remove entries matching a given date.
    /// Must only be called after ALL summaries are complete and persisted.
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
fn filter_jsonl_file(path: &std::path::Path, keep: impl Fn(&str) -> bool) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, BufWriter};

    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let tmp_file = fs::File::create(&tmp_path)?;
        let mut writer = BufWriter::new(tmp_file);

        for line in reader.lines() {
            let line = line?;
            if keep(&line) {
                writeln!(writer, "{}", line)?;
            }
        }
        writer.flush()?;
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
    fn test_extract_date_filtering() {
        assert!(daily::extract_date("2026-05-14T09:00:00+00:00") == Some("2026-05-14"));
    }

    #[test]
    fn test_state_constants() {
        assert_eq!(STATE_DETERMINISTIC_COMPLETE, "deterministic_complete");
        assert_eq!(STATE_SEMANTIC_PENDING, "semantic_pending");
        assert_eq!(STATE_SEMANTIC_COMPLETE, "semantic_complete");
        assert_eq!(STATE_FINALIZED, "finalized");
        assert_eq!(STATE_FAILED, "failed");
    }
}
