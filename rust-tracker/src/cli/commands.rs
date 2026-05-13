use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tracker")]
#[command(about = "Local-first deterministic desktop activity tracker", long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Path to tracker.toml config file")]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the tracking loop (press Ctrl+C to stop)
    Start,
    /// Log a pause event (tracking continues synchronously)
    Pause,
    /// Log a resume event
    Resume,
    /// Log a stop event
    Stop,
    /// Display activity report from normalized sessions
    Report,
    /// Generate AI-powered daily summary (requires py-analyzer + Ollama)
    ReportAi,
    /// Write a default tracker.toml to the current directory
    InitConfig,
}
