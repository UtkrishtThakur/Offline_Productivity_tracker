# CLAUDE.md — Architecture Guide

**Project:** `rust-tracker` — Local-first deterministic desktop telemetry and activity reconstruction engine.  
**Location:** `/mnt/ai/Projects/tracker` (workspace root), `rust-tracker/` (Rust project root)  
**License/Maturity:** Very early stage — working prototype with empty placeholder modules.

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
│                        SessionManager                             │
│              (session/manager.rs)                                  │
│                                                                     │
│  Owns: ActivityGrouper, last_event buffer                          │
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
│  5-minute adjacency window                                         │
│  Idle splits flush groups                                          │
│  Pushes completed ActivityGroup to storage                         │
└──────┬─────────────────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────────────────┐
│                      Storage Layer                                 │
│              (storage/logger.rs)                                   │
│                                                                     │
│  ../sessions/active_session/                                       │
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

## Runtime Flow

### Tracking Loop (`main.rs:run_tracking_loop`)

Polls every **3 seconds** (hardcoded). Collects four telemetry sources in parallel:

```
loop (every 3s):
    1. Window info         (get_active_window)
    2. Idle time           (get_idle_ms)
    3. Terminal command    (get_latest_command)
    4. Git activity        (get_git_activity)

    ├── IDLE CHECK ─────────────────────────────────────────
    │   if idle >= 120s (IDLE_THRESHOLD_SEC):
    │       1. Flush current window session (with elapsed duration)
    │       2. manager.emit_idle(idle_sec)
    │          ├── logs idle_session event
    │          └── grouper.split_on_idle() — finalizes current group
    │       3. Sleep 2s, continue (skip other polling)
    │
    ├── WINDOW TRACKING ────────────────────────────────────
    │   On window change (app::title differs from last):
    │       1. manager.process_window_session(app, title, workspace, elapsed_sec)
    │          ├── if duration < 2s: ignore (noise filter)
    │          ├── if matches last_event: merge (extend duration)
    │          └── else:
    │              1. flush old last_event
    │                 ├── log_event (raw JSONL)
    │                 └── enrich_event → grouper.push_enriched
    │              2. create new last_event
    │
    ├── TERMINAL TRACKING ──────────────────────────────────
    │   On new command (differs from last):
    │       1. classify_command(cmd) → TerminalWorkflow enum
    │       2. log_event("terminal_command", ...)
    │       3. manager.push_terminal_workflow(workflow.label())
    │
    ├── GIT TRACKING ───────────────────────────────────────
    │   On git state change:
    │       1. log_event("git_activity", ...)
    │       2. build_git_summary(&json) → Option<GitSummary>
    │       3. manager.push_git_summary(summary)
    │
    └── sleep(3s)

On Ctrl+C:
    manager.finalize()
        ├── flush() — flush pending last_event
        ├── grouper.finalize_all() — finalize current + drain completed
        └── log_normalized_session for each group
```

### CLI Commands (`cli/commands.rs`)

| Command | Behavior |
|---------|----------|
| `tracker Start` | Logs `tracker_started` event, enters tracking loop |
| `tracker Pause` | Logs `tracker_paused` event |
| `tracker Resume` | Logs `tracker_resumed` event |
| `tracker Stop` | Logs `tracker_stopped` event |
| `tracker Report` | Reads `normalized_sessions.jsonl` → `format_report()` → stdout |

Note: Pause/Resume/Stop currently only log events — they don't actually pause/stop the running loop (the loop runs synchronously in `Start`). This is an intended extension point.

---

## Data Model

### Raw Event (`models/event.rs`)

```rust
pub struct Event {
    pub timestamp: String,        // RFC 3339
    pub event_type: String,       // "window_session" | "terminal_command" | "git_activity" | "idle_session" | "tracker_started" | ...
    pub source: String,           // "window_tracker" | "terminal_tracker" | "git_tracker" | "idle_tracker" | "system"
    pub app: Option<String>,      // raw window application name
    pub title: Option<String>,    // raw window title
    pub workspace: Option<i64>,   // workspace/desktop number
    pub duration_sec: Option<u64>,// time spent on this event
    pub data: Value,              // arbitrary JSON payload (git data, command text, etc.)
}
```

