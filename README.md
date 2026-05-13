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
- **Human-readable reports** — neutral, factual summaries
- **LLM-powered daily summaries** (optional) — local AI via Ollama for narrative daily reviews
- **Cross-platform** — Linux (gnomectl/xprintidle) and Windows (Win32 API) collectors

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              main.rs                                    │
│                    Orchestration & Polling Loop (3s)                    │
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
│              (session/manager.rs)                                       │
│                                                                         │
│  Owns: last_event buffer, ActivityGrouper                               │
│  Merges identical consecutive window sessions                           │
│  Routes events → enrich → grouper                                       │
│  Handles idle boundaries & graceful shutdown                            │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Enrichment Layer                                │
│              (processing/enrich.rs)                                     │
│                                                                         │
│  extract_from_title() → project + file                                  │
│  detect_language()    → language from file extension                    │
│  normalize_app_name() → canonical app name (vscode, chrome, etc.)       │
│  enrich_event()       → Event → EnrichedEvent                           │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Activity Grouper                                 │
│              (processing/activity.rs)                                   │
│                                                                         │
│  Groups by (project, app) key                                           │
│  5-minute adjacency window for merging                                  │
│  Idle boundary splits groups (>= 2 min idle)                            │
│  Side-channels: terminal workflows + git summaries                      │
│  Pushes completed ActivityGroup to storage                              │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Storage Layer                                   │
│              (storage/logger.rs)                                        │
│                                                                         │
│  ../sessions/active_session/                                            │
│    ├── events.jsonl               (raw Event JSONL)                    │
│    └── normalized_sessions.jsonl  (ActivityGroup JSONL)                │
└───────────────────────────┬─────────────────────────────────────────────┘
                            │
        ┌───────────────────┴────────────────────┐
        ▼                                        ▼
