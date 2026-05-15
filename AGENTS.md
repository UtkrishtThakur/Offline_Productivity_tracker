# AGENTS.md — rust-tracker

**Project:** Local-first deterministic desktop telemetry and activity reconstruction engine.
**Structure:** `rust-tracker/` (Rust binary), `py-analyzer/` (optional Python AI layer).

## Essential commands (run from `rust-tracker/`)

```bash
cargo build --release   # idempotent binary
cargo test              # 20+ tests in processing/*.rs
cargo test -- <test_name>  # e.g. cargo test -- test_merge_same_project
```

No `Makefile`/`Justfile` — just `cargo` directly.

## Binary usage

```bash
./target/release/tracker [--config PATH] <SUBCOMMAND>
```
Subcommands: `Start`, `Pause`, `Resume`, `Stop`, `Report`, `ReportAi`, `InitConfig`.

- `--config` is a **global flag** before the subcommand.
- `Pause`/`Resume`/`Stop` only **log events** — they don't actually pause the loop.
- Binary runs from any CWD; `session_dir` default `../sessions/active_session` is relative to CWD.

## Platform quirks

- **Only Linux collectors are fully implemented.** `#[cfg(not(target_os = "linux"))]` returns `None` for all collectors (hardcoded in `main.rs:180-189`).
- Linux depends on external tools: `gnomectl` (window info), `xprintidle` (idle detection).
- `WindowInfo` struct is **duplicated** across `collector/linux/window.rs` and `collector/windows/window.rs` (known issue, not yet extracted to `models/`).

## Config system

- Config resolution: CLI `--config` → `$TRACKER_CONFIG` → `./tracker.toml` → `~/.config/tracker/tracker.toml` → built-in defaults.
- Every field overridable by `TRACKER_*` env vars at startup (see `config/mod.rs:135-197`).
- `tracker InitConfig` writes default `tracker.toml` to CWD (limited docs compared to `tracker.toml` in root).
- Config is read **once** at startup — changes require restart.

## Architecture notes

- **Single-threaded sync loop.** No async, no channels, no locks. `ctrlc` sets an `AtomicBool` flag.
- Tracking loop polls every `poll_interval_sec` (default 3s). Collectors must return <50ms.
- Activity groups by `(project, app)` key with configurable adjacency window (default 300s).
- Events < `min_meaningful_sec` (default 2s) are filtered as noise.
- Storage is flat JSONL files — `events.jsonl` and `normalized_sessions.jsonl` in `session_dir`.
- `#[serde(flatten)]` on `EnrichedEvent.event` makes JSON output flat.

## AI analyzer (`py-analyzer/`)

- Invoked as subprocess: `python3 py-analyzer/analyzer.py`, config forwarded as `TRACKER_AI_*` env vars.
- Python deps: `ollama`, `python-dotenv`, `rich` (see `py-analyzer/requirements.txt`).
- Toggled by `ai_analyzer.enabled` in config (default `false`).
- Docker: `docker compose up -d` from `py-analyzer/`, uses `network_mode: "host"` for Ollama.
- `guardrails.py` strips judgment words from AI output (`productive`, `lazy`, etc.).

## Daily rotation & summary system

- The tracking loop detects day changes (midnight) and automatically finalizes the previous day.
- Per-day archives in `summaries/YYYY-MM-DD/`: `deterministic.txt` (always), `semantic.txt` (AI only), `metadata.json`.
- After successful finalization, raw/normalized JSONL logs for that day are cleaned up (via atomic tmp+rename rewrite).
- On startup, any unarchived previous days in the JSONL are archived before the loop begins.
- AI summary retry queue: failed semantic summaries retry up to `retry_attempts` (default 3) with `retry_delay_sec` (default 30s) between attempts.
- New config: `[summary]` section in `tracker.toml` overridable via `TRACKER_AUTO_CLEANUP`, `TRACKER_RETRY_ATTEMPTS`, `TRACKER_RETRY_DELAY_SEC` env vars.
- New storage field: `summaries_dir` (default `../sessions/summaries`, overridable via `TRACKER_SUMMARIES_DIR`).

## New modules

- `processing/daily.rs` — deterministic timeline summary formatting from `ActivityGroup` list
- `storage/archiver.rs` — summary I/O, `DayMetadata`, JSONL cleanup, pending day archival

## Development conventions

- Tests are inline in source files (`#[cfg(test)] mod tests { ... }`) in `processing/*.rs` and `storage/archiver.rs`.
- No `pub` exports on test helper functions — they're module-internal.
- Placeholder modules with empty files: `utils/paths.rs`, `utils/time.rs`, `storage/schema.rs`, `collector/linux/browser.rs`.
- The full architecture is documented in `CLAUDE.md` — consult for deeper details.

## Git

- `Cargo.lock` is tracked (binary application).
- `sessions/`, `*.jsonl`, `*.log` are gitignored.
- Root `.gitignore` covers Rust, Python, and IDE artifacts.
