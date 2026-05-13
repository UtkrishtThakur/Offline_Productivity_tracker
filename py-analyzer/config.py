"""
py-analyzer configuration.

Reads from environment variables with TRACKER_AI_* prefix for consistency
with the Rust tracker config system. Falls back to legacy NORMALIZED_LOG,
OLLAMA_HOST, MODEL, OUTPUT_DIR for Docker compatibility.
"""

import os
from pathlib import Path


class AnalyzerConfig:
    """Immutable config snapshot read from environment at construction time."""

    def __init__(self):
        # ── Session data source ──────────────────────────────────────
        session_dir = os.getenv(
            "TRACKER_SESSION_DIR",
            os.getenv("SESSION_DIR", "../sessions/active_session"),
        )
        normalized_file = os.getenv(
            "TRACKER_NORMALIZED_FILE",
            os.getenv("NORMALIZED_FILE", "normalized_sessions.jsonl"),
        )
        # Support legacy Docker env var with absolute path
        legacy_path = os.getenv("NORMALIZED_LOG", "")
        if legacy_path and Path(legacy_path).is_absolute():
            self.normalized_log = legacy_path
        else:
            self.normalized_log = str(Path(session_dir) / normalized_file)

        # ── Output ───────────────────────────────────────────────────
        self.output_dir = os.getenv(
            "TRACKER_AI_OUTPUT_DIR",
            os.getenv("OUTPUT_DIR", "outputs"),
        )

        # ── AI provider ──────────────────────────────────────────────
        self.ollama_host = os.getenv(
            "TRACKER_AI_OLLAMA_HOST",
            os.getenv("OLLAMA_HOST", "http://localhost:11434"),
        )
        self.model = os.getenv(
            "TRACKER_AI_MODEL",
            os.getenv("MODEL", "qwen2.5:7b"),
        )

        # ── Feature toggle ───────────────────────────────────────────
        raw = os.getenv("TRACKER_AI_ENABLED", os.getenv("AI_ENABLED", "true"))
        self.enabled = raw.lower() in ("true", "1", "yes")

    def __repr__(self) -> str:
        return (
            f"AnalyzerConfig("
            f"normalized_log={self.normalized_log}, "
            f"output_dir={self.output_dir}, "
            f"ollama_host={self.ollama_host}, "
            f"model={self.model}, "
            f"enabled={self.enabled})"
        )
