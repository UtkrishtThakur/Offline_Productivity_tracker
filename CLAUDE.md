# CLAUDE.md — Architecture Guide

**Project:** `rust-tracker` — Local-first deterministic desktop telemetry and activity reconstruction engine.  
**Location:** `/mnt/ai/Projects/tracker` (workspace root), `rust-tracker/` (Rust project root)  
**License/Maturity:** Early stage — working prototype with centralized configuration.

---

## Core Philosophy (do not violate)

- **Metadata only** — no screenshots, no OCR, no keylogging, no content capture
- **Deterministic** — all enrichment is pure string parsing, no ML/AI heuristics
- **Local-first** — all data stays on the machine, no telemetry egress
- **Explainable** — every transformation is auditable in code
- **No scoring** — this is not productivity tracking. Reports describe *what happened*, not *how good/bad it was*.

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────────┐
│                     config/mod.rs + settings.rs                     │
│               tracker.toml loading & env overrides                  │
└────────────────────────────────┬───────────────────────────────────┘
                                 │ AppContext
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                          main.rs                                   │
│                   Orchestration & Loop                             │
└──────┬──────────┬──────────┬──────────┬───────────┬──────────────┘
       │          │          │          │           │
       ▼          ▼          ▼          ▼           ▼
┌──────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐
│ Window   │ │ Idle   │ │Terminal│ │ Git    │ │ CLI      │
│ Collector│ │Collector│ │Collector│ │Collector│ │(clap)   │
└────┬─────┘ └───┬────┘ └───┬────┘ └───┬────┘ └──────────┘
     │           │          │          │
     ▼           ▼          ▼          ▼
┌────────────────────────────────────────────────────────────────────┐
│                        SessionManager                              │
│              (session/manager.rs)                                  │
│                                                                     │
│  Owns: ActivityGrouper, Logger, last_event buffer                  │
│  Merges consecutive identical windows                              │
│  Routes events → enrichment → grouper                             │
│  Handles idle boundaries                                           │
└──────┬─────────────────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────────────────┐
│                       Enrichment Layer                             │
│              (processing/enrich.rs)                                │
│                                                                     │
│  extract_from_title() → (project, file)                            │
│  detect_language() → language                                      │
│  normalize_app_name() → canonical app name                         │
│  enrich_event(Event) → EnrichedEvent                               │
└──────┬─────────────────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────────────────┐
│                      Activity Grouper                              │
│              (processing/activity.rs)                              │
│                                                                     │
│  Groups by (project, app) key                                      │
│  Configurable adjacency window                                     │
│  Idle splits flush groups                                          │
│  Pushes completed ActivityGroup to storage                         │
└──────┬─────────────────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────────────────┐
│                      Storage Layer (Logger struct)                  │
│              (storage/logger.rs)                                   │
│                                                                     │
│  Configurable session_dir, events_file, normalized_file            │
│    ├── events.jsonl              (raw Event lines)                 │
│    └── normalized_sessions.jsonl  (ActivityGroup lines)            │
│                                                                     │
│  read_normalized_sessions() → Vec<ActivityGroup>                   │
└──────┬─────────────────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────────────────┐
│                      Report Layer                                  │
│              (processing/summary.rs)                               │
│                                                                     │
│  format_report(&[ActivityGroup]) → human-readable string           │
│  No judgment, no scores, no AI                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Config System (`config/`)

### Files

| File | Purpose |
|------|---------|
| `config/mod.rs` | `AppContext`, config loader, env override logic, `write_default()`, `ConfigError` |
| `config/settings.rs` | Typed structs: `TrackerConfig`, `TrackingConfig`, `StorageConfig`, `AiAnalyzerConfig`, `LoggingConfig` |

### Config resolution order

1. **CLI `--config` flag** — explicit path, must exist or errors
2. **`TRACKER_CONFIG`** env var — explicit path
3. **`./tracker.toml`** — current working directory
4. **Platform config dir** — `~/.config/tracker/tracker.toml` (Linux) or `%APPDATA%/tracker/tracker.toml` (Windows)
5. **Built-in defaults** — no file required, all values have sane defaults

### Environment variable overrides

Every config field can be overridden at runtime via `TRACKER_*` env variables:

