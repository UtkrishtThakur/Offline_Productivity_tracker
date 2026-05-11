use serde_json::json;

use crate::models::event::Event;
use crate::storage::logger::log_event;

pub struct SessionManager {

    pub last_event: Option<Event>,
}

impl SessionManager {

    pub const IDLE_THRESHOLD_SEC: u64 = 120;

    pub fn new() -> Self {

        Self {
            last_event: None,
        }
    }

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

        // Merge same consecutive sessions
        if let Some(ref mut last_evt) =
            self.last_event
        {

            if last_evt.app == Some(app.clone())
                && last_evt.title == Some(title.clone())
                && last_evt.workspace == Some(workspace)
            {

                let current_duration =
                    last_evt.duration_sec.unwrap_or(0);

                last_evt.duration_sec =
                    Some(current_duration + duration_sec);

                return;
            }

            // Flush previous event
            log_event(
                &last_evt.event_type,
                &last_evt.source,
                last_evt.app.clone(),
                last_evt.title.clone(),
                last_evt.workspace,
                last_evt.duration_sec,
                json!({
                    "message": "Normalized session"
                }),
            );
        }

        // Create new session
        self.last_event = Some(Event {

            timestamp:
                chrono::Local::now()
                    .to_rfc3339(),

            event_type:
                "window_session".to_string(),

            source:
                "window_tracker".to_string(),

            app:
                Some(app),

            title:
                Some(title),

            workspace:
                Some(workspace),

            duration_sec:
                Some(duration_sec),

            data: json!({
                "normalized": true
            }),
        });
    }

    pub fn flush(&mut self) {

        if let Some(ref evt) =
            self.last_event
        {

            log_event(
                &evt.event_type,
                &evt.source,
                evt.app.clone(),
                evt.title.clone(),
                evt.workspace,
                evt.duration_sec,
                json!({
                    "message": "Flushed session"
                }),
            );
        }

        self.last_event = None;
    }

    pub fn emit_idle(
        &self,
        idle_sec: u64,
    ) {

        log_event(
            "idle_session",
            "idle_tracker",
            None,
            None,
            None,
            Some(idle_sec),
            json!({
                "idle": true
            }),
        );
    }
}