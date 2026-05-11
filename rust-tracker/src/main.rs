mod cli;
mod collector;
mod config;
mod models;
mod session;
mod storage;
mod utils;

use clap::Parser;

use cli::commands::{Cli, Commands};

use collector::linux::idle::get_idle_ms;
use collector::linux::window::get_active_window;

use session::manager::SessionManager;

use serde_json::json;

use std::thread;
use std::time::{Duration, Instant};

use storage::logger::log_event;

fn main() {

    let cli = Cli::parse();

    match cli.command {

        Commands::Start => {

            log_event(
                "tracker_started",
                "system",
                None,
                None,
                None,
                None,
                json!({
                    "message": "Tracking started"
                }),
            );

            println!("Tracker started");

            let mut manager =
                SessionManager::new();

            let mut last_window:
                Option<String> = None;

            let mut focus_start =
                Instant::now();

            let mut idle_active = false;

            loop {

                // Idle detection
                if let Some(idle_ms) =
                    get_idle_ms()
                {

                    let idle_sec =
                        idle_ms / 1000;

                    if idle_sec
                        >= SessionManager::IDLE_THRESHOLD_SEC
                    {

                        if !idle_active {

                            manager.emit_idle(
                                idle_sec
                            );

                            idle_active = true;
                        }

                        thread::sleep(
                            Duration::from_secs(2)
                        );

                        continue;
                    }
                    else {

                        idle_active = false;
                    }
                }

                // Window tracking
                if let Some(win) =
                    get_active_window()
                {

                    if win.app.is_empty() {
                        continue;
                    }

                    let current =
                        format!(
                            "{}::{}",
                            win.app,
                            win.title
                        );

                    if Some(current.clone())
                        != last_window
                    {

                        let duration =
                            focus_start
                                .elapsed()
                                .as_secs();

                        manager.process_window_session(
                            win.app.clone(),
                            win.title.clone(),
                            win.workspace,
                            duration,
                        );

                        println!(
                            "Focused: {} | {} | {} sec",
                            win.app,
                            win.title,
                            duration
                        );

                        last_window =
                            Some(current);

                        focus_start =
                            Instant::now();
                    }
                }

                thread::sleep(
                    Duration::from_secs(3)
                );
            }
        }

        Commands::Pause => {

            log_event(
                "tracker_paused",
                "system",
                None,
                None,
                None,
                None,
                json!({
                    "message": "Tracking paused"
                }),
            );

            println!("Tracker paused");
        }

        Commands::Resume => {

            log_event(
                "tracker_resumed",
                "system",
                None,
                None,
                None,
                None,
                json!({
                    "message": "Tracking resumed"
                }),
            );

            println!("Tracker resumed");
        }

        Commands::Stop => {

            log_event(
                "tracker_stopped",
                "system",
                None,
                None,
                None,
                None,
                json!({
                    "message": "Tracking stopped"
                }),
            );

            println!("Tracker stopped");
        }
    }
}