| Env var | Config field | Example |
|---------|-------------|---------|
| `TRACKER_POLL_INTERVAL_SEC` | `tracking.poll_interval_sec` | `5` |
| `TRACKER_IDLE_THRESHOLD_SEC` | `tracking.idle_threshold_sec` | `180` |
| `TRACKER_ADJACENCY_WINDOW_SEC` | `tracking.adjacency_window_sec` | `600` |
| `TRACKER_SESSION_DIR` | `storage.session_dir` | `/data/sessions` |
| `TRACKER_EVENTS_FILE` | `storage.events_file` | `raw.jsonl` |
| `TRACKER_NORMALIZED_FILE` | `storage.normalized_file` | `sessions.jsonl` |
| `TRACKER_AI_ENABLED` | `ai_analyzer.enabled` | `true` |
| `TRACKER_AI_OLLAMA_HOST` | `ai_analyzer.ollama_host` | `http://10.0.0.1:11434` |
| `TRACKER_AI_MODEL` | `ai_analyzer.model` | `llama3` |
| `TRACKER_AI_OUTPUT_DIR` | `ai_analyzer.output_dir` | `./summaries` |
| `TRACKER_LOG_LEVEL` | `logging.log_level` | `debug` |

### Config file format (TOML)

See `tracker.toml` at workspace root for the authoritative reference.

### Default config generation

```bash
tracker init-config
# Writes tracker.toml to CWD with all defaults and documentation
```

### `AppContext`

Holds the parsed `TrackerConfig` and config path. Created once at startup:

```rust
let ctx = config::AppContext::load(cli.config.as_deref())?;
// Access:
ctx.config.tracking.poll_interval_sec
ctx.config.storage.session_dir
ctx.config.ai_analyzer.enabled
```

---

## Runtime Flow

### Tracking Loop (`main.rs:run_tracking_loop`)

Polls every `tracking.poll_interval_sec` (default 3s). All thresholds read from config:

```
loop (every poll_interval_sec):
    1. Window info         (get_active_window)
    2. Idle time           (get_idle_ms)
    3. Terminal command    (get_latest_command)
    4. Git activity        (get_git_activity)

    ├── IDLE CHECK ─────────────────────────────────────────
    │   if idle >= idle_threshold_sec (default 120):
    │       1. Flush current window session (with elapsed duration)
    │       2. manager.emit_idle(idle_sec)
    │          ├── logs idle_session event
    │          └── grouper.split_on_idle() — finalizes current group
    │       3. Sleep idle_sleep_sec (default 2s), continue
    │
    ├── WINDOW TRACKING ────────────────────────────────────
    │   On window change (app::title differs from last):
    │       1. manager.process_window_session(...)
    │          ├── if duration < min_meaningful_sec (default 2s): ignore
    │          ├── if matches last_event: merge (extend duration)
    │          └── else:
    │              1. flush old last_event
    │              2. create new last_event
    │
    ├── TERMINAL TRACKING ──────────────────────────────────
    │   On new command:
    │       1. classify_command(cmd) → TerminalWorkflow enum
    │       2. logger.log_event("terminal_command", ...)
    │       3. manager.push_terminal_workflow(workflow.label())
    │
    ├── GIT TRACKING ───────────────────────────────────────
    │   On git state change:
    │       1. logger.log_event("git_activity", ...)
    │       2. build_git_summary(&json) → Option<GitSummary>
    │       3. manager.push_git_summary(summary)
    │
    └── sleep(poll_interval_sec)

On Ctrl+C:
    manager.finalize()
        ├── flush() — flush pending last_event
        ├── grouper.finalize_all() — finalize current + drain completed
        └── logger.log_normalized_session() for each group
```

### Tracking Loop (`main.rs:run_tracking_loop`)

Polls every `tracking.poll_interval_sec` (default 3s). All thresholds read from config:

