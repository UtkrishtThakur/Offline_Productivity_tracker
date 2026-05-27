use crate::config::AppContext;
use crate::storage::archiver::Archiver;
use crate::storage::logger::Logger;

use super::ai_queue::{AiRetryQueue, PendingAiJob};

pub struct DayManager {
    pub current_date: String,
}

impl DayManager {
    pub fn new() -> Self {
        Self {
            current_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        }
    }

    pub fn today() -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }

    pub fn date_changed(&self) -> bool {
        Self::today() != self.current_date
    }

    pub fn advance(&mut self) {
        self.current_date = Self::today();
    }
}

/// Startup recovery: detect unfinished previous days and finalize them.
///
/// Recovery flow:
///   1. Scan session dates (light string matching) to find days before today.
///   2. For each day without metadata: write deterministic summary, enqueue semantic.
///   3. NEVER blocks on AI generation — semantic jobs are deferred to runtime loop.
///   4. NEVER cleans up — cleanup only happens after AI completes in runtime loop.
pub fn startup_recovery(ctx: &AppContext, archiver: &Archiver, ai_enabled: bool, retry_queue: &mut AiRetryQueue) {
    let logger = Logger::new(&ctx.config.storage);
    let today = DayManager::today();

    let session_dates = logger.read_session_dates();
    let pending_dates: Vec<String> = session_dates
        .into_iter()
        .filter(|d| d.as_str() != today && !archiver.day_has_metadata(d))
        .collect();

    if pending_dates.is_empty() {
        return;
    }

    println!(
        "Startup recovery: found {} day(s) needing archival",
        pending_dates.len()
    );

    for date in &pending_dates {
        let day_groups = logger.read_normalized_sessions_for_date(date);
        if day_groups.is_empty() {
            continue;
        }

        match archiver.write_deterministic_summary(date, &day_groups) {
            Ok(_) => {
                println!("Startup recovery: deterministic summary written for {date}");
                if ai_enabled {
                    if archiver.day_has_metadata(date) {
                        if let Err(e) = archiver.mark_semantic_pending(date) {
                            eprintln!("Startup recovery: failed to mark semantic pending for {date}: {e}");
                        }
                    }
                    retry_queue.enqueue_if_missing(PendingAiJob::new(date));
                    println!("Startup recovery: queued semantic summary for {date}");
                } else {
                    if let Err(e) = archiver.mark_finalized(date) {
                        eprintln!("Startup recovery: failed to finalize {date}: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("Startup recovery: failed to write deterministic summary for {date}: {e}");
            }
        }
    }
}
