// processing/activity.rs
//
// Activity grouping layer.
// Merges related enriched events into ActivityGroup structs
// grouped by (project, app) with time-adjacency rules.

use chrono::Local;

use crate::models::activity::{ActivityGroup, GitSummary};
use crate::models::enriched::EnrichedEvent;

/// Maximum gap (in seconds) between events before
/// a new activity group is started.
const ADJACENCY_WINDOW_SEC: u64 = 300; // 5 minutes

/// Minimum duration (in seconds) for a window event
/// to be considered meaningful.
const MIN_MEANINGFUL_SEC: u64 = 2;

/// Key used for grouping activities.
#[derive(Debug, Clone, PartialEq)]
struct GroupKey {
    project: Option<String>,
    app: String,
}

/// Stateful grouper that accumulates events into ActivityGroups.
pub struct ActivityGrouper {

    /// The currently-building activity group
    current_key: Option<GroupKey>,
    current_start: String,
    current_duration: u64,
    current_files: Vec<String>,
    current_languages: Vec<String>,
    current_terminal_workflows: Vec<String>,
    current_git: Option<GitSummary>,

    /// Completed groups ready for flushing
    completed: Vec<ActivityGroup>,
}

impl ActivityGrouper {

    pub fn new() -> Self {
        Self {
            current_key: None,
            current_start: String::new(),
            current_duration: 0,
            current_files: Vec::new(),
            current_languages: Vec::new(),
            current_terminal_workflows: Vec::new(),
            current_git: None,
            completed: Vec::new(),
        }
    }

    /// Push an enriched window event into the grouper.
    pub fn push_enriched(
        &mut self,
        enriched: &EnrichedEvent,
    ) {
        let duration = enriched.event.duration_sec
            .unwrap_or(0);

        // Filter noise
        if duration < MIN_MEANINGFUL_SEC {
            return;
        }

        let key = GroupKey {
            project: enriched.project.clone(),
            app: enriched.normalized_app.clone(),
        };

        // Check if this belongs to the current group
        if let Some(ref current) = self.current_key {
            if *current == key
                && duration < ADJACENCY_WINDOW_SEC
            {
                // Extend current group
                self.current_duration += duration;

                if let Some(ref f) = enriched.file {
                    if !self.current_files.contains(f) {
                        self.current_files.push(f.clone());
                    }
                }

                if let Some(ref lang) = enriched.language {
                    if !self.current_languages.contains(lang) {
                        self.current_languages.push(lang.clone());
                    }
                }

                return;
            }

            // Different key — finalize current group
            self.finalize_current();
        }

        // Start new group
        self.current_key = Some(key);
        self.current_start = enriched.event.timestamp.clone();
        self.current_duration = duration;

        self.current_files = enriched.file
            .as_ref()
            .map(|f| vec![f.clone()])
            .unwrap_or_default();

        self.current_languages = enriched.language
            .as_ref()
            .map(|l| vec![l.clone()])
            .unwrap_or_default();
    }

    /// Push a terminal workflow label.
    pub fn push_terminal_workflow(
        &mut self,
        workflow: &str,
    ) {
        if !self.current_terminal_workflows.contains(
            &workflow.to_string()
        ) {
            self.current_terminal_workflows
                .push(workflow.to_string());
        }
    }

    /// Push git summary data.
    pub fn push_git(
        &mut self,
        summary: GitSummary,
    ) {
        self.current_git = Some(summary);
    }

    /// Split on idle — finalizes the current group
    /// so the next event starts a fresh one.
    pub fn split_on_idle(&mut self) {
        self.finalize_current();
    }