┌───────────────────┐              ┌──────────────────────────────┐
│   Report Layer    │              │      AI Analysis Layer       │
│ (summary.rs)      │              │      (py-analyzer/)          │
│                   │              │                              │
│ format_report()   │              │ build_ai_context() → prompt  │
│ → human-readable  │              │ ollama.chat() → summary      │
│   terminal output │              │ write_summary() → daily .txt │
└───────────────────┘              └──────────────────────────────┘
```

---

## Repository Structure

```
tracker/
├── rust-tracker/                      # Core Rust tracking engine
│   ├── Cargo.toml                     # Rust deps: clap, serde, chrono, ctrlc
│   └── src/
│       ├── main.rs                    # Entrypoint, polling loop, Ctrl+C handler
│       ├── cli/commands.rs            # CLI: Start, Pause, Resume, Stop, Report
│       ├── collector/
│       │   ├── linux/                 # Linux collectors
│       │   │   ├── window.rs          # gnomectl JSON → WindowInfo
│       │   │   ├── idle.rs            # xprintidle → ms
│       │   │   ├── terminal.rs        # .bash_history → last command
│       │   │   ├── git.rs             # git commands → JSON activity
│       │   │   └── browser.rs         # PLACEHOLDER
│       │   └── windows/               # Windows collectors
│       │       ├── window.rs          # Win32 GetForegroundWindow
│       │       ├── idle.rs            # Win32 GetLastInputInfo
│       │       ├── terminal.rs        # PowerShell history
│       │       ├── git.rs             # git status --porcelain
│       │       └── browser.rs         # PLACEHOLDER
│       ├── models/
│       │   ├── event.rs               # Raw Event struct
│       │   ├── enriched.rs            # EnrichedEvent (project, file, lang, app)
│       │   └── activity.rs            # ActivityGroup, GitSummary
│       ├── session/
│       │   └── manager.rs             # SessionManager orchestrator
│       ├── processing/
│       │   ├── enrich.rs              # Title parsing, lang detection, app normalization
│       │   ├── activity.rs            # ActivityGrouper: merge, split, group
│       │   ├── terminal.rs            # Command classification (8 workflow types)
│       │   ├── git.rs                 # GitSummary builder, dev area detection
│       │   └── summary.rs            # Human-readable report formatting
│       ├── storage/
│       │   ├── logger.rs              # JSONL append/read
│       │   └── schema.rs              # PLACEHOLDER
│       ├── config/
│       │   ├── mod.rs                 # PLACEHOLDER
│       │   └── settings.rs            # PLACEHOLDER
│       └── utils/
│           ├── mod.rs
│           ├── paths.rs               # PLACEHOLDER
│           └── time.rs                # PLACEHOLDER
│
├── py-analyzer/                       # Python AI analysis layer (optional)
│   ├── analyzer.py                    # Main: reads JSONL → Ollama → daily summary
│   ├── formatter.py                   # Loads & aggregates ActivityGroup JSONL
│   ├── prompts.py                     # System + user prompt templates
│   ├── memory.py                      # SessionMemory accumulator
│   ├── guardrails.py                  # Banned word filter (no "productive", etc.)
│   ├── daily_writer.py                # Writes summaries to outputs/{date}.txt
│   ├── Dockerfile                     # python:3.12-slim container
│   ├── docker-compose.yml             # mounts sessions/, network=host for Ollama
│   ├── requirements.txt               # ollama, python-dotenv
│   └── .env                           # MODEL, OLLAMA_HOST config
│
├── sessions/                          # Runtime session data (gitignored)
│   └── active_session/
│       ├── events.jsonl               # Raw event log
│       └── normalized_sessions.jsonl   # Processed activity groups
│
├── CLAUDE.md                          # Full architecture guide (engineer-facing)
└── .gitignore
```

---

## How It Works

### Tracking Loop

The main loop in `main.rs:run_tracking_loop()` polls every **3 seconds**:

1. **Idle Detection** — If `xprintidle` reports ≥ 120s of inactivity:
   - Flush current window session
   - Split activity groups at the idle boundary
   - Log an `idle_session` event
   - Skip other collectors, sleep 2s, repeat

2. **Window Tracking** — Reads active window via `gnomectl` (Linux) or `GetForegroundWindow` (Windows):
   - On app/title change: calculate elapsed focus duration, push to `SessionManager`
   - Consecutive identical windows are **merged** (durations accumulated)
   - `< 2s` windows are filtered as noise

3. **Terminal Tracking** — Reads last command from shell history:
   - On new command: classify (`classify_command`) into one of 8 workflow types
   - Log event, push workflow label to current activity group

4. **Git Tracking** — Runs `git rev-parse`, `git status --porcelain`, `git log`, `git rev-list`:
   - On state change: log raw git event, build `GitSummary`, push to grouper

### Enrichment Pipeline

Every window event passes through `processing/enrich.rs`:

| Step | Function | Logic |
|------|----------|-------|
| App normalization | `normalize_app_name()` | Exact match → substring match → lowercase fallback |
| Title parsing | `extract_from_title()` | Split on ` - ` / ` — `, find file segment (dot + known extension), find project segment (not app, not file) |
| Language detection | `detect_language()` | File extension → lookup in 44-entry `LANGUAGE_MAP` |

### Activity Grouping

`processing/activity.rs` implements a stateful `ActivityGrouper`:

- **Group key**: `(project: Option<String>, app: String)`
- **Merge condition**: Same key + event duration < 300s (5 minute adjacency window)
- **On merge**: Accumulate `total_duration_sec`, deduplicate `files_touched` and `languages`
- **On split**: Finalize current group as `ActivityGroup` with `start_time`, `end_time`, accumulated fields
- **Idle split**: `split_on_idle()` finalizes current group; next event starts fresh
- **Side-channels**: Terminal workflows (deduplicated) and git summaries (latest wins) are injected into the current group

### Storage Format

**JSONL** (JSON Lines) — one JSON object per line, append-only:

- `events.jsonl` — Full raw event audit trail (`Event` struct)
- `normalized_sessions.jsonl` — Processed activity groups (`ActivityGroup` struct)

Path: `../sessions/active_session/` relative to the Rust binary.

### Report Generation

`processing/summary.rs` formats `ActivityGroup` data into a neutral, factual report:

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

### AI Summarization Flow

The optional Python layer (`py-analyzer/`) uses a local LLM via [Ollama](https://ollama.ai):

1. `formatter.py` reads `normalized_sessions.jsonl` and aggregates by project
2. `prompts.py` constructs a system prompt enforcing factual, non-judgmental output
3. `analyzer.py` sends the aggregated data to Ollama for narrative summarization
4. `guardrails.py` strips banned words ("productive", "lazy", "efficient", etc.)
5. `daily_writer.py` saves the summary to `outputs/{YYYY-MM-DD}.txt`

**Prompt philosophy**: The system prompt explicitly forbids hallucination, productivity scoring, motivational advice, and invented work. The LLM is used as a *formatting engine* for structured data, not as an inference engine.

---

## Installation

### Prerequisites

- **Rust** 1.70+ (`rustup` recommended)
- **Linux**: `gnomectl` (for window tracking) + `xprintidle` (for idle detection)
- **Python** 3.12+ (optional, for AI analysis)
- **Ollama** (optional, for AI analysis) — pull a model like `qwen2.5:7b`

### Build

```bash
cd rust-tracker
cargo build --release
```

The binary will be at `rust-tracker/target/release/tracker`.

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
# Start the tracking loop
tracker Start

# View the activity report (reads normalized_sessions.jsonl)
tracker Report

# Log lifecycle events (currently log-only, loop runs synchronously in Start)
tracker Pause
tracker Resume
tracker Stop

# Generate AI daily summary
cd py-analyzer && python analyzer.py
```

