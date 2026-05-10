mod cli;
mod collector;
mod models;

use chrono::Local;
use clap::Parser;

use cli::commands::{Cli, Commands};

use collector::linux::window::get_active_window;

use models::event::Event;

use serde_json::json;

use std::fs::{self, OpenOptions};
use std::io::Write;

use std::thread;
use std::time::Duration;

fn log_event(
    event_type: &str,
    source: &str,
    app: Option<String>,
    title: Option<String>,
    workspace: Option<i64>,
    data: serde_json::Value,
) {

    let session_name = "active_session";

    let session_path =
        format!("../sessions/{}", session_name);

    fs::create_dir_all(&session_path)
        .expect("Failed to create session directory");

    let log_file_path =
        format!("{}/events.jsonl", session_path);

    let event = Event {

        timestamp:
            Local::now().to_rfc3339(),

        event_type:
            event_type.to_string(),

        source:
            source.to_string(),

        app,

        title,

        workspace,

        data,
    };

    let json =
        serde_json::to_string(&event)
            .expect("Failed to serialize");

    let mut file =
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path)
            .expect("Failed to open log file");

    writeln!(file, "{}", json)
        .expect("Failed to write");
}

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
                json!({
                    "message": "Tracking started"
                }),
            );

            println!("Tracker started");

            let mut last_window:
                Option<String> = None;

            loop {

                if let Some(window) =
                    get_active_window()
                {

                    let current =
                        format!(
                            "{}::{}",
                            window.app,
                            window.title
                        );

                    if Some(current.clone())
                        != last_window
                    {

                        log_event(
                            "window_focus",
                            "window_tracker",
                            Some(window.app.clone()),
                            Some(window.title.clone()),
                            Some(window.workspace),
                            json!({
                                "message": "Window changed"
                            }),
                        );

                        println!(
                            "Focused: {} | {} | Workspace {}",
                            window.app,
                            window.title,
                            window.workspace
                        );

                        last_window =
                            Some(current);
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
                json!({
                    "message": "Tracking stopped"
                }),
            );

            println!("Tracker stopped");
        }
    }
}