    /// Finalize the current in-progress group and
    /// move it to the completed list.
    fn finalize_current(&mut self) {

        if let Some(ref key) = self.current_key {

            if self.current_duration == 0 {
                // Nothing meaningful accumulated
                self.reset_current();
                return;
            }

            let group = ActivityGroup {
                start_time: self.current_start.clone(),
                end_time: Local::now().to_rfc3339(),
                project: key.project.clone(),
                app: key.app.clone(),
                total_duration_sec: self.current_duration,
                files_touched: self.current_files.clone(),
                languages: self.current_languages.clone(),
                terminal_workflows:
                    self.current_terminal_workflows.clone(),
                git_summary: self.current_git.clone(),
            };

            self.completed.push(group);
        }

        self.reset_current();
    }

    /// Reset current group state.
    fn reset_current(&mut self) {
        self.current_key = None;
        self.current_start.clear();
        self.current_duration = 0;
        self.current_files.clear();
        self.current_languages.clear();
        self.current_terminal_workflows.clear();
        self.current_git = None;
    }

    /// Drain all completed groups.
    /// Call this to get finalized groups for persistence.
    pub fn drain_completed(
        &mut self,
    ) -> Vec<ActivityGroup> {
        std::mem::take(&mut self.completed)
    }

    /// Finalize everything (current + completed) and return all groups.
    pub fn finalize_all(
        &mut self,
    ) -> Vec<ActivityGroup> {
        self.finalize_current();
        std::mem::take(&mut self.completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::event::Event;
    use serde_json::json;

    fn make_enriched(
        app: &str,
        project: Option<&str>,
        file: Option<&str>,
        language: Option<&str>,
        duration: u64,
    ) -> EnrichedEvent {
        EnrichedEvent {
            event: Event {
                timestamp: Local::now().to_rfc3339(),
                event_type: "window_session".to_string(),
                source: "window_tracker".to_string(),
                app: Some(app.to_string()),
                title: Some("test".to_string()),
                workspace: Some(0),
                duration_sec: Some(duration),
                data: json!({}),
            },
            project: project.map(|s| s.to_string()),
            file: file.map(|s| s.to_string()),
            language: language.map(|s| s.to_string()),
            normalized_app: app.to_lowercase(),
            repo: None,
            branch: None,
        }
    }

    #[test]
    fn test_merge_same_project() {
        let mut grouper = ActivityGrouper::new();

        let e1 = make_enriched(
            "antigravity", Some("tracker"),
            Some("main.rs"), Some("rust"), 60,
        );

        let e2 = make_enriched(
            "antigravity", Some("tracker"),
            Some("manager.rs"), Some("rust"), 120,
        );

        grouper.push_enriched(&e1);
        grouper.push_enriched(&e2);

        let groups = grouper.finalize_all();

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].total_duration_sec, 180);
        assert_eq!(groups[0].files_touched.len(), 2);
    }

    #[test]
    fn test_split_different_project() {
        let mut grouper = ActivityGrouper::new();

        let e1 = make_enriched(
            "antigravity", Some("tracker"),
            Some("main.rs"), Some("rust"), 60,
        );

        let e2 = make_enriched(
            "antigravity", Some("other-project"),
            Some("app.py"), Some("python"), 30,
        );

        grouper.push_enriched(&e1);
        grouper.push_enriched(&e2);

        let groups = grouper.finalize_all();

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_idle_split() {
        let mut grouper = ActivityGrouper::new();

        let e1 = make_enriched(
            "antigravity", Some("tracker"),
            Some("main.rs"), Some("rust"), 60,
        );

        grouper.push_enriched(&e1);
        grouper.split_on_idle();

        let e2 = make_enriched(
            "antigravity", Some("tracker"),
            Some("main.rs"), Some("rust"), 30,
        );

        grouper.push_enriched(&e2);

        let groups = grouper.finalize_all();

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_noise_filtered() {
        let mut grouper = ActivityGrouper::new();

        // 1-second event should be filtered
        let e1 = make_enriched(
            "antigravity", Some("tracker"),
            Some("main.rs"), Some("rust"), 1,
        );

        grouper.push_enriched(&e1);

        let groups = grouper.finalize_all();
        assert!(groups.is_empty());
    }
}