Press `Ctrl+C` during tracking to gracefully flush all pending sessions.

---

## Example Output

### Raw Event (`events.jsonl`)

```json
{"timestamp":"2026-05-12T15:10:40.090512029+05:30","event_type":"window_session","source":"window_tracker","app":"Antigravity","title":"tracker - Antigravity - git.rs","workspace":0,"duration_sec":81,"data":{"message":"Normalized session"}}
```

### Normalized Session (`normalized_sessions.jsonl`)

```json
{"start_time":"2026-05-12T15:10:34.065395748+05:30","end_time":"2026-05-12T15:11:34.319094460+05:30","project":"tracker","app":"antigravity","total_duration_sec":81,"files_touched":["git.rs"],"languages":["rust"],"terminal_workflows":[],"git_summary":null}
```

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
> - **Spotify Premium**: Listening to music for 4.97 minutes using Spotify.
>
> Summary: The day consisted primarily of Rust programming, light web browsing, and listening to music.

---

## AI Pipeline

### Architecture

```
normalized_sessions.jsonl
         │
         ▼
  formatter.py ─── build_ai_context()
         │          aggregates by project:
         │          {project: {time_min, apps, files, languages,
         │                     terminal_workflows, git_commits, dev_areas}}
         ▼
  prompts.py ───── SYSTEM_PROMPT (factual, no scoring, no hallucination)
                  USER_PROMPT_TEMPLATE ({data})
         ▼
  analyzer.py ──── ollama.chat(model=MODEL, messages=[...])
         │
         ▼
  guardrails.py ── sanitize_output() — strips banned judgment words
         │
         ▼
  daily_writer.py ── write_summary() → outputs/{date}.txt
```

### Prompts

**System prompt:**
```
You are a local activity reconstruction engine.
Rules:
- Never hallucinate
- Never invent work
- Never assign productivity scores
- Never give motivational advice
- Be factual and concise
- Summarize only provided data
```

**User prompt template:**
```
Generate a clean daily activity summary from this structured activity data:

{data}
```

### Guardrails

`guardrails.py` strips these words from LLM output: `productive`, `lazy`, `efficient`, `inefficient`, `excellent`, `bad` — preventing the model from making qualitative judgments that contradict the project's "no scoring" philosophy.

---

## Developer Notes

### Design Decisions

