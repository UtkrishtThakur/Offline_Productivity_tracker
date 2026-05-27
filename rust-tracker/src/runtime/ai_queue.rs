use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAiJob {
    pub date: String,
    pub retry_count: u32,
    pub last_attempt_epoch: Option<u64>,
    #[serde(skip)]
    pub last_attempt: Option<Instant>,
}

impl PendingAiJob {
    pub fn new(date: &str) -> Self {
        Self {
            date: date.to_string(),
            retry_count: 0,
            last_attempt: None,
            last_attempt_epoch: None,
        }
    }

    fn update_attempt(&mut self) {
        let now = Instant::now();
        self.last_attempt = Some(now);
        self.last_attempt_epoch = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }
}

pub struct AiRetryQueue {
    jobs: Vec<PendingAiJob>,
    queue_path: PathBuf,
    max_retries: u32,
    retry_delay: Duration,
}

impl AiRetryQueue {
    pub fn new(
        session_dir: &str,
        max_retries: u32,
        retry_delay_sec: u64,
    ) -> Self {
        let queue_path = PathBuf::from(session_dir).join("ai_retry_queue.json");
        let mut queue = Self {
            jobs: Vec::new(),
            queue_path,
            max_retries,
            retry_delay: Duration::from_secs(retry_delay_sec),
        };
        queue.load();
        queue
    }

    fn queue_path(&self) -> &PathBuf {
        &self.queue_path
    }

    fn load(&mut self) {
        let contents = match fs::read_to_string(self.queue_path()) {
            Ok(c) => c,
            Err(_) => return,
        };
        self.jobs = match serde_json::from_str::<Vec<PendingAiJob>>(&contents) {
            Ok(mut jobs) => {
                for job in &mut jobs {
                    if let Some(epoch) = job.last_attempt_epoch {
                        let elapsed = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH + Duration::from_secs(epoch))
                            .unwrap_or_default();
                        let elapsed_ms = elapsed.as_millis() as u64;
                        if elapsed_ms > 0 {
                            let inst = Instant::now()
                                .checked_sub(Duration::from_millis(elapsed_ms))
                                .unwrap_or_else(Instant::now);
                            job.last_attempt = Some(inst);
                        }
                    }
                }
                jobs
            }
            Err(e) => {
                eprintln!("Warning: failed to parse AI retry queue: {e}");
                Vec::new()
            }
        };
    }

    fn save(&self) {
        if let Some(parent) = self.queue_path().parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&self.jobs) {
            let _ = fs::write(self.queue_path(), &json);
        }
    }

    /// Check if the queue already contains a job for the given date.
    pub fn contains(&self, date: &str) -> bool {
        self.jobs.iter().any(|j| j.date == date)
    }

    /// Enqueue a job only if no job exists for the same date.
    /// Returns true if the job was added, false if it already existed.
    pub fn enqueue_if_missing(&mut self, job: PendingAiJob) -> bool {
        if self.contains(&job.date) {
            return false;
        }
        self.jobs.push(job);
        self.save();
        true
    }

    /// Push a job unconditionally (may create duplicates).
    #[allow(dead_code)]
    pub fn push(&mut self, job: PendingAiJob) {
        self.jobs.push(job);
        self.save();
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Process retries with separate callbacks for success and exhaustion.
    ///
    /// `retry_fn`: called for each eligible job, returns Ok(()) on success.
    /// `exhausted_fn`: called when a job's retries are exhausted with (date, retry_count).
    pub fn process_retries<F, G>(&mut self, retry_fn: F, exhausted_fn: G)
    where
        F: Fn(&str) -> Result<(), String>,
        G: Fn(&str, u32),
    {
        if self.jobs.is_empty() {
            return;
        }

        let mut completed_indices: Vec<usize> = Vec::new();

        for (i, job) in self.jobs.iter_mut().enumerate() {
            let should_retry = match job.last_attempt {
                Some(t) => t.elapsed() >= self.retry_delay,
                None => true,
            };
            if !should_retry {
                continue;
            }

            job.update_attempt();

            match retry_fn(&job.date) {
                Ok(_) => {
                    println!("AI summary completed for {} (retry {})", job.date, job.retry_count);
                    completed_indices.push(i);
                }
                Err(e) => {
                    job.retry_count += 1;
                    if job.retry_count >= self.max_retries {
                        eprintln!(
                            "AI summary exhausted for {} after {} retries: {e}",
                            job.date, self.max_retries
                        );
                        exhausted_fn(&job.date, job.retry_count);
                        completed_indices.push(i);
                    } else {
                        eprintln!(
                            "AI summary retry {}/{} for {} failed: {e}",
                            job.retry_count, self.max_retries, job.date
                        );
                    }
                }
            }
        }

        for &i in completed_indices.iter().rev() {
            self.jobs.remove(i);
        }

        self.save();
    }
}
