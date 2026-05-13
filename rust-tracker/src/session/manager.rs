use serde_json::json;

use crate::config::TrackerConfig;
use crate::models::activity::GitSummary;
use crate::models::event::Event;
use crate::processing::activity::ActivityGrouper;
use crate::processing::enrich;
use crate::storage::logger::Logger;

pub struct SessionManager {
    pub last_event: Option<Event>,
    pub grouper: ActivityGrouper,
    pub logger: Logger,
}

impl SessionManager {
    pub fn new(config: &TrackerConfig) -> Self {
        Self {
            last_event: None,
            grouper: ActivityGrouper::new(&config.tracking),
            logger: Logger::new(&config.storage),
        }
    }

    pub fn process_window_session(
        &mut self,
        app: String,
        title: String,
        workspace: i64,
        duration_sec: u64,
    ) {
        if duration_sec < 2 {
            return;
        }

        let is_same = self.last_event.as_ref().map_or(false, |evt| {
            evt.app == Some(app.clone())
                && evt.title == Some(title.clone())
                && evt.workspace == Some(workspace)
        });

        if is_same {
            let last = self.last_event.as_mut().unwrap();
            let prev = last.duration_sec.unwrap_or(0);
            last.duration_sec = Some(prev + duration_sec);
            return;
        }

        if let Some(prev) = self.last_event.take() {
            self.flush_event(prev);
        }

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

    fn flush_event(&mut self, evt: Event) {
        self.logger.log_event(
            &evt.event_type,
            &evt.source,
            evt.app.clone(),
            evt.title.clone(),
            evt.workspace,
            evt.duration_sec,
            json!({ "message": "Normalized session" }),
        );

        let enriched = enrich::enrich_event(&evt);
        self.grouper.push_enriched(&enriched);
    }

    pub fn push_terminal_workflow(&mut self, workflow: &str) {
        self.grouper.push_terminal_workflow(workflow);
    }

    pub fn push_git_summary(&mut self, summary: GitSummary) {
        self.grouper.push_git(summary);
    }

    pub fn emit_idle(&mut self, idle_sec: u64) {
        self.logger.log_event(
            "idle_session",
            "idle_tracker",
            None,
            None,
            None,
            Some(idle_sec),
            json!({ "idle": true }),
        );

        self.grouper.split_on_idle();
        self.flush_completed_groups();
    }

    pub fn flush(&mut self) {
        if let Some(evt) = self.last_event.take() {
            self.flush_event(evt);
        }
    }

    pub fn flush_completed_groups(&mut self) {
        let groups = self.grouper.drain_completed();
        for group in &groups {
            self.logger.log_normalized_session(group);
        }
    }

    pub fn finalize(&mut self) {
        self.flush();
        let groups = self.grouper.finalize_all();
        for group in &groups {
            self.logger.log_normalized_session(group);
        }
    }
}