| Decision | Rationale |
|----------|-----------|
| **No ML in pipeline** | Enrichment is pure string parsing; reproducibility and auditability are guaranteed |
| **JSONL over SQLite** | Append-only, trivially inspectable with `cat`/`jq`/`tail`, no schema migrations |
| **Dual logging** | `events.jsonl` = raw audit trail; `normalized_sessions.jsonl` = processed view; both independently replayable |
| **`#[serde(flatten)]` on EnrichedEvent** | Flat JSON output matching downstream consumer expectations |
| **Compile-time platform selection** | Simpler than trait objects at current scale; `#[cfg]` avoids dynamic dispatch |
| **5-minute adjacency window** | Balances merge aggressiveness vs. fragmentation; validated against real usage |
| **2-second noise filter** | Eliminates alt-tab flicker and workspace switches without meaningful focus |
| **Single-threaded loop** | Collector calls are fast I/O (< 50ms); async/channels add complexity without benefit at this scale |

### Known Technical Debt

- **`WindowInfo` type duplication** — Linux and Windows both define identical structs; should be unified in `models/`
- **Pause/Resume/Stop are log-only** — lifecycle events are recorded but don't actually pause the tracking loop (runs synchronously in `Start`)
- **Hardcoded paths** — session directory and poll interval (3s) are hardcoded; should be configurable
- **Windows platform path returns `None`** — the platform-agnostic `get_platform_telemetry()` on Windows always returns `None` due to type mismatch
- **Terminal tracking is naive** — reads entire `.bash_history` file each poll; should use `tail` or inotify
- **Empty placeholders** — `config/`, `storage/schema.rs`, `utils/`, `browser.rs` contain empty modules awaiting implementation
- **Git unpushed commit counting** uses `HEAD...@{upstream}` which fails if no upstream is configured

### Extension Points

| File | Purpose | Status |
|------|---------|--------|
| `collector/linux/browser.rs` | Browser tab/URL tracking | Empty |
| `config/settings.rs` | Runtime configuration | Empty |
| `storage/schema.rs` | Schema versioning + migration | Empty |
| `utils/paths.rs` | Path resolution utilities | Empty |
| `utils/time.rs` | Time formatting utilities | Empty |

---

## Testing

```bash
cd rust-tracker
cargo test
```

Test modules exist in:
- `processing/activity.rs` — group merging, splitting, noise filtering
- `processing/enrich.rs` — language detection, app normalization, title parsing
- `processing/git.rs` — summary building, dev area detection, burst detection
- `processing/terminal.rs` — command classification, workflow detection, deduplication

---

## Future Improvements

1. **Config system** — move poll interval, thresholds, and paths from hardcoded constants to TOML/YAML config
2. **Unified `WindowInfo`** — extract shared struct into `models/` with a `TelemetryCollector` trait
3. **Real Pause/Resume** — make lifecycle commands actually start/stop the tracking loop
4. **Browser tracking** — implement `browser.rs` to capture active tab titles via browser extension or DBus
5. **File-based project detection** — scan for `Cargo.toml`, `.git`, `setup.py` to validate inferred projects
6. **Session archival** — move completed sessions to timestamped directories
7. **Web dashboard** — local-first SPA reading JSONL directly
8. **Export formats** — JSON, CSV, Markdown report export
9. **Schema versioning** — implement `storage/schema.rs` for forward-compatible evolution
10. **Plugin system** — declarative collector plugins (not dynamic linking)
11. **Encrypted storage** — optional at-rest encryption for sensitive metadata
12. **AI fine-tuning** — few-shot examples for more structured LLM daily summaries
13. **Cross-platform CI** — test on Linux and Windows in CI pipeline

---

## Contributing

1. Fork the repo
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

**Guidelines:**
- Maintain the "no AI in pipeline" principle — enrichment must be deterministic
- Add tests for new processing logic
- Keep collector calls fast and non-blocking (< 50ms)
- Don't add scoring, productivity metrics, or judgmental output
- Run `cargo test` before submitting

---

## License

This project is licensed under the MIT License. No license file is present in the repository — the author should add one. The intent is inferred from standard open-source practices.

---

## Related

- [gnomectl](https://github.com/utkrisht-thakur-2003/gnomectl) — Companion tool for Linux window tracking (writes `activewindow.json`)
- [Ollama](https://ollama.ai) — Local LLM runtime for AI summaries