### Enriched Event (`models/enriched.rs`)

```rust
pub struct EnrichedEvent {
    #[serde(flatten)]
    pub event: Event,             // the original event (fields merged at top level)
    pub project: Option<String>,  // determined from window title parsing
    pub file: Option<String>,     // active file from window title
    pub language: Option<String>, // mapped from file extension
    pub normalized_app: String,   // canonical app name (e.g. "vscode" not "Code - OSS")
    pub repo: Option<String>,     // extracted from event data
    pub branch: Option<String>,   // extracted from event data
}
```

### Git Summary (`models/activity.rs`)

```rust
pub struct GitSummary {
    pub repo: String,
    pub branch: String,
    pub commit_count: u32,
    pub unpushed: u32,
    pub changed_files: Vec<String>,  // git porcelain format entries
    pub dev_areas: Vec<String>,      // parent directories of changed files
}
```

### Activity Group (`models/activity.rs`)

```rust
pub struct ActivityGroup {
    pub start_time: String,              // RFC 3339 of first event in group
    pub end_time: String,                // RFC 3339 when group was finalized
    pub project: Option<String>,
    pub app: String,                     // normalized app name
    pub total_duration_sec: u64,         // sum of constituent event durations
    pub files_touched: Vec<String>,      // deduplicated
    pub languages: Vec<String>,          // deduplicated
    pub terminal_workflows: Vec<String>, // deduplicated workflow labels
    pub git_summary: Option<GitSummary>,
}
```

---

## Normalization Pipeline (raw window title → structured context)

The enrichment system in `processing/enrich.rs` converts raw telemetry into structured, deterministic context through three parallel transforms:

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

The `looks_like_file()` function requires:
- A dot character (`.`) in the segment
- An extension of 1–10 characters after the dot
- The extension must appear in `LANGUAGE_MAP`

### 2. Language Detection

```
Filename: "main.rs"
            │  │
            │  └── extension → lookup in LANGUAGE_MAP
            │                   (55 entries: rs→rust, py→python, ...)
            └── result: "rust"
```

Pure linear scan over a static slice. O(n) but trivially fast for 55 entries.

### 3. App Name Normalization

```
Raw: "Code - OSS"  →  exact match: "vscode"
Raw: "firefox"     →  exact match: "firefox"
Raw: "unknown"     →  fallback: lowercase + trim → "unknown"
```

Two-pass strategy:
1. **Exact match** against `APP_NAME_MAP` (38 entries)
2. **Case-insensitive substring match** against normalized keys
3. **Fallback** — lowercase + trim the raw string

Design rationale: app names vary across OS versions, distributions, and installations. Substring matching catches variants like "Firefox ESR", "Google Chrome", "firefox-developer-edition" without needing exhaustive enumeration.

---

## Session Reconstruction Algorithm (`processing/activity.rs`)

### Grouping Key

```
GroupKey = (project: Option<String>, app: String)
```

Every enriched event is bucketed by this key. When the key changes, the current group is finalized.

### Adjacency Window (300 seconds / 5 minutes)

If an enriched event matches the current `GroupKey` AND its duration is less than `ADJACENCY_WINDOW_SEC`, it's **merged** into the current group:

- `total_duration_sec` accumulates
- `files_touched` gets deduplicated append
- `languages` gets deduplicated append

If duration exceeds `ADJACENCY_WINDOW_SEC` (theoretical — in practice a single poll is 3s), the group is finalized and a new one starts.

### Idle Splitting

```
manager.emit_idle(120)  →  grouper.split_on_idle()
                                  │
                                  └── finalize_current()
                                        │
                                        └── if duration > 0:
                                              push ActivityGroup to completed

next event → fresh group
```

This ensures that periods of activity separated by >= 2 minutes of idle produce distinct `ActivityGroup` records.

### Terminal & Git Side-Channels