```
loop (every poll_interval_sec):
    [DAY ROTATION]
    if today != current_date:
        1. manager.finalize() — flush pending
        2. Read all groups from normalized_sessions.jsonl
        3. Filter groups matching previous date
        4. archiver.finalize_day(date, groups, storage)
           ├── write deterministic.txt
           ├── write metadata.json (status=finalized)
           └── if auto_cleanup: rewrite JSONL files without that day
        5. If AI enabled:
           ├── invoke_ai_analyzer(ctx, Some(&date))
           │   └── on success: update metadata.json (semantic_summary)
           └── on failure: push PendingAiJob to retry queue
        6. current_date = today
        7. manager = fresh SessionManager

    [AI RETRY QUEUE]
    if ai_enabled && pending_ai not empty:
        for each pending job where last_attempt + retry_delay has passed:
            retry AI analyzer
            on success: update metadata, remove from queue
            on failure: increment retry_count; if max retries reached,
                        mark metadata as failed, remove from queue

    [COLLECTORS] (unchanged)
    1. Window info, idle, terminal, git
    2. Idle check, window tracking, terminal, git
    └── sleep(poll_interval_sec)

On Ctrl+C:
    manager.finalize() — flush remaining (no forced rotation)
```

## CLI Commands (`cli/commands.rs`)

| Command | Behavior |
|---------|----------|
| `tracker [--config PATH] Start` | Logs `tracker_started`, archives pending days, enters tracking loop |
| `tracker [--config PATH] Pause` | Logs `tracker_paused` event |
| `tracker [--config PATH] Resume` | Logs `tracker_resumed` event |
| `tracker [--config PATH] Stop` | Logs `tracker_stopped` event |
| `tracker [--config PATH] Report` | Reads `normalized_sessions.jsonl` → `format_report()` → stdout |
| `tracker [--config PATH] ReportAi` | Invokes `py-analyzer/analyzer.py` subprocess (if `ai_analyzer.enabled`) |
| `tracker InitConfig` | Writes default `tracker.toml` to CWD |

Note: Pause/Resume/Stop currently only log events — they don't actually pause/stop the running loop. This is an intended extension point.

On `Start`, the system first calls `archiver.archive_pending_days()` which reads the entire normalized log, groups entries by date, and archives any complete day that does not yet have a summary. This ensures crash recovery across restarts.

---

## Data Model

### Raw Event (`models/event.rs`)

```rust
pub struct Event {
    pub timestamp: String,        // RFC 3339
    pub event_type: String,       // "window_session" | "terminal_command" | "git_activity" | ...
    pub source: String,           // "window_tracker" | "terminal_tracker" | ...
    pub app: Option<String>,      // raw window application name
    pub title: Option<String>,    // raw window title
    pub workspace: Option<i64>,   // workspace/desktop number
    pub duration_sec: Option<u64>,// time spent on this event
    pub data: Value,              // arbitrary JSON payload
}
```

### Enriched Event (`models/enriched.rs`)

```rust
pub struct EnrichedEvent {
    #[serde(flatten)]
    pub event: Event,
    pub project: Option<String>,
    pub file: Option<String>,
    pub language: Option<String>,
    pub normalized_app: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
}
```

### Git Summary (`models/activity.rs`)

```rust
pub struct GitSummary {
    pub repo: String,
    pub branch: String,
    pub commit_count: u32,
    pub unpushed: u32,
    pub changed_files: Vec<String>,
    pub dev_areas: Vec<String>,
}
```

### Activity Group (`models/activity.rs`)

```rust
pub struct ActivityGroup {
    pub start_time: String,
    pub end_time: String,
    pub project: Option<String>,
    pub app: String,
    pub total_duration_sec: u64,
    pub files_touched: Vec<String>,
    pub languages: Vec<String>,
    pub terminal_workflows: Vec<String>,
    pub git_summary: Option<GitSummary>,
}
```

---

## Normalization Pipeline (raw window title → structured context)

The enrichment system in `processing/enrich.rs` converts raw telemetry into structured, deterministic context:

### 1. Title Parsing (project & file extraction)

```
Input: "tracker - Antigravity - main.rs"
               │            │         │
               ▼            ▼         ▼
          project        app name   file name
          candidate     (filtered)  candidate

Algorithm:
  1. Split on " - " and " — " delimiters
  2. Scan segments for "looks like a file" (contains a dot + known extension)
  3. Scan remaining segments for "looks like a project" (not app name, not file, not filler)
  4. Return (project, file)
```

### 2. Language Detection

Pure linear scan over static `LANGUAGE_MAP` slice (44 entries). O(n) but trivially fast.

### 3. App Name Normalization

Two-pass: exact match → case-insensitive substring match → lowercase fallback. Covers variants like "Firefox ESR", "Google Chrome", "firefox-developer-edition".

---

## Session Reconstruction Algorithm (`processing/activity.rs`)

### Grouping Key

