import json
from collections import defaultdict
from pathlib import Path


def load_logs(cfg):
    path = Path(cfg.normalized_log)

    if not path.exists():
        return []

    logs = []

    with open(path, "r") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                logs.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    return logs


def aggregate_logs(logs):
    projects = defaultdict(lambda: {
        "time_sec": 0,
        "apps": set(),
        "files": set(),
        "languages": set(),
        "terminal_workflows": set(),
        "git_commits": 0,
        "dev_areas": set()
    })

    for log in logs:
        project = log.get("project") or "unknown"
        entry = projects[project]

        entry["time_sec"] += log.get("total_duration_sec", 0)

        app = log.get("app")
        if app:
            entry["apps"].add(app)

        for f in log.get("files_touched", []):
            entry["files"].add(f)

        for lang in log.get("languages", []):
            entry["languages"].add(lang)

        for workflow in log.get("terminal_workflows", []):
            entry["terminal_workflows"].add(workflow)

        git_summary = log.get("git_summary")
        if git_summary:
            entry["git_commits"] += git_summary.get("commit_count", 0)
            for area in git_summary.get("dev_areas", []):
                entry["dev_areas"].add(area)

    return projects


def build_ai_context(logs):
    aggregated = aggregate_logs(logs)

    final = {}

    for project, data in aggregated.items():
        final[project] = {
            "time_min": round(data["time_sec"] / 60, 2),
            "apps": list(data["apps"]),
            "files": list(data["files"]),
            "languages": list(data["languages"]),
            "terminal_workflows": list(data["terminal_workflows"]),
            "git_commits": data["git_commits"],
            "dev_areas": list(data["dev_areas"])
        }

    return final