Terminal workflows and git summaries are **not grouped by window focus** — they are injected into the *current* group via:
- `grouper.push_terminal_workflow(label)` — deduplicated ordered list
- `grouper.push_git(GitSummary)` — replaces previous (latest state wins)

This means the grouper produces: `ActivityGroup { ..., terminal_workflows: ["Rust build workflow", "Git commit workflow"], git_summary: Some(...), }`

### Deterministic Guarantee

Given the same sequence of raw events, the grouping algorithm always produces the same `Vec<ActivityGroup>`. No randomness, no ML, no external state.

---

## Collector Layer

### Linux (`collector/linux/`)

| Collector | Source | Returns | Notes |
|-----------|--------|---------|-------|
| `window.rs` | `~/.config/gnomectl/activewindow.json` | `WindowInfo { app, title, workspace }` | Depends on `gnomectl` being installed; reads a JSON file written by a companion tool |
| `idle.rs` | `xprintidle` command | `Option<u64>` (milliseconds) | Depends on `xprintidle` being installed |
| `terminal.rs` | `~/.bash_history` | `Option<String>` (last command) | Reads entire file, takes last line; naive approach for now |
| `git.rs` | `git` commands in CWD | `Option<Value>` (JSON) | Runs `git rev-parse --show-toplevel`, `git rev-parse --abbrev-ref HEAD`, `git status --porcelain`, `git log -1`, `git cherry -v` |
| `browser.rs` | — | — | Placeholder (empty) |

### Windows (`collector/windows/`)

| Collector | Source | Status |
|-----------|--------|--------|
| `window.rs` | Win32 `GetForegroundWindow` + `GetWindowText` + `GetWindowThreadProcessId` | Implemented but returns `None` in platform-agnostic path |
| `idle.rs` | Win32 `GetLastInputInfo` | Implemented |
| `terminal.rs` | PowerShell history file | Implemented |
| `git.rs` | `git status --porcelain` | Implemented |
| `browser.rs` | — | Stub (`// Unimplemented`) |

### Platform Selection (`main.rs:get_platform_telemetry`)

Uses `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "windows")]` for compile-time platform selection. Currently the platform-agnostic path only works on Linux (Windows path returns all `None` due to `WindowInfo` type mismatch).

---

## Terminal Workflow Classification (`processing/terminal.rs`)

Pure prefix matching against the base command (first whitespace-delimited token):

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

`detect_workflows()` also deduplicates consecutive same-type commands (similar to `uniq`).

---

## Git Reconstruction (`processing/git.rs`)

### `build_git_summary`

Expects JSON shape from the git collector:

```json
{
  "repo": "/mnt/ai/Projects/tracker",
  "branch": "main",
  "changed_files": ["M src/main.rs", "M src/session/manager.rs"],
  "changed_count": 2,
  "last_commit_hash": "abc123",
  "last_commit_message": "message",
  "unpushed_commits": 2
}
```

### `detect_dev_areas`

Parses git porcelain format lines:
```
"M src/main.rs"  →  split on whitespace, take last  →  "src/main.rs"
                                                  →  parent directory  →  "src"
```

Produces deduplicated list of directories being actively edited. Root-level files produce no dev area entry.

---

## Storage Layer (`storage/logger.rs`)

### File Layout

```
../sessions/
└── active_session/
    ├── events.jsonl               ← Raw Event structs, one JSON object per line
    └── normalized_sessions.jsonl  ← ActivityGroup structs, one JSON object per line
```

### Write Strategy

- `log_event()`: Serializes `Event` to JSON, appends to `events.jsonl`. Called for every telemetry data point.
- `log_normalized_session()`: Serializes `ActivityGroup` to JSON, appends to `normalized_sessions.jsonl`. Called when groups are completed (idle flush, finalize).

### Read Strategy

- `read_normalized_sessions()`: Reads entire `normalized_sessions.jsonl`, deserializes each line. Returns `Vec<ActivityGroup>`. Returns empty vec if file doesn't exist.

### Path Convention

Path is hardcoded as `../sessions/active_session` (relative to the Rust binary, i.e. `rust-tracker/../sessions/active_session` = `sessions/active_session` at workspace root). `fs::create_dir_all` ensures directory exists before every write.