```
GroupKey = (project: Option<String>, app: String)
```

### Adjacency Window (configurable, default 300s / 5 minutes)

Events matching the current `GroupKey` with duration < `adjacency_window_sec` are merged:
- `total_duration_sec` accumulates
- `files_touched` deduplicated append
- `languages` deduplicated append

### Idle Splitting

```
manager.emit_idle(n)  →  grouper.split_on_idle()
                              └── finalize_current()
                                    └── if duration > 0: push to completed
next event → fresh group
```

### Terminal & Git Side-Channels

Injected into the *current* group via:
- `grouper.push_terminal_workflow(label)` — deduplicated
- `grouper.push_git(GitSummary)` — latest state wins

---

## Collector Layer

### Linux (`collector/linux/`)

| Collector | Source | Returns |
|-----------|--------|---------|
| `window.rs` | `~/.config/gnomectl/activewindow.json` | `WindowInfo { app, title, workspace }` |
| `idle.rs` | `xprintidle` command | `Option<u64>` (milliseconds) |
| `terminal.rs` | `~/.bash_history` | `Option<String>` (last command) |
| `git.rs` | `git` commands in CWD | `Option<Value>` (JSON) |
| `browser.rs` | — | Placeholder |

### Windows (`collector/windows/`)

| Collector | Source | Status |
|-----------|--------|--------|
| `window.rs` | Win32 `GetForegroundWindow` + `GetWindowText` + `GetWindowThreadProcessId` | Implemented |
| `idle.rs` | Win32 `GetLastInputInfo` | Implemented |
| `terminal.rs` | PowerShell history | Implemented |
| `git.rs` | `git status --porcelain` | Implemented |
| `browser.rs` | — | Stub |

---

## Terminal Workflow Classification (`processing/terminal.rs`)

Pure prefix matching against base command token:

| Workflow | Matching Commands |
|----------|------------------|
| `RustBuild` | cargo, rustc, rustup, clippy |
| `GitCommit` | git |
| `NodeJs` | npm, npx, yarn, pnpm, node, bun, deno |
| `Python` | python, python3, pip, pip3, pytest, poetry, pdm, uv, conda, virtualenv, venv |
| `Docker` | docker, docker-compose, podman |
| `FileNavigation` | cd, ls, ll, la, cat, less, head, tail, find, fd, tree, mkdir, rmdir, cp, mv, ln, pwd, exa, bat, rg |
| `SystemAdmin` | sudo, systemctl, journalctl, apt, pacman, dnf, yum, brew, snap, flatpak, chmod, chown, kill, ps, top, htop, df, du, mount, umount, ssh, scp |
| `Unknown` | everything else |

---

## Git Reconstruction (`processing/git.rs`)

`build_git_summary()` expects JSON shape from git collector:

```json
{
  "repo": "/mnt/ai/Projects/tracker",
  "branch": "main",
  "changed_files": ["M src/main.rs"],
  "changed_count": 2,
  "last_commit_hash": "abc123",
  "last_commit_message": "message",
  "unpushed_commits": 2
}
```

`detect_dev_areas()` extracts parent directories from git porcelain-format file paths.

---

## Storage Layer (`storage/logger.rs`)

### Logger struct

```rust
pub struct Logger {
    pub session_dir: PathBuf,
    pub events_path: PathBuf,
    pub normalized_path: PathBuf,
}
```

Created from `StorageConfig`. All paths are configurable via `tracker.toml` or env vars.

### Methods

- `log_event()` — serialize `Event` to JSON, append to `events.jsonl`
- `log_normalized_session()` — serialize `ActivityGroup` to JSON, append to `normalized_sessions.jsonl`
- `read_normalized_sessions()` — read and deserialize all lines from `normalized_sessions.jsonl`

---

## AI Analyzer Integration (`py-analyzer/`)

### Flow

```
normalized_sessions.jsonl
         │
         ▼
  formatter.py ─── build_ai_context(cfg)
         │          aggregates by project
         ▼
  prompts.py ───── SYSTEM_PROMPT + USER_PROMPT_TEMPLATE
         ▼
  analyzer.py ──── ollama.chat(model, messages)
         │
         ▼
  guardrails.py ── sanitize_output() — strips banned judgment words
         │
         ▼
  daily_writer.py ── write_summary(text, cfg) → outputs/{date}.txt
```

