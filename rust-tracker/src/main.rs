mod cli;
mod collector;
mod config;
mod models;
mod processing;
mod session;
mod storage;
mod utils;
mod ai_validation;

use std::time::Instant;

use clap::Parser;

use cli::commands::{Cli, Commands};

use collector::linux::git::get_git_activity;
use collector::linux::idle::get_idle_ms;
use collector::linux::terminal::get_latest_command;
use collector::linux::window::get_active_window;

use processing::git::build_git_summary;
use processing::terminal::classify_command;

use serde_json::json;

use session::manager::SessionManager;

use std::thread;
use std::time::Duration;

use processing::summary::format_report;
use storage::archiver::Archiver;
use storage::logger::Logger;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Pending AI summary job — drives the retry queue
// ---------------------------------------------------------------------------

struct PendingAiJob {
    date: String,
    retry_count: u32,
    last_attempt: Option<Instant>,
}

impl PendingAiJob {
    fn new(date: &str) -> Self {
        Self {
            date: date.to_string(),
            retry_count: 0,
            last_attempt: None,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let ctx = config::AppContext::load(cli.config.as_deref()).unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    // One-off logger for system events outside the tracking loop
    let sys_logger = Logger::new(&ctx.config.storage);

    match cli.command {
        Commands::Start => {
            sys_logger.log_event(
                "tracker_started",
                "system",
                None, None, None, None,
                json!({ "message": "Tracking started" }),
            );
            println!("Tracker started. Press Ctrl+C to stop.");

            // Archive any unarchived days from previous runs
            let archiver = Archiver::new(&ctx.config);
            let existing = sys_logger.read_normalized_sessions();
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if let Err(e) = archiver.archive_pending_days(&existing, &today, &ctx.config.storage) {
                eprintln!("Warning: failed to archive pending days: {e}");
            }

            run_tracking_loop(&ctx);
        }

        Commands::Pause => {
            sys_logger.log_event(
                "tracker_paused",
                "system", None, None, None, None,
                json!({ "message": "Tracking paused" }),
            );
            println!("Tracker paused");
        }

        Commands::Resume => {
            sys_logger.log_event(
                "tracker_resumed",
                "system", None, None, None, None,
                json!({ "message": "Tracking resumed" }),
            );
            println!("Tracker resumed");
        }

        Commands::Stop => {
            sys_logger.log_event(
                "tracker_stopped",
                "system", None, None, None, None,
                json!({ "message": "Tracking stopped" }),
            );
            println!("Tracker stopped");
        }

        Commands::Report => {
            let sessions = sys_logger.read_normalized_sessions();
            println!("{}", format_report(&sessions));
            if ctx.config.ai_analyzer.enabled {
                println!("AI analysis is enabled. Run 'tracker report-ai' for an AI summary.");
            }
        }

        Commands::ReportAi => {
            if !ctx.config.ai_analyzer.enabled {
                eprintln!(
                    "AI analyzer is disabled. Set ai_analyzer.enabled = true in tracker.toml \
                     or export TRACKER_AI_ENABLED=true"
                );
                std::process::exit(1);
            }
            if let Err(e) = crate::ai_validation::validate_ai_capabilities(&ctx.config.ai_analyzer) {
                eprintln!("AI validation failed: {}", e);
                std::process::exit(1);
            }
            match invoke_ai_analyzer(&ctx, None) {
                Ok(output) => println!("{output}"),
                Err(e) => {
                    eprintln!("AI analyzer failed: {e}");
                    eprintln!(
                        "Make sure py-analyzer is set up: \
                         cd py-analyzer && pip install -r requirements.txt"
                    );
                    std::process::exit(1);
                }
            }
        }

        Commands::InitConfig => {
            let path = std::path::Path::new("tracker.toml");
            config::AppContext::write_default(path).unwrap_or_else(|e| {
                eprintln!("Failed to write config: {e}");
                std::process::exit(1);
            });
            println!("Default config written to ./tracker.toml");
            println!("Edit it to customize behavior, then run: tracker --config tracker.toml Start");
        }
    }
}

/// Invoke the Python AI analyzer, optionally for a specific date.
/// When `date` is Some, writes to summaries/{date}/ directory.
/// When `date` is None, uses the legacy output/ directory.
fn invoke_ai_analyzer(ctx: &config::AppContext, date: Option<&str>) -> Result<String, String> {
    let analyzer_dir = std::path::Path::new("py-analyzer");
    let analyzer_script = analyzer_dir.join("analyzer.py");

    if !analyzer_script.exists() {
        return Err(format!(
            "py-analyzer/analyzer.py not found (cwd: {})",
            std::env::current_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default()
        ));
    }

    let c = &ctx.config.ai_analyzer;
    let mut cmd = std::process::Command::new("python3");
    cmd.arg(&analyzer_script)
        .env("TRACKER_AI_ENABLED", "true")
        .env("TRACKER_AI_OLLAMA_HOST", &c.ollama_host)
        .env("TRACKER_AI_MODEL", &c.model)
        .env("TRACKER_SESSION_DIR", &ctx.config.storage.session_dir)
        .env("TRACKER_NORMALIZED_FILE", &ctx.config.storage.normalized_file)
        .env("TRACKER_AI_OUTPUT_DIR", &c.output_dir);

    // When called from day-rotation, forward summaries path for semantic.txt
    if let Some(d) = date {
        cmd.env("TRACKER_SUMMARIES_DIR", &ctx.config.storage.summaries_dir);
        cmd.arg("--date");
        cmd.arg(d);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to launch python3: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AI analyzer exited with error:\n{stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------------------------------------------------------------------------
// Tracking loop with day rotation and AI retry queue
// ---------------------------------------------------------------------------

fn run_tracking_loop(ctx: &config::AppContext) {
    let mut manager = SessionManager::new(&ctx.config);
    let archiver = Archiver::new(&ctx.config);
    let retry_delay = Duration::from_secs(ctx.config.summary.retry_delay_sec);
    let max_retries = ctx.config.summary.retry_attempts;
    let mut ai_enabled = ctx.config.ai_analyzer.enabled;

    if ai_enabled {
        if let Err(e) = crate::ai_validation::validate_ai_capabilities(&ctx.config.ai_analyzer) {
            eprintln!("[WARN] AI validation failed: {}", e);
            eprintln!("[WARN] Gracefully falling back to deterministic summaries only.");
            ai_enabled = false;
        } else {
            println!("[OK] AI capabilities validated. Semantic summaries enabled.");
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let mut last_window: Option<String> = None;
    let mut focus_start = Instant::now();
    let mut idle_active = false;
    let mut last_command: Option<String> = None;
    let mut last_git_state: Option<String> = None;
    let mut current_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut pending_ai: Vec<PendingAiJob> = Vec::new();

    let poll_interval = Duration::from_secs(ctx.config.tracking.poll_interval_sec);
    let idle_sleep = Duration::from_secs(ctx.config.tracking.idle_sleep_sec);
    let idle_threshold = ctx.config.tracking.idle_threshold_sec;

    while running.load(Ordering::SeqCst) {
        #[cfg(target_os = "linux")]
        let (win_opt, idle_ms_opt, cmd_opt, git_opt) = (
            get_active_window(),
            get_idle_ms(),
            get_latest_command(),
            get_git_activity(),
        );

        #[cfg(not(target_os = "linux"))]
        let (win_opt, idle_ms_opt, cmd_opt, git_opt) = (None, None, None, None);

        // ─── Day rotation ─────────────────────────────────────────────
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if today != current_date {
            println!("\nDay changed: {current_date} -> {today}");
            manager.finalize();

            let logger = Logger::new(&ctx.config.storage);
            let all_groups = logger.read_normalized_sessions();

            let prev_groups: Vec<_> = all_groups
                .iter()
                .filter(|g| {
                    g.start_time.starts_with(&current_date)
                })
                .cloned()
                .collect();

            if !prev_groups.is_empty() {
                if let Err(e) = archiver.finalize_day(&current_date, &prev_groups, &ctx.config.storage) {
                    eprintln!("Warning: failed to archive day {current_date}: {e}");
                } else {
                    if ai_enabled {
                        match invoke_ai_analyzer(ctx, Some(&current_date)) {
                            Ok(_) => {
                                // Semantic summary written, update metadata
                                let mut meta = archiver.read_metadata(&current_date)
                                    .ok()
                                    .flatten()
                                    .unwrap_or_else(|| {
                                        archiver.build_metadata(
                                            &current_date, &prev_groups, "finalized",
                                            None, None, 0,
                                        )
                                    });
                                meta.semantic_summary = Some("semantic.txt".to_string());
                                meta.status = "finalized".to_string();
                                let _ = archiver.write_metadata(&current_date, &meta);
                                println!("Semantic summary generated for {current_date}");
                            }
                            Err(e) => {
                                eprintln!("AI summary failed for {current_date}: {e}");
                                if max_retries > 0 {
                                    pending_ai.push(PendingAiJob::new(&current_date));
                                    println!("Queued AI summary for retry (date: {current_date})");
                                }
                            }
                        }
                    }
                }
            }

            let new_date = today.clone();
            current_date = today;
            manager = SessionManager::new(&ctx.config);
            println!("New day started: {new_date}");
        }

        // ─── AI retry queue ───────────────────────────────────────────
        if ai_enabled && !pending_ai.is_empty() {
            let mut completed_indices: Vec<usize> = Vec::new();
            for (i, job) in pending_ai.iter().enumerate() {
                let should_retry = match job.last_attempt {
                    Some(t) => t.elapsed() >= retry_delay,
                    None => true,
                };
                if !should_retry {
                    continue;
                }

                match invoke_ai_analyzer(ctx, Some(&job.date)) {
                    Ok(_) => {
                        // Update metadata
                        if let Ok(Some(mut meta)) = archiver.read_metadata(&job.date) {
                            meta.semantic_summary = Some("semantic.txt".to_string());
                            meta.status = "finalized".to_string();
                            let _ = archiver.write_metadata(&job.date, &meta);
                        }
                        println!("AI summary completed for {} (retry {})", job.date, job.retry_count);
                        completed_indices.push(i);
                    }
                    Err(e) => {
                        if job.retry_count + 1 >= max_retries {
                            eprintln!("AI summary exhausted for {} after {} retries: {e}", job.date, max_retries);
                            if let Ok(Some(mut meta)) = archiver.read_metadata(&job.date) {
                                meta.error = Some(format!("Exhausted after {max_retries} retries: {e}"));
                                meta.status = "failed".to_string();
                                let _ = archiver.write_metadata(&job.date, &meta);
                            }
                            completed_indices.push(i);
                        } else {
                            eprintln!("AI summary retry {}/{} for {} failed: {e}",
                                job.retry_count + 1, max_retries, job.date);
                        }
                    }
                }
            }
            // Remove completed jobs (walk backwards so indices stay valid)
            for &i in completed_indices.iter().rev() {
                pending_ai.remove(i);
            }
            // Bump retry_count and last_attempt for remaining jobs
            for job in &mut pending_ai {
                if job.last_attempt.is_some() {
                    job.retry_count += 1;
                }
                job.last_attempt = Some(Instant::now());
            }
        }

        // ─── Idle detection ───────────────────────────────────────────
        if let Some(idle_ms) = idle_ms_opt {
            let idle_sec = idle_ms / 1000;

            if idle_sec >= idle_threshold {
                if !idle_active {
                    if last_window.is_some() {
                        let duration = focus_start.elapsed().as_secs();
                        if let Some(win) = win_opt.clone() {
                            manager
                                .process_window_session(win.app, win.title, win.workspace, duration);
                        }
                        focus_start = Instant::now();
                    }
                    manager.emit_idle(idle_sec);
                    idle_active = true;
                    println!("User idle: {idle_sec} sec");
                }
                thread::sleep(idle_sleep);
                continue;
            } else {
                idle_active = false;
            }
        }

        // ─── Window tracking ──────────────────────────────────────────
        if let Some(win) = win_opt {
            if !win.app.is_empty() {
                let current = format!("{}::{}", win.app, win.title);

                if Some(current.clone()) != last_window {
                    let duration = focus_start.elapsed().as_secs();
                    manager.process_window_session(
                        win.app.clone(),
                        win.title.clone(),
                        win.workspace,
                        duration,
                    );
                    println!("Focused: {} | {} | {duration} sec", win.app, win.title);
                    last_window = Some(current);
                    focus_start = Instant::now();
                }
            }
        }

        // ─── Terminal tracking ────────────────────────────────────────
        if let Some(cmd) = cmd_opt {
            if Some(cmd.clone()) != last_command {
                let workflow = classify_command(&cmd);
                manager.logger.log_event(
                    "terminal_command",
                    "terminal_tracker",
                    Some("Console".to_string()),
                    None,
                    None,
                    None,
                    json!({ "command": cmd, "workflow": workflow.label() }),
                );
                manager.push_terminal_workflow(workflow.label());
                println!("Terminal: {cmd} [{}]", workflow.label());
                last_command = Some(cmd);
            }
        }

        // ─── Git tracking ─────────────────────────────────────────────
        if let Some(activity) = git_opt {
            let current_git = activity.to_string();
            if Some(current_git.clone()) != last_git_state {
                manager.logger.log_event(
                    "git_activity",
                    "git_tracker",
                    None,
                    None,
                    None,
                    None,
                    activity.clone(),
                );
                if let Some(summary) = build_git_summary(&activity) {
                    manager.push_git_summary(summary);
                }
                println!("Git activity updated");
                last_git_state = Some(current_git);
            }
        }

        thread::sleep(poll_interval);
    }

    println!("\nShutting down gracefully...");
    manager.finalize();
    println!("Flushed all sessions. Goodbye!");
}
