use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Default helpers
// ---------------------------------------------------------------------------

fn default_poll_interval() -> u64 { 3 }
fn default_idle_sleep() -> u64 { 2 }
fn default_idle_threshold() -> u64 { 120 }
fn default_adjacency_window() -> u64 { 300 }
fn default_min_meaningful() -> u64 { 2 }
fn default_session_dir() -> String { "../sessions/active_session".into() }
fn default_events_file() -> String { "events.jsonl".into() }
fn default_normalized_file() -> String { "normalized_sessions.jsonl".into() }
fn default_ollama_host() -> String { "http://localhost:11434".into() }
fn default_model() -> String { "qwen2.5:7b".into() }
fn default_ai_output_dir() -> String { "outputs".into() }
fn default_ai_enabled() -> bool { false }
fn default_log_enabled() -> bool { true }
fn default_log_level() -> String { "info".into() }

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerConfig {
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub ai_analyzer: AiAnalyzerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            tracking: TrackingConfig::default(),
            storage: StorageConfig::default(),
            ai_analyzer: AiAnalyzerConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_sec: u64,

    #[serde(default = "default_idle_sleep")]
    pub idle_sleep_sec: u64,

    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_sec: u64,

    #[serde(default = "default_adjacency_window")]
    pub adjacency_window_sec: u64,

    #[serde(default = "default_min_meaningful")]
    pub min_meaningful_sec: u64,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            poll_interval_sec: default_poll_interval(),
            idle_sleep_sec: default_idle_sleep(),
            idle_threshold_sec: default_idle_threshold(),
            adjacency_window_sec: default_adjacency_window(),
            min_meaningful_sec: default_min_meaningful(),
        }
    }
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_session_dir")]
    pub session_dir: String,

    #[serde(default = "default_events_file")]
    pub events_file: String,

    #[serde(default = "default_normalized_file")]
    pub normalized_file: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            session_dir: default_session_dir(),
            events_file: default_events_file(),
            normalized_file: default_normalized_file(),
        }
    }
}

// ---------------------------------------------------------------------------
// AI Analyzer (optional Python layer)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnalyzerConfig {
    #[serde(default = "default_ai_enabled")]
    pub enabled: bool,

    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,

    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default = "default_ai_output_dir")]
    pub output_dir: String,
}

impl Default for AiAnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: default_ai_enabled(),
            ollama_host: default_ollama_host(),
            model: default_model(),
            output_dir: default_ai_output_dir(),
        }
    }
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_enabled")]
    pub enable_file_logging: bool,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enable_file_logging: default_log_enabled(),
            log_level: default_log_level(),
        }
    }
}