### Configuration

The Python analyzer reads from environment variables (same `TRACKER_AI_*` prefix as Rust):

| Python config field | Env var | Default |
|--------------------|---------|---------|
| `normalized_log` | `TRACKER_NORMALIZED_LOG` / `NORMALIZED_LOG` | `../sessions/active_session/normalized_sessions.jsonl` |
| `output_dir` | `TRACKER_AI_OUTPUT_DIR` / `OUTPUT_DIR` | `outputs` |
| `ollama_host` | `TRACKER_AI_OLLAMA_HOST` / `OLLAMA_HOST` | `http://localhost:11434` |
| `model` | `TRACKER_AI_MODEL` / `MODEL` | `qwen2.5:7b` |
| `enabled` | `TRACKER_AI_ENABLED` / `AI_ENABLED` | `true` |

### Feature Toggle

In `tracker.toml`:
```toml
[ai_analyzer]
enabled = false   # tracker works fully without Python/AI
```

When disabled:
- `tracker report-ai` exits with a message
- `tracker report` shows a note that AI is disabled
- The Rust tracking loop is completely unaffected

When enabled:
- `tracker report-ai` runs `py-analyzer/analyzer.py` as a subprocess
- Config values (model, host, paths) are forwarded as env vars

### Docker

```bash
cd py-analyzer
docker compose up -d
```

The container mounts `../sessions:/sessions` and reads `TRACKER_SESSION_DIR=/sessions/active_session`. Ollama runs on the host via `network_mode: "host"`.

---

## Subsystem Dependency Graph

```
main.rs
  ├── config              (settings + mod — loaded first)
  │     ├── settings.rs   (TrackerConfig structs)
  │     └── mod.rs        (AppContext, loader, env overrides)
  ├── cli::commands       (CLI argument parsing)
  ├── collector::linux::* (OS telemetry)
  ├── session::manager    (depends on: config, models, processing, storage::logger)
  │     ├── config::TrackerConfig
  │     ├── models::event, models::activity
  │     ├── processing::activity  (ActivityGrouper, config-aware)
  │     ├── processing::enrich    (enrich_event)
  │     └── storage::logger       (Logger struct)
  ├── processing::git     (depends on: models::activity)
  ├── processing::terminal (no deps)
  ├── processing::summary  (depends on: models::activity)
  └── storage::logger     (depends on: config::StorageConfig, models)
```

---

## Daily Rotation & Archival (`storage/archiver.rs`, `processing/daily.rs`)

### `processing/daily.rs`
- `format_timeline_summary(groups, date)` → chronological deterministic timeline text
- `extract_date(rfc3339)` → `YYYY-MM-DD` string for grouping

### `storage/archiver.rs`
- `Archiver` struct manages per-day summary directories under `summaries_dir`
- `finalize_day()` orchestrates: write deterministic.txt → write metadata.json → optional cleanup
- `archive_pending_days()` startups: scans all normalized sessions, archives any completed day without a summary
- `cleanup_day_logs()` atomically rewrites JSONL (tmp+rename) to remove a finalized day's entries
- `DayMetadata` tracks status, retry_count, errors, and per-day aggregate stats

### Summary directory layout
```
summaries/YYYY-MM-DD/
  deterministic.txt   # always — timeline from format_timeline_summary()
  semantic.txt        # only when AI succeeds (written by py-analyzer)
  metadata.json       # always — status, stats, error info
```

### Cleanup semantics
After a day is finalized with `status=finalized`:
- `normalized_sessions.jsonl` and `events.jsonl` are rewritten (tmp+rename) without that day's entries
- This keeps active logs small — only the current day's data remains

### AI retry queue
- On day rotation, if AI is enabled and `invoke_ai_analyzer` fails, a `PendingAiJob` is queued
- Each loop iteration, pending jobs with elapsed `retry_delay` are retried
- After `retry_attempts` failures, metadata is marked `status=failed` and the job is dropped

## Placeholder / Empty Modules (Extension Points)

| Module | File | Status | Intended Purpose |
|--------|------|--------|------------------|
| `utils/paths.rs` | `utils/paths.rs` | Empty | Path resolution utilities |
| `utils/time.rs` | `utils/time.rs` | Empty | Time formatting, duration utilities |
| `storage/schema.rs` | `storage/schema.rs` | Empty | Schema versioning, migration support |
| `collector/linux/browser.rs` | `collector/linux/browser.rs` | Empty | Browser tab/URL tracking |
| `collector/windows/browser.rs` | `collector/windows/browser.rs` | Stub | Browser tab/URL tracking on Windows |

