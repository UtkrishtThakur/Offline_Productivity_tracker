mod cli;
mod collector;
mod config;
mod models;
mod processing;
mod session;
mod storage;
mod utils;
mod ai_validation;
mod runtime;

use clap::Parser;

use cli::commands::{Cli, Commands};

use processing::summary::format_report;
use storage::archiver::Archiver;
use storage::logger::Logger;

use runtime::lifecycle;
use runtime::lockfile::TrackerLock;
use runtime::day_manager;
use runtime::ai_queue::AiRetryQueue;

use serde_json::json;

fn main() {
    let cli = Cli::parse();

    let ctx = config::AppContext::load(cli.config.as_deref()).unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    let sys_logger = Logger::new(&ctx.config.storage);

    match cli.command {
        Commands::Start => {
            // Acquire single-instance lock
            let lock_path = std::path::Path::new(&ctx.config.storage.session_dir)
                .join("tracker.lock");
            let _lock = match TrackerLock::acquire(&lock_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error: another tracker instance may be running (lockfile: {}): {e}", lock_path.display());
                    eprintln!("If the previous instance crashed, delete the lockfile and retry.");
                    std::process::exit(1);
                }
            };

            sys_logger.log_event(
                "tracker_started",
                "system",
                None, None, None, None,
                json!({ "message": "Tracking started" }),
            );
            println!("Tracker started. Press Ctrl+C to stop.");

            // Create shared retry queue (disk-backed, survives restarts)
            let mut retry_queue = AiRetryQueue::new(
                &ctx.config.storage.session_dir,
                ctx.config.summary.retry_attempts,
                ctx.config.summary.retry_delay_sec,
            );

            // Startup recovery: finalize any unfinished previous days
            let archiver = Archiver::new(&ctx.config);
            let ai_enabled = ctx.config.ai_analyzer.enabled;
            day_manager::startup_recovery(&ctx, &archiver, ai_enabled, &mut retry_queue);

            lifecycle::run_tracking_loop(&ctx, retry_queue);
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
            match lifecycle::invoke_ai_analyzer(&ctx, None) {
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

        Commands::Doctor => {
            lifecycle::run_doctor(&ctx);
        }
    }
}
