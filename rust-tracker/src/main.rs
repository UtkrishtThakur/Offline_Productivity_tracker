// main.rs — Orchestration Only
//
// Collectors collect. Processing enriches. Storage persists.
// This file only wires everything together.

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

use storage::logger::log_event;

use processing::summary::format_report;
use storage::logger::read_normalized_sessions;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {

    let cli = Cli::parse();

    match cli.command {

        Commands::Start => {
            log_event(
                "tracker_started",
                "system",
                None, None, None, None,
                json!({ "message": "Tracking started" }),
            );
            println!("Tracker started. Press Ctrl+C to stop.");
            run_tracking_loop();
        }

        Commands::Pause => {
            log_event("tracker_paused", "system", None, None, None, None, json!({ "message": "Tracking paused" }));
            println!("Tracker paused");
        }

        Commands::Resume => {
            log_event("tracker_resumed", "system", None, None, None, None, json!({ "message": "Tracking resumed" }));
            println!("Tracker resumed");
        }

        Commands::Stop => {
            log_event("tracker_stopped", "system", None, None, None, None, json!({ "message": "Tracking stopped" }));
            println!("Tracker stopped");
        }

        Commands::Report => {
            let sessions = read_normalized_sessions();
            println!("{}", format_report(&sessions));
        }
    }
}

/// Platform-agnostic collector selection
fn get_platform_telemetry() -> (Option<collector::linux::window::WindowInfo>, Option<u64>, Option<String>, Option<serde_json::Value>) {
    #[cfg(target_os = "linux")]
    {
        use collector::linux::{window, idle, terminal, git};
        (
            window::get_active_window(),
            idle::get_idle_ms(),
            terminal::get_latest_command(),
            git::get_git_activity(),
        )
    }

    #[cfg(target_os = "windows")]
    {
        use collector::windows::{window, idle, terminal, git};
        (
            // Need to map windows::WindowInfo to linux::WindowInfo or use a shared trait/struct
            // For now, assume common struct in models/ if we had one, but window info is currently split.
            // Let's use a simplified approach since we are focusing on "rest of work".
            None, // Placeholder for Windows implementation details
            None,
            None,
            None,
        )
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    (None, None, None, None)
}

/// Core tracking loop — collects, enriches, groups, persists.
fn run_tracking_loop() {

    let mut manager = SessionManager::new();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let mut last_window: Option<String> = None;
    let mut focus_start = Instant::now();
    let mut idle_active = false;
    let mut last_command: Option<String> = None;
    let mut last_git_state: Option<String> = None;

    while running.load(Ordering::SeqCst) {

        #[cfg(target_os = "linux")]
        let (win_opt, idle_ms_opt, cmd_opt, git_opt) = {
            (
                get_active_window(),
                get_idle_ms(),
                get_latest_command(),
                get_git_activity(),
            )
        };

        #[cfg(not(target_os = "linux"))]
        let (win_opt, idle_ms_opt, cmd_opt, git_opt) = (None, None, None, None);

        // -----------------------------------------
        // 1. IDLE TRACKING
        // -----------------------------------------

        if let Some(idle_ms) = idle_ms_opt {
            let idle_sec = idle_ms / 1000;

            if idle_sec >= SessionManager::IDLE_THRESHOLD_SEC {
                if !idle_active {
                    if last_window.is_some() {
                        let duration = focus_start.elapsed().as_secs();
                        if let Some(win) = win_opt.clone() {
                            manager.process_window_session(win.app, win.title, win.workspace, duration);
                        }
                        focus_start = Instant::now();
                    }
                    manager.emit_idle(idle_sec);
                    idle_active = true;
                    println!("User idle: {} sec", idle_sec);
                }
                thread::sleep(Duration::from_secs(2));
                continue;
            } else {
                idle_active = false;
            }
        }

        // -----------------------------------------
        // 2. WINDOW TRACKING
        // -----------------------------------------

        if let Some(win) = win_opt {
            if !win.app.is_empty() {
                let current = format!("{}::{}", win.app, win.title);

                if Some(current.clone()) != last_window {
                    let duration = focus_start.elapsed().as_secs();
                    manager.process_window_session(win.app.clone(), win.title.clone(), win.workspace, duration);
                    println!("Focused: {} | {} | {} sec", win.app, win.title, duration);
                    last_window = Some(current);
                    focus_start = Instant::now();
                }
            }
        }

        // -----------------------------------------
        // 3. TERMINAL TRACKING
        // -----------------------------------------

        if let Some(cmd) = cmd_opt {
            if Some(cmd.clone()) != last_command {
                let workflow = classify_command(&cmd);
                log_event("terminal_command", "terminal_tracker", Some("Console".to_string()), None, None, None, json!({ "command": cmd, "workflow": workflow.label() }));
                manager.push_terminal_workflow(workflow.label());
                println!("Terminal: {} [{}]", cmd, workflow.label());
                last_command = Some(cmd);
            }
        }

        // -----------------------------------------
        // 4. GIT TRACKING
        // -----------------------------------------

        if let Some(activity) = git_opt {
            let current_git = activity.to_string();
            if Some(current_git.clone()) != last_git_state {
                log_event("git_activity", "git_tracker", None, None, None, None, activity.clone());
                if let Some(summary) = build_git_summary(&activity) {
                    manager.push_git_summary(summary);
                }
                println!("Git activity updated");
                last_git_state = Some(current_git);
            }
        }

        thread::sleep(Duration::from_secs(3));
    }

    println!("\nShutting down gracefully...");
    manager.finalize();
    println!("Flushed all sessions. Goodbye!");
}