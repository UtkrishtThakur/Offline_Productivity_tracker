mod cli;
mod collector;
mod config;
mod models;
mod processing;
mod session;
mod storage;
mod utils;

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
use std::time::{Duration, Instant};

use processing::summary::format_report;
use storage::logger::Logger;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
            match invoke_ai_analyzer(&ctx) {
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

fn invoke_ai_analyzer(ctx: &config::AppContext) -> Result<String, String> {
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
    let output = std::process::Command::new("python3")
        .arg(&analyzer_script)
        .env("TRACKER_AI_ENABLED", if c.enabled { "true" } else { "false" })
        .env("TRACKER_AI_OLLAMA_HOST", &c.ollama_host)
        .env("TRACKER_AI_MODEL", &c.model)
        .env("TRACKER_AI_OUTPUT_DIR", &c.output_dir)
        .env("TRACKER_SESSION_DIR", &ctx.config.storage.session_dir)
        .env("TRACKER_NORMALIZED_FILE", &ctx.config.storage.normalized_file)
        .output()
        .map_err(|e| format!("Failed to launch python3: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AI analyzer exited with error:\n{stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_tracking_loop(ctx: &config::AppContext) {
    let mut manager = SessionManager::new(&ctx.config);
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