---

## Report Layer (`processing/summary.rs`)

Formats `ActivityGroup` into a human-readable block:

```
--- Activity Reconstruction Report ---

Used vscode in:
tracker

Worked on:
- main.rs
- manager.rs

Time spent:
3 minutes, 15 seconds

Terminal activity:
- Rust build workflow
- Git commit workflow

Git activity:
- 2 commits
- Development areas: src, src/session

---
```

The format is intentionally neutral — factual reconstruction, no scores, no efficiency metrics, no judgment.

---

## Subsystem Dependency Graph

```
main.rs
  ├── cli::commands       (CLI argument parsing — no deps on rest)
  ├── collector::linux::* (OS telemetry — no deps on other modules)
  ├── session::manager    (depends on: models, processing, storage)
  │     ├── models::event
  │     ├── models::activity
  │     ├── processing::activity  (ActivityGrouper)
  │     ├── processing::enrich    (enrich_event)
  │     └── storage::logger       (log_event, log_normalized_session)
  ├── processing::git     (depends on: models::activity)
  ├── processing::terminal (no deps)
  ├── processing::summary  (depends on: models::activity)
  └── storage::logger     (depends on: models)
```

---

## Placeholder / Empty Modules (Extension Points)

| Module | File | Status | Intended Purpose |
|--------|------|--------|------------------|
| `config/mod.rs` | `config/mod.rs` | Empty | Runtime configuration (poll interval, thresholds, paths) |
| `config/settings.rs` | `config/settings.rs` | Empty | Settings structs, config file parsing |
| `utils/paths.rs` | `utils/paths.rs` | Empty | Path resolution utilities |
| `utils/time.rs` | `utils/time.rs` | Empty | Time formatting, duration utilities |
| `storage/schema.rs` | `storage/schema.rs` | Empty | Schema versioning, migration support |
| `collector/linux/browser.rs` | `collector/linux/browser.rs` | Empty | Browser tab/URL tracking |
| `collector/windows/browser.rs` | `collector/windows/browser.rs` | Stub | Browser tab/URL tracking on Windows |
| `py-analyzer/` (entire dir) | various `.py` | All empty | Future Python LLM analysis layer |

---

## Configuration Constants

| Constant | Value | File | Description |
|----------|-------|------|-------------|
| `ADJACENCY_WINDOW_SEC` | 300 | `processing/activity.rs:14` | Max gap between merged events in a group |
| `MIN_MEANINGFUL_SEC` | 2 | `processing/activity.rs:18` | Minimum window duration to be meaningful |
| `IDLE_THRESHOLD_SEC` | 120 | `session/manager.rs:24` | Idle time before splitting groups |
| Poll interval | 3s | `main.rs:219` | Main loop sleep duration |

---

## Test Coverage

Test modules exist in:
- `processing/activity.rs:205` — ActivityGroup merging, splitting, noise filtering
- `processing/enrich.rs:265` — Language detection, app normalization, title parsing
- `processing/git.rs:111` — Summary building, dev area detection, commit burst detection
- `processing/terminal.rs:137` — Command classification, workflow detection, deduplication

Run tests from `rust-tracker/`: `cargo test`

---

## Design Decisions & Rationale

### Why flat `Vec<ActivityGroup>` instead of a tree/relational model?
Activity reconstruction is fundamentally flat — a sequence of user attention spans. Tree structures would overcomplicate the model for no gain at current scope.

### Why JSONL instead of SQLite/sled?
JSONL is the simplest possible append-only durable format. It enables:
- Trivially inspectable with `cat`, `jq`, `tail`
- No schema migration needed
- Easy to pipe into other tools
- Atomic appends (no locking concerns at single-writer scale)

Trade-off: no random access, no indexing. Read is O(n). Acceptable for local desktop use.

### Why separate `events.jsonl` and `normalized_sessions.jsonl`?
- `events.jsonl` is the raw audit log — every telemetry tick
- `normalized_sessions.jsonl` is the processed view — reconstructed activity blocks
- Both can be independently replayed or reprocessed

