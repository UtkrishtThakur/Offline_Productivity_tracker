// session/manager.rs
//
// Session management with integrated activity grouping.
// Owns an ActivityGrouper, enriches raw events before grouping,
// and flushes completed groups to normalized_sessions.jsonl.

use serde_json::json;

use crate::models::activity::GitSummary;
use crate::models::event::Event;
use crate::processing::activity::ActivityGrouper;
use crate::processing::enrich;
use crate::storage::logger::{log_event, log_normalized_session};

pub struct SessionManager {

    pub last_event: Option<Event>,

    pub grouper: ActivityGrouper,
}

impl SessionManager {

    pub const IDLE_THRESHOLD_SEC: u64 = 120;

    pub fn new() -> Self {
        Self {
            last_event: None,
            grouper: ActivityGrouper::new(),
        }
    }

    /// Process a window focus change.
    /// Merges consecutive identical sessions, enriches the event,
    /// and pushes it into the activity grouper.
    pub fn process_window_session(
        &mut self,
        app: String,
        title: String,
        workspace: i64,
        duration_sec: u64,
    ) {
        // Ignore tiny switches
        if duration_sec < 2 {
            return;
        }

        // Check if this matches the current session
        let is_same = self.last_event.as_ref().map_or(
            false,
            |evt| {
                evt.app == Some(app.clone())
                    && evt.title == Some(title.clone())
                    && evt.workspace == Some(workspace)
            },
        );

        if is_same {
            // Merge: extend duration of existing session
            let last = self.last_event.as_mut().unwrap();
            let prev = last.duration_sec.unwrap_or(0);
            last.duration_sec = Some(prev + duration_sec);
            return;
        }

        // Different session — flush previous if any
        if let Some(prev) = self.last_event.take() {
            self.flush_event(prev);
        }

        // Create new tracked session
        self.last_event = Some(Event {
            timestamp: chrono::Local::now().to_rfc3339(),
            event_type: "window_session".to_string(),
            source: "window_tracker".to_string(),
            app: Some(app),
            title: Some(title),
            workspace: Some(workspace),
            duration_sec: Some(duration_sec),
            data: json!({ "normalized": true }),
        });
    }

    /// Log a raw event and push its enriched version
    /// into the activity grouper.
    fn flush_event(&mut self, evt: Event) {

        // 1. Log the raw event
        log_event(
            &evt.event_type,
            &evt.source,
            evt.app.clone(),
            evt.title.clone(),
            evt.workspace,
            evt.duration_sec,
            json!({ "message": "Normalized session" }),
        );

        // 2. Enrich and push to grouper
        let enriched = enrich::enrich_event(&evt);
        self.grouper.push_enriched(&enriched);
    }

    /// Push a terminal workflow label into the current
    /// activity group.
    pub fn push_terminal_workflow(
        &mut self,
        workflow: &str,
    ) {
        self.grouper.push_terminal_workflow(workflow);
    }

    /// Push git summary into the current activity group.
    pub fn push_git_summary(
        &mut self,
        summary: GitSummary,
    ) {
        self.grouper.push_git(summary);
    }

    /// Handle idle event.
    /// Splits the current activity group and flushes
    /// completed groups to storage.
    pub fn emit_idle(&mut self, idle_sec: u64) {

        log_event(
            "idle_session",
            "idle_tracker",
            None,
            None,
            None,
            Some(idle_sec),
            json!({ "idle": true }),
        );

        // Idle boundary splits activity groups
        self.grouper.split_on_idle();
        self.flush_completed_groups();
    }

    /// Flush the current in-progress event (if any).
    pub fn flush(&mut self) {

        if let Some(evt) = self.last_event.take() {
            self.flush_event(evt);
        }
    }

    /// Flush all completed activity groups to
    /// normalized_sessions.jsonl.
    pub fn flush_completed_groups(&mut self) {

        let groups = self.grouper.drain_completed();

        for group in &groups {
            log_normalized_session(group);
        }
    }

    /// Finalize everything — flush pending event,
    /// finalize all groups, and persist.
    pub fn finalize(&mut self) {

        self.flush();

        let groups = self.grouper.finalize_all();

        for group in &groups {
            log_normalized_session(group);
        }
    }
}