use std::path::Path;
use std::time::Instant;
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::collector::linux::git::get_git_activity;
use crate::collector::linux::idle::get_idle_ms;
use crate::collector::linux::terminal::get_latest_command;
use crate::collector::linux::window::get_active_window;

use crate::config::AppContext;
use crate::processing::git::build_git_summary;
use crate::processing::terminal::classify_command;
use crate::session::manager::SessionManager;
use crate::storage::archiver::Archiver;
use crate::storage::logger::Logger;

use super::ai_queue::{AiRetryQueue, PendingAiJob};
use super::day_manager::DayManager;
use super::lockfile::TrackerLock;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Invoke the Python AI analyzer, optionally for a specific date.
/// When `date` is Some, the analyzer writes semantic.txt to summaries/{date}/.
pub fn invoke_ai_analyzer(ctx: &AppContext, date: Option<&str>) -> Result<String, String> {
    let exe_dir = crate::utils::paths::get_exe_dir();
    let analyzer_script = exe_dir.join("py-analyzer").join("analyzer.py");

    let script_path = if analyzer_script.exists() {
        analyzer_script
    } else {
        let local_script = Path::new("py-analyzer/analyzer.py");
        if local_script.exists() {
            local_script.to_path_buf()
        } else {
            return Err(format!(
                "py-analyzer/analyzer.py not found (checked exe-relative and CWD)"
            ));
        }
    };

    let c = &ctx.config.ai_analyzer;
    let mut cmd = std::process::Command::new("python3");
    cmd.arg(&script_path)
        .env("TRACKER_AI_ENABLED", "true")
        .env("TRACKER_AI_OLLAMA_HOST", &c.ollama_host)
        .env("TRACKER_AI_MODEL", &c.model)
        .env("TRACKER_SESSION_DIR", &ctx.config.storage.session_dir)
        .env("TRACKER_NORMALIZED_FILE", &ctx.config.storage.normalized_file)
        .env("TRACKER_AI_OUTPUT_DIR", &c.output_dir);

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

/// Run the main tracking loop with day rotation and AI retry queue.
pub fn run_tracking_loop(ctx: &AppContext, mut retry_queue: AiRetryQueue) {
    let mut manager = SessionManager::new(&ctx.config);
    let archiver = Archiver::new(&ctx.config);
    let mut day_manager = DayManager::new();
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
        if day_manager.date_changed() {
            let prev_date = day_manager.current_date.clone();
            println!("\nDay changed: {prev_date} -> {}", DayManager::today());

            flush_and_finalize_day(
                ctx,
                &mut manager,
                &archiver,
                &prev_date,
                ai_enabled,
                &mut retry_queue,
            );

            day_manager.advance();
            manager = SessionManager::new(&ctx.config);
            println!("New day started: {}", day_manager.current_date);
        }

        // ─── AI retry queue ───────────────────────────────────────────
        if ai_enabled {
            let archiver_ref = &archiver;
            retry_queue.process_retries(
                |date| {
                    let result = invoke_ai_analyzer(ctx, Some(date));
                    if let Ok(_) = &result {
                        if let Err(e) = archiver_ref.mark_semantic_complete(date) {
                            eprintln!("Warning: failed to update metadata for {date}: {e}");
                        }
                    }
                    result.map(|_| ())
                },
                |date, retry_count| {
                    if let Err(e) = archiver_ref.mark_failed(
                        date,
                        &format!("Exhausted after {retry_count} retries"),
                    ) {
                        eprintln!("Warning: failed to mark {date} as failed: {e}");
                    }
                },
            );
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

/// Day-rotation finalization with proper lifecycle ordering.
///
/// Order:
///   1. Flush manager (write pending events + groups to JSONL)
///   2. Read date-scoped groups
///   3. Write deterministic summary → metadata = deterministic_complete
///   4a. AI enabled: try sync AI, if succeeds → semantic_complete; if fails → semantic_pending + enqueue
///   4b. AI disabled: skip to 5
///   5. Cleanup JSONL logs (only after all summaries are persisted)
///   6. Mark finalized
fn flush_and_finalize_day(
    ctx: &AppContext,
    manager: &mut SessionManager,
    archiver: &Archiver,
    date: &str,
    ai_enabled: bool,
    retry_queue: &mut AiRetryQueue,
) {
    manager.finalize();

    let logger = Logger::new(&ctx.config.storage);
    let prev_groups = logger.read_normalized_sessions_for_date(date);

    if prev_groups.is_empty() {
        return;
    }

    // Phase 1: deterministic summary (always)
    let det_result = archiver.write_deterministic_summary(date, &prev_groups);
    let _det_meta = match det_result {
        Ok(meta) => {
            println!("Deterministic summary written for {date}");
            meta
        }
        Err(e) => {
            eprintln!("Warning: failed to write deterministic summary for {date}: {e}");
            return;
        }
    };

    // Phase 2: semantic summary (AI optional)
    let semantic_done = if ai_enabled {
        match invoke_ai_analyzer(ctx, Some(date)) {
            Ok(_) => {
                if let Err(e) = archiver.mark_semantic_complete(date) {
                    eprintln!("Warning: failed to update metadata for {date}: {e}");
                }
                println!("Semantic summary generated for {date}");
                true
            }
            Err(e) => {
                eprintln!("AI summary failed for {date}: {e}");
                if ctx.config.summary.retry_attempts > 0 {
                    if let Err(e) = archiver.mark_semantic_pending(date) {
                        eprintln!("Warning: failed to mark semantic pending for {date}: {e}");
                    }
                    retry_queue.enqueue_if_missing(PendingAiJob::new(date));
                    println!("Queued AI summary for retry (date: {date})");
                }
                false
            }
        }
    } else {
        true
    };

    // Phase 3: cleanup and finalize — only when all summaries are complete
    if semantic_done {
        if archiver.auto_cleanup {
            if let Err(e) = archiver.cleanup_day_logs(date, &ctx.config.storage) {
                eprintln!("Warning: log cleanup failed for {date}: {e}");
            }
        }
        if let Err(e) = archiver.mark_finalized(date) {
            eprintln!("Warning: failed to finalize metadata for {date}: {e}");
        }
    }
}

/// Run diagnostic checks and print a health report.
pub fn run_doctor(ctx: &AppContext) {
    println!("=== Tracker Doctor ===\n");
    let mut all_ok = true;

    // 1. Config validation
    println!("[1/9] Config...");
    if let Some(ref path) = ctx.config_path {
        if path.exists() {
            println!("  Config file: {} (OK)", path.display());
        } else {
            println!("  Config file: {} (WARN: path resolved but file missing)", path.display());
        }
    } else {
        println!("  Config: built-in defaults (OK)");
    }

    // 2. Session directory
    println!("\n[2/9] Session directory...");
    let session_dir = Path::new(&ctx.config.storage.session_dir);
    if session_dir.exists() {
        println!("  Path: {} (OK)", session_dir.display());
    } else {
        println!("  Path: {} (does not exist, will be created on first write)", session_dir.display());
    }
    match std::fs::create_dir_all(session_dir) {
        Ok(_) => println!("  Write permission: OK"),
        Err(e) => {
            println!("  Write permission: FAILED ({e})");
            all_ok = false;
        }
    }

    // 3. Summaries directory
    println!("\n[3/9] Summaries directory...");
    let summaries_dir = Path::new(&ctx.config.storage.summaries_dir);
    match std::fs::create_dir_all(summaries_dir) {
        Ok(_) => println!("  Path: {} (OK)", summaries_dir.display()),
        Err(e) => {
            println!("  Path: {} (FAILED: {e})", summaries_dir.display());
            all_ok = false;
        }
    }

    // 4. Lockfile state
    println!("\n[4/9] Lockfile state...");
    let lock_path = Path::new(&ctx.config.storage.session_dir).join("tracker.lock");
    if lock_path.exists() {
        println!("  Lockfile exists at {}", lock_path.display());
        match TrackerLock::acquire(&lock_path) {
            Ok(_) => println!("  Lockfile is stale (was able to acquire lock)"),
            Err(e) => {
                println!("  Lockfile is active (another tracker process is running): {e}");
                all_ok = false;
            }
        }
    } else {
        println!("  No lockfile (OK)");
    }

    // 5. AI analyzer availability
    println!("\n[5/9] AI analyzer...");
    if ctx.config.ai_analyzer.enabled {
        let exe_dir = crate::utils::paths::get_exe_dir();
        let script1 = exe_dir.join("py-analyzer").join("analyzer.py");
        let script2 = Path::new("py-analyzer/analyzer.py");
        if script1.exists() || script2.exists() {
            println!("  Script found (OK)");
        } else {
            println!("  Script NOT found at {} or {} (will fall back to deterministic)", script1.display(), script2.display());
        }
        match std::process::Command::new("python3").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("  Python3: {ver} (OK)");
            }
            _ => {
                println!("  Python3: NOT FOUND (will fall back to deterministic)");
            }
        }
    } else {
        println!("  AI analyzer: disabled (OK)");
    }

    // 6. Ollama connectivity
    println!("\n[6/9] Ollama...");
    if ctx.config.ai_analyzer.enabled {
        let url = format!("{}/api/tags", ctx.config.ai_analyzer.ollama_host);
        let agent = ureq::builder()
            .timeout(Duration::from_secs(3))
            .build();
        match agent.get(&url).call() {
            Ok(_) => println!("  Server at {} reachable (OK)", ctx.config.ai_analyzer.ollama_host),
            Err(e) => println!("  Server at {} unreachable ({}) (will fall back to deterministic)", ctx.config.ai_analyzer.ollama_host, e),
        }
    } else {
        println!("  Ollama: skipped (AI disabled)");
    }

    // 7. Model existence
    println!("\n[7/9] Model...");
    if ctx.config.ai_analyzer.enabled {
        match crate::ai_validation::validate_ai_capabilities(&ctx.config.ai_analyzer) {
            Ok(_) => println!("  Model '{}' found (OK)", ctx.config.ai_analyzer.model),
            Err(e) => print!("  Model check: {e}"),
        }
    } else {
        println!("  Model check: skipped (AI disabled)");
    }

    // 8. Active sessions
    println!("\n[8/9] Active session data...");
    let logger = Logger::new(&ctx.config.storage);
    let sessions = logger.read_normalized_sessions();
    if sessions.is_empty() {
        println!("  No sessions recorded yet");
    } else {
        let total_sec: u64 = sessions.iter().map(|s| s.total_duration_sec).sum();
        println!("  {} session groups, {} total seconds tracked", sessions.len(), total_sec);
    }

    // 9. Summary directories
    println!("\n[9/9] Summary directories...");
    let summaries = Path::new(&ctx.config.storage.summaries_dir);
    if summaries.exists() {
        let entries: Vec<_> = std::fs::read_dir(summaries)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        println!("  {} day summaries archived", entries.len());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let meta_path = entry.path().join("metadata.json");
            let has_det = entry.path().join("deterministic.txt").exists();
            let has_sem = entry.path().join("semantic.txt").exists();
            let status = if meta_path.exists() {
                match std::fs::read_to_string(&meta_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.get("status").and_then(|s| s.as_str().map(|s| s.to_string())))
                {
                    Some(s) => s,
                    None => "unknown".to_string(),
                }
            } else {
                "no metadata".to_string()
            };
            println!("  {name}: status={status}, deterministic={has_det}, semantic={has_sem}");
        }
    } else {
        println!("  No summary directory yet");
    }

    println!("\n=== Doctor complete ===");
    if all_ok {
        println!("Result: ALL CHECKS PASSED");
    } else {
        println!("Result: SOME CHECKS FAILED (review warnings above)");
    }
}
