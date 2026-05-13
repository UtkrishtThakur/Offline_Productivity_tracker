import os
import sys

import ollama
from dotenv import load_dotenv

from config import AnalyzerConfig
from formatter import build_ai_context
from prompts import SYSTEM_PROMPT, USER_PROMPT_TEMPLATE
from guardrails import sanitize_output
from daily_writer import write_summary


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
    cfg = AnalyzerConfig()

    if not cfg.enabled:
        print("AI analyzer is disabled (TRACKER_AI_ENABLED=false).")
        return

    context = build_ai_context(cfg)

    if not context:
        print("\nNo activity data found.\n")
        return

    print(f"\nGenerating summary (model={cfg.model}, host={cfg.ollama_host})...\n")

    summary = generate_summary(context, cfg)
    summary = sanitize_output(summary)
    output_file = write_summary(summary, cfg)

    print("\n====================================")
    print(summary)
    print("====================================")
    print(f"\nSummary written to: {output_file}\n")


if __name__ == "__main__":
    main()