### Why `#[serde(flatten)]` on `EnrichedEvent.event`?
This makes the JSON output flat (no nested "event" key), matching the expected schema for downstream consumers. The Rust type still preserves the structural relationship.

### Why compile-time platform selection instead of trait objects?
At current scale (2 platforms, <1000 LOC), `#[cfg]` is simpler and avoids dynamic dispatch overhead. If more platforms are added, a `PlatformTelemetry` trait should be extracted.

---

## Future Roadmap

### Immediate (unblocks basic usage)
1. **Fix platform-agnostic path** — resolve `WindowInfo` type mismatch between Linux and Windows collectors
2. **Make polling interval configurable** — move from hardcoded 3s to config
3. **Implement Pause/Resume/Stop as real operations** — currently they only log events
4. **Make session path configurable** — currently hardcoded relative path

### Short-term
5. **Browser integration** — implement `collector/linux/browser.rs` to capture active tab titles
6. **Config file** — implement `config/settings.rs` for YAML/TOML config
7. **Schema versioning** — implement `storage/schema.rs` for forward-compatible storage
8. **Session archival** — move completed sessions out of `active_session/` into timestamped archives

### Medium-term
9. **`py-analyzer` integration** — LLM-powered semantic analysis reading normalized JSONL
10. **Cross-platform `WindowInfo` unification** — extract a shared `WindowInfo` into `models/` and implement a `TelemetryCollector` trait
11. **File-based project detection** — scan filesystem for project markers (Cargo.toml, .git, setup.py) to validate inferred project names
12. **Editor-agnostic project detection** — read IDE workspace files for more accurate project boundaries

### Long-term
13. **Plugin system** — loadout of collector plugins (declarative, not dynamic linking)
14. **Export formats** — JSON, CSV, markdown report generation
15. **Web dashboard** — local-first SPA reading JSONL directly
16. **Encrypted storage** — optional at-rest encryption for sensitive metadata

---

## How to Extend the Pipeline

### Add a new collector
1. Create file in `collector/{platform}/{source}.rs`
2. Implement `pub fn get_{source}() -> Option<...>`
3. Add `pub mod {source};` to `collector/{platform}/mod.rs`
4. Add the call in `main.rs:run_tracking_loop()`
5. Log the raw event with `log_event()`
6. Process/enrich via `SessionManager`

### Add a new enrichment field
1. Add field to `EnrichedEvent` in `models/enriched.rs`
2. Add extraction logic in `processing/enrich.rs:enrich_event()`
3. (Optional) Update `ActivityGroup` if the field should be aggregated

### Add a new workflow type
1. Add variant to `TerminalWorkflow` enum
2. Add `label()` match arm
3. Add base commands to `classify_command()` match

### Add a new report format
1. Create function in `processing/summary.rs` or a new module
2. Accept `&[ActivityGroup]` or `&ActivityGroup`
3. Return formatted string or structured output
4. Wire to a CLI command

---

## Important Implementation Notes

### The `WindowInfo` Problem
Two different `WindowInfo` structs exist (linux and windows), both with identical fields. They need unification. Currently `get_platform_telemetry()` on Windows returns `None` for window info because of the type mismatch. Fix: extract `WindowInfo` into `models/` and share.

### The `last_event` Merge Semantics
`SessionManager::process_window_session` compares the new event against `self.last_event`. If app, title, and workspace match, the durations are merged. If anything differs, the old event is flushed and a new one starts. This means rapid alt-tab switches between the same app+title produce a single merged session — correct and intentional.

### Idle Detection Edge Case
The idle detector transitions from `!idle_active` to `idle_active` on first crossing of threshold. It stays `idle_active` until idle drops below threshold. During idle, window polling is skipped. This means the first non-idle poll sees a PITI (point-in-time) window snapshot with unknown idle duration — the loop relies on the `last_window` comparison and `focus_start` timer to calculate accurate focus duration.

### Threading Model
Single-threaded synchronous loop. `ctrlc` sets an `AtomicBool` flag checked on each iteration. No async, no channels, no locks (except the atomic flag). This is by design — collector calls are fast I/O, not blocking operations.
