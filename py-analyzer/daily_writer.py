import json
import os
from datetime import datetime
from pathlib import Path


def write_summary(text, cfg):
    """Legacy: write to outputs/<date>.txt"""
    output_dir = Path(cfg.output_dir)
    output_dir.mkdir(exist_ok=True)

    today = datetime.now().strftime("%Y-%m-%d")
    file_path = output_dir / f"{today}.txt"

    with open(file_path, "w") as f:
        f.write(text)

    return file_path


def write_semantic_summary(text, date, summaries_dir):
    """Write semantic.txt to summaries/<date>/ directory."""
    day_dir = Path(summaries_dir) / date
    day_dir.mkdir(parents=True, exist_ok=True)

    file_path = day_dir / "semantic.txt"
    with open(file_path, "w") as f:
        f.write(text)

    return file_path


def update_metadata(date, summaries_dir, semantic_file="semantic.txt"):
    """Update metadata.json to record that semantic summary was written."""
    day_dir = Path(summaries_dir) / date
    meta_path = day_dir / "metadata.json"

    metadata = {}
    if meta_path.exists():
        with open(meta_path, "r") as f:
            try:
                metadata = json.load(f)
            except json.JSONDecodeError:
                metadata = {}

    metadata["semantic_summary"] = semantic_file
    metadata["status"] = "finalized"
    metadata["error"] = None

    with open(meta_path, "w") as f:
        json.dump(metadata, f, indent=2)