Note: `config/` is no longer a placeholder — it's fully implemented.

---

## Configuration Constants — Now in `tracker.toml`

| Config key | Default | Description |
|-----------|---------|-------------|
| `tracking.poll_interval_sec` | 3 | Main loop sleep duration |
| `tracking.idle_sleep_sec` | 2 | Sleep during idle periods |
| `tracking.idle_threshold_sec` | 120 | Idle time before splitting groups |
| `tracking.adjacency_window_sec` | 300 | Max gap between merged events |
| `tracking.min_meaningful_sec` | 2 | Minimum window duration to be meaningful |
| `storage.session_dir` | `../sessions/active_session` | Session data directory |
| `storage.events_file` | `events.jsonl` | Raw event file name |
| `storage.normalized_file` | `normalized_sessions.jsonl` | Normalized session file name |
| `ai_analyzer.enabled` | `false` | Enable AI summary generation |
| `ai_analyzer.ollama_host` | `http://localhost:11434` | Ollama API endpoint |
| `ai_analyzer.model` | `qwen2.5:7b` | LLM model |
| `ai_analyzer.output_dir` | `outputs` | AI summary output directory |
| `logging.enable_file_logging` | `true` | Write log files |
| `logging.log_level` | `info` | Log verbosity |

---

## Test Coverage

Test modules in:
- `processing/activity.rs` — ActivityGroup merging, splitting, noise filtering
- `processing/enrich.rs` — Language detection, app normalization, title parsing
- `processing/git.rs` — Summary building, dev area detection, commit burst detection
- `processing/terminal.rs` — Command classification, workflow detection, deduplication

Run tests from `rust-tracker/`:
```bash
cargo test
```

---

## Design Decisions & Rationale

### Why TOML for config?
TOML is the standard for Rust projects (used by Cargo itself). It's readable, typed, and has first-class serde support.

### Why `Logger` struct instead of free functions?
Encapsulating paths in a struct eliminates duplicated path logic, makes the config injection explicit, and enables future features like log rotation or multi-session logging.

### Why env var overrides in addition to config file?
Containerized deployments (Docker, Kubernetes) pass config through environment variables. The `TRACKER_*` prefix avoids collisions with system vars.

### Why AI analyzer as a separate Python process?
Keeps the AI dependency (Ollama, Python runtime, large models) completely optional. The Rust core is self-contained for deterministic tracking. Python is only needed for AI summaries.

### Why flat `Vec<ActivityGroup>` instead of a tree/relational model?
Activity reconstruction is fundamentally flat — a sequence of user attention spans.

### Why JSONL instead of SQLite/sled?
Trivially inspectable with `cat`/`jq`/`tail`. No schema migration. Atomic appends. Acceptable O(n) read for local desktop use.

### Why `#[serde(flatten)]` on `EnrichedEvent.event`?
Flat JSON output matching expected schema for downstream consumers.

### Why compile-time platform selection instead of trait objects?
Simpler at current scale (2 platforms, <1200 LOC). Extract a `TelemetryCollector` trait when more platforms are added.

---

## Important Implementation Notes

### The `WindowInfo` Problem
Two identical `WindowInfo` structs exist (linux and windows). Currently `get_platform_telemetry()` on Windows returns `None`. Fix: extract `WindowInfo` into `models/` and share.

### The `last_event` Merge Semantics
`SessionManager::process_window_session` compares against `self.last_event`. Matches on (app, title, workspace) — durations are merged. This means rapid alt-tab switches between the same app+title produce a single merged session.

### Idle Detection Edge Case
The idle detector transitions from `!idle_active` to `idle_active` on first crossing of threshold. During idle, window polling is skipped. The first non-idle poll relies on `last_window` comparison and `focus_start` timer for accurate duration.

### Threading Model
Single-threaded synchronous loop. `ctrlc` sets an `AtomicBool` flag. No async, no channels, no locks. Collector calls are fast I/O (< 50ms).

### Config Loading
Config is loaded once at startup. Changes to `tracker.toml` require a restart. Environment variables are read at startup only.
