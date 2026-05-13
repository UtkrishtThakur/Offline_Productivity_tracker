# rust-tracker

**Local-first, deterministic desktop activity reconstruction engine. Metadata-only, no AI in the pipeline, fully explainable.**

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/python-3.12-blue)](https://python.org)
[![License](https://img.shields.io/badge/license-MIT-green)](#license)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows-lightgrey)]()

---

## Features

- **Metadata-only tracking** — no screenshots, no OCR, no keylogging, no content capture
- **Deterministic enrichment** — pure string parsing, zero ML/AI heuristics in the pipeline
- **Local-first** — all data stays on your machine, no telemetry egress
- **Explainable** — every transformation is auditable in ~200 LOC of Rust
- **No scoring** — reports describe *what happened*, not *how good/bad it was*
- **Multi-source collection** — window focus, idle detection, terminal commands, git activity
- **Activity reconstruction** — groups related work into sessions with project/file/language context
- **Fully configurable** — `tracker.toml` + environment variable overrides, no hardcoded values
- **AI optional** — LLM-powered daily summaries toggle via `ai_analyzer.enabled`
- **Cross-platform** — Linux (gnomectl/xprintidle) and Windows (Win32 API) collectors

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     config/mod.rs + settings.rs                         │
│           tracker.toml loading / env overrides / AppContext             │
└────────────────────────────────┬────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              main.rs                                    │
│                    Orchestration & Polling Loop                         │
└──────┬──────────┬──────────┬──────────┬───────────┬────────────────────┘
       │          │          │          │           │
       ▼          ▼          ▼          ▼           ▼
┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐
│ Window   │ │  Idle    │ │ Terminal │ │  Git   │ │   CLI    │
│ Collector│ │ Collector│ │ Collector│ │Collector│ │  (clap)  │
└────┬─────┘ └───┬──────┘ └────┬─────┘ └───┬────┘ └──────────┘
     │           │             │           │
     ▼           ▼             ▼           ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          SessionManager                                 │
│                                                                         │
│  Owns: Logger, ActivityGrouper, last_event buffer                      │
│  All thresholds from config, all paths from config                     │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Enrichment Layer                                │
│              (processing/enrich.rs)                                     │
│  extract_from_title() → project + file                                  │
│  detect_language()    → language from file extension                    │
│  normalize_app_name() → canonical app name                              │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Activity Grouper                                 │
│              (processing/activity.rs)                                   │
│  Groups by (project, app) key, configurable adjacency window           │
│  Idle boundary splits, terminal/git side-channels                      │
│  Pushes completed ActivityGroup to storage                              │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     Storage Layer (Logger struct)                       │
│              (storage/logger.rs)                                        │
│  Configurable paths: session_dir, events_file, normalized_file         │
│    ├── events.jsonl               (raw Event JSONL)                    │
│    └── normalized_sessions.jsonl  (ActivityGroup JSONL)                │
└──────────┬──────────────────────────────────────────────────────────────┘
           │
     ┌─────┴──────────────────────┐
     ▼                            ▼
┌──────────────┐     ┌──────────────────────────────┐
│   Report     │     │   AI Analysis (optional)     │
│ (summary.rs) │     │   (py-analyzer/)             │
└──────────────┘     └──────────────────────────────┘
```

---

## Repository Structure

```
tracker/
├── rust-tracker/                      # Core Rust tracking engine
│   ├── Cargo.toml                     # Rust deps: clap, serde, chrono, ctrlc, toml
│   └── src/
│       ├── main.rs                    # Entrypoint, config loading, polling loop
│       ├── cli/commands.rs            # CLI: Start/Pause/Resume/Stop/Report/ReportAi/InitConfig
│       ├── config/
│       │   ├── mod.rs                 # AppContext, config loader, env overrides, ConfigError
│       │   └── settings.rs            # TrackerConfig, TrackingConfig, StorageConfig, AiAnalyzerConfig
│       ├── collector/
│       │   ├── linux/                 # Window, idle, terminal, git collectors
│       │   └── windows/               # Win32-based collectors
│       ├── models/                    # Event, EnrichedEvent, ActivityGroup, GitSummary
│       ├── session/manager.rs         # SessionManager orchestrator
│       ├── processing/
│       │   ├── enrich.rs              # Title parsing, lang detection, app normalization
│       │   ├── activity.rs            # Configurable ActivityGrouper
│       │   ├── terminal.rs            # Command classification
│       │   ├── git.rs                 # GitSummary builder
│       │   └── summary.rs             # Human-readable report formatting
│       ├── storage/
│       │   ├── logger.rs              # Logger struct (configurable paths)
│       │   └── schema.rs              # PLACEHOLDER
│       └── utils/                     # PLACEHOLDER (paths, time)
│
├── py-analyzer/                       # Python AI analysis layer (optional)
│   ├── analyzer.py                    # Main entry point
│   ├── config.py                      # AnalyzerConfig from env vars
│   ├── formatter.py                   # JSONL loading + project aggregation
│   ├── prompts.py                     # System + user prompt templates
│   ├── guardrails.py                  # Banned word filter
│   ├── daily_writer.py                # Write summaries to outputs/{date}.txt
│   ├── Dockerfile                     # python:3.12-slim
│   ├── docker-compose.yml             # Mounts sessions/, network=host for Ollama
│   └── .env                           # TRACKER_AI_* configuration
│
├── tracker.toml                       # Default config (generated by tracker init-config)
├── sessions/                          # Runtime session data (gitignored)
├── CLAUDE.md                          # Full architecture guide
└── README.md
```

---

## Configuration

### `tracker.toml` reference

```toml
[tracking]
poll_interval_sec = 3          # Main loop sleep (seconds)
idle_sleep_sec = 2             # Sleep during idle
idle_threshold_sec = 120       # Idle before splitting groups
adjacency_window_sec = 300     # Max gap for merged events (5 min)
min_meaningful_sec = 2         # Noise filter threshold

[storage]
session_dir = "../sessions/active_session"   # Session data directory
events_file = "events.jsonl"                 # Raw event log
normalized_file = "normalized_sessions.jsonl" # Normalized sessions

[ai_analyzer]
enabled = false                # Toggle AI summarization
ollama_host = "http://localhost:11434"
model = "qwen2.5:7b"
output_dir = "outputs"

[logging]
enable_file_logging = true
log_level = "info"
```

### Config resolution order

1. **CLI `--config PATH`** — explicit path (must exist)
2. **`TRACKER_CONFIG`** env var
3. **`./tracker.toml`** — current working directory
4. **`~/.config/tracker/tracker.toml`** — Linux platform dir
5. **Built-in defaults** — no file required

### Environment variable overrides

Every config field can be overridden at runtime:

| Env var | Config field | Default |
|---------|-------------|---------|
| `TRACKER_POLL_INTERVAL_SEC` | `tracking.poll_interval_sec` | `3` |
| `TRACKER_IDLE_THRESHOLD_SEC` | `tracking.idle_threshold_sec` | `120` |
| `TRACKER_ADJACENCY_WINDOW_SEC` | `tracking.adjacency_window_sec` | `300` |
| `TRACKER_MIN_MEANINGFUL_SEC` | `tracking.min_meaningful_sec` | `2` |
| `TRACKER_SESSION_DIR` | `storage.session_dir` | `../sessions/active_session` |
| `TRACKER_EVENTS_FILE` | `storage.events_file` | `events.jsonl` |
| `TRACKER_NORMALIZED_FILE` | `storage.normalized_file` | `normalized_sessions.jsonl` |
| `TRACKER_AI_ENABLED` | `ai_analyzer.enabled` | `false` |
| `TRACKER_AI_OLLAMA_HOST` | `ai_analyzer.ollama_host` | `http://localhost:11434` |
| `TRACKER_AI_MODEL` | `ai_analyzer.model` | `qwen2.5:7b` |
| `TRACKER_AI_OUTPUT_DIR` | `ai_analyzer.output_dir` | `outputs` |
| `TRACKER_LOG_LEVEL` | `logging.log_level` | `info` |

### Generate default config

```bash
tracker init-config
# Writes tracker.toml to CWD with all defaults and documentation
```

---

## How It Works

### Tracking Loop

The main loop polls every `tracking.poll_interval_sec` (default 3s). All thresholds and paths come from config:

1. **Idle Detection** — If idle >= `idle_threshold_sec` (default 120s): flush, split groups, sleep
2. **Window Tracking** — On app/title change: push to `SessionManager`, merge consecutive identical windows, filter `< min_meaningful_sec` noise
3. **Terminal Tracking** — On new command: classify into workflow type, log, push to group
4. **Git Tracking** — On state change: log, build `GitSummary`, push to group

### Enrichment Pipeline

Every window event passes through `processing/enrich.rs`:
- **App normalization** — exact match → substring match → fallback
- **Title parsing** — split on ` - ` / ` — `, find file segment, find project segment
- **Language detection** — file extension → 44-entry lookup

### Activity Grouping

`processing/activity.rs` groups by `(project, app)` with configurable `adjacency_window_sec` and `min_meaningful_sec`. Idle boundaries split groups. Terminal workflows and git summaries inject into the current group as side-channels.

### Storage

`Logger` struct writes to configurable paths. Both `events.jsonl` (raw) and `normalized_sessions.jsonl` (processed) are JSONL — append-only, trivially inspectable.

### AI Summarization (Optional)

When `ai_analyzer.enabled = true`, `tracker report-ai` invokes `py-analyzer/analyzer.py` as a subprocess, forwarding all config values as environment variables. The Python layer reads `normalized_sessions.jsonl`, aggregates by project, sends to Ollama, and writes `outputs/{date}.txt`.

---

## Installation

### Prerequisites

- **Rust** 1.70+ (`rustup` recommended)
- **Linux**: `gnomectl` + `xprintidle` for window/idle tracking
- **Python** 3.12+ (optional, for AI analysis)
- **Ollama** (optional, for AI analysis)

### Build

```bash
cd rust-tracker
cargo build --release
```

### Generate config

```bash
./target/release/tracker init-config
# Edit tracker.toml to customize
```

### AI Analyzer (Optional)

```bash
cd py-analyzer
pip install -r requirements.txt
# Or with Docker:
docker compose up -d
```

---

## Usage

```bash
# Start tracking (press Ctrl+C to stop)
tracker Start
tracker --config /path/to/tracker.toml Start

# View activity report
tracker Report

# Generate AI daily summary (if enabled)
tracker ReportAi

# Log lifecycle events
tracker Pause
tracker Resume
tracker Stop

# Generate default config
tracker InitConfig
```

---

## Example Output

### CLI Report

```
--- Activity Reconstruction Report ---

Used antigravity in:
tracker

Worked on:
- git.rs

Time spent:
1 minutes, 21 seconds

---
```

### AI Daily Summary (`outputs/2026-05-12.txt`)

> Today's activities involved:
>
> - **tracker**: Worked on Rust, spending 2.3 minutes on 'git.rs'.
> - **Offline Productivity Tracker**: A brief session of 0.55 minutes with Chrome.
>
> Summary: The day consisted primarily of Rust programming and light web browsing.

---

## AI Pipeline

### Feature Toggle

In `tracker.toml`:
```toml
[ai_analyzer]
enabled = false   # tracker works fully without Python/AI
```

When **disabled**: `tracker report-ai` exits with a message. The Rust tracking loop is completely unaffected.

When **enabled**: `tracker report-ai` runs `py-analyzer/analyzer.py` as a subprocess, forwarding:
- `TRACKER_AI_OLLAMA_HOST` — Ollama endpoint
- `TRACKER_AI_MODEL` — LLM model
- `TRACKER_AI_OUTPUT_DIR` — output directory
- `TRACKER_SESSION_DIR` + `TRACKER_NORMALIZED_FILE` — session data source

The Python `AnalyzerConfig` class reads these env vars with fallbacks for Docker compatibility.

### Guardrails

`guardrails.py` strips judgment words: `productive`, `lazy`, `efficient`, `inefficient`, `excellent`, `bad`.

---

## Testing

```bash
cd rust-tracker
cargo test
```

20 tests across enrichment, activity grouping, git analysis, and terminal classification.

---

## License

This project is licensed under the MIT License.

---

## How to Extend

1. **New collector**: create in `collector/{platform}/{source}.rs`, add to module, wire in `main.rs`
2. **New config field**: add field to `config/settings.rs`, default value function, env override in `config/mod.rs`
3. **New enrichment field**: add to `EnrichedEvent` in `models/enriched.rs`, extraction in `processing/enrich.rs`
4. **New workflow type**: add variant to `TerminalWorkflow` enum, base commands to `classify_command`
