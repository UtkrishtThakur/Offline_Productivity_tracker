use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tracker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Start,
    Pause,
    Resume,
    Stop,
    Report,
}