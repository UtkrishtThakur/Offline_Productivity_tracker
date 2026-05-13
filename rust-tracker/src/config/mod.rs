mod settings;

pub use settings::*;

use std::fmt;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// AppContext — shared runtime context holding the parsed config
// ---------------------------------------------------------------------------

pub struct AppContext {
    pub config: TrackerConfig,
    #[allow(dead_code)]
    pub config_path: Option<PathBuf>,
}

impl AppContext {
    pub fn load(cli_config_path: Option<&str>) -> Result<Self, ConfigError> {
        let config_path = resolve_config_path(cli_config_path)?;
        let config = match &config_path {
            Some(path) => {
                let contents = std::fs::read_to_string(path)
                    .map_err(|e| ConfigError::Io {
                        path: path.clone(),
                        source: e,
                    })?;
                toml::from_str(&contents)
                    .map_err(|e| ConfigError::Parse {
                        path: path.clone(),
                        source: e,
                    })?
            }
            None => TrackerConfig::default(),
        };

        let config = apply_env_overrides(config);

        Ok(Self { config, config_path })
    }

    /// Generate default config as a TOML string and write to path.
    pub fn write_default(path: &std::path::Path) -> Result<(), ConfigError> {
        let config = TrackerConfig::default();
        let toml_str = toml::to_string_pretty(&config)
            .map_err(|e| ConfigError::Serialize { source: e })?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }

        std::fs::write(path, toml_str)
            .map_err(|e| ConfigError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Config resolution order
// ---------------------------------------------------------------------------

/// Resolve config file path. Returns `None` if no file found (use defaults).
fn resolve_config_path(cli_path: Option<&str>) -> Result<Option<PathBuf>, ConfigError> {
    // 1. CLI --config path
    if let Some(p) = cli_path {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(Some(path));
        }
        return Err(ConfigError::NotFound { path });
    }

    // 2. TRACKER_CONFIG env var
    if let Ok(p) = std::env::var("TRACKER_CONFIG") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    // 3. ./tracker.toml (CWD)
    let cwd_path = PathBuf::from("tracker.toml");
    if cwd_path.exists() {
        return Ok(Some(cwd_path));
    }

    // 4. Platform-specific config dir
    if let Some(dir_path) = platform_config_path() {
        if dir_path.exists() {
            return Ok(Some(dir_path));
        }
    }

    // 5. No config found — use defaults
    Ok(None)
}

/// Platform-specific config directory path.
fn platform_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(base.join("tracker").join("tracker.toml"))
    }

    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(base).join("tracker").join("tracker.toml"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    None
}

// ---------------------------------------------------------------------------
// Environment variable overrides
// ---------------------------------------------------------------------------

fn apply_env_overrides(mut config: TrackerConfig) -> TrackerConfig {
    // Tracking
    if let Ok(v) = std::env::var("TRACKER_POLL_INTERVAL_SEC") {
        if let Ok(n) = v.parse() {
            config.tracking.poll_interval_sec = n;
        }
    }
    if let Ok(v) = std::env::var("TRACKER_IDLE_SLEEP_SEC") {
        if let Ok(n) = v.parse() {
            config.tracking.idle_sleep_sec = n;
        }
    }
    if let Ok(v) = std::env::var("TRACKER_IDLE_THRESHOLD_SEC") {
        if let Ok(n) = v.parse() {
            config.tracking.idle_threshold_sec = n;
        }
    }
    if let Ok(v) = std::env::var("TRACKER_ADJACENCY_WINDOW_SEC") {
        if let Ok(n) = v.parse() {
            config.tracking.adjacency_window_sec = n;
        }
    }
    if let Ok(v) = std::env::var("TRACKER_MIN_MEANINGFUL_SEC") {
        if let Ok(n) = v.parse() {
            config.tracking.min_meaningful_sec = n;
        }
    }

    // Storage
    if let Ok(v) = std::env::var("TRACKER_SESSION_DIR") {
        config.storage.session_dir = v;
    }
    if let Ok(v) = std::env::var("TRACKER_EVENTS_FILE") {
        config.storage.events_file = v;
    }
    if let Ok(v) = std::env::var("TRACKER_NORMALIZED_FILE") {
        config.storage.normalized_file = v;
    }

    // AI Analyzer
    if let Ok(v) = std::env::var("TRACKER_AI_ENABLED") {
        config.ai_analyzer.enabled = v.eq_ignore_ascii_case("true");
    }
    if let Ok(v) = std::env::var("TRACKER_AI_OLLAMA_HOST") {
        config.ai_analyzer.ollama_host = v;
    }
    if let Ok(v) = std::env::var("TRACKER_AI_MODEL") {
        config.ai_analyzer.model = v;
    }
    if let Ok(v) = std::env::var("TRACKER_AI_OUTPUT_DIR") {
        config.ai_analyzer.output_dir = v;
    }

    // Logging
    if let Ok(v) = std::env::var("TRACKER_LOG_FILE") {
        config.logging.enable_file_logging = v.eq_ignore_ascii_case("true");
    }
    if let Ok(v) = std::env::var("TRACKER_LOG_LEVEL") {
        config.logging.log_level = v;
    }

    config
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigError {
    NotFound { path: PathBuf },
    Io { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, source: toml::de::Error },
    Serialize { source: toml::ser::Error },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(f, "Config file not found: {}", path.display())
            }
            Self::Io { path, source } => {
                write!(f, "IO error reading {}: {}", path.display(), source)
            }
            Self::Parse { path, source } => {
                write!(f, "Parse error in {}: {}", path.display(), source)
            }
            Self::Serialize { source } => {
                write!(f, "Config serialization error: {}", source)
            }
        }
    }
}

impl std::error::Error for ConfigError {}
