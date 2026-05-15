import argparse
import json
import os
import sys
from pathlib import Path

import ollama
from dotenv import load_dotenv

from config import AnalyzerConfig
from formatter import build_ai_context, load_logs
from prompts import SYSTEM_PROMPT, USER_PROMPT_TEMPLATE
from guardrails import sanitize_output
from daily_writer import write_semantic_summary, update_metadata


load_dotenv()


def generate_summary(data, cfg):
    prompt = USER_PROMPT_TEMPLATE.format(data=data)

    ollama_client = ollama.Client(host=cfg.ollama_host)

    response = ollama_client.chat(
        model=cfg.model,
        messages=[
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": prompt}
        ]
    )

    return response["message"]["content"]


def main():
    parser = argparse.ArgumentParser(description="AI activity summary generator")
    parser.add_argument("--date", type=str, default=None,
                        help="Date filter (YYYY-MM-DD). When set, writes to summaries/<date>/")
    args = parser.parse_args()

    cfg = AnalyzerConfig()

    if not cfg.enabled:
        print("AI analyzer is disabled (TRACKER_AI_ENABLED=false).")
        return

    logs = load_logs(cfg)

    # Filter by date if --date is provided
    if args.date:
        logs = [log for log in logs if log.get("start_time", "").startswith(args.date)]
        if not logs:
            print(f"No activity data found for {args.date}.")
            return

    context = build_ai_context(logs)

    if not context:
        print("\nNo activity data found.\n")
        return

    model_info = f"model={cfg.model}, host={cfg.ollama_host}"
    if args.date:
        model_info += f", date={args.date}"
    print(f"\nGenerating summary ({model_info})...\n")

    try:
        summary = generate_summary(context, cfg)
    except Exception as e:
        print(f"AI generation failed: {e}", file=sys.stderr)
        sys.exit(2)

    summary = sanitize_output(summary)

    if args.date:
        # Write to summaries/<date>/semantic.txt and update metadata
        summaries_dir = os.getenv("TRACKER_SUMMARIES_DIR", "../sessions/summaries")
        output_file = write_semantic_summary(summary, args.date, summaries_dir)
        update_metadata(args.date, summaries_dir, "semantic.txt")
    else:
        # Legacy path: write to outputs/<date>.txt
        from daily_writer import write_summary as legacy_write
        output_file = legacy_write(summary, cfg)

    print("\n====================================")
    print(summary)
    print("====================================")
    print(f"\nSummary written to: {output_file}\n")


if __name__ == "__main__":
    main()
