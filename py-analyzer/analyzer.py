import os
import ollama

from dotenv import load_dotenv

from formatter import build_ai_context
from prompts import SYSTEM_PROMPT, USER_PROMPT_TEMPLATE
from guardrails import sanitize_output
from daily_writer import write_summary


load_dotenv()

MODEL = os.getenv("MODEL")


def generate_summary(data):
    prompt = USER_PROMPT_TEMPLATE.format(data=data)

    response = ollama.chat(
        model=MODEL,
        messages=[
            {
                "role": "system",
                "content": SYSTEM_PROMPT
            },
            {
                "role": "user",
                "content": prompt
            }
        ]
    )

    return response["message"]["content"]


def main():
    context = build_ai_context()

    if not context:
        print("\nNo activity data found.\n")
        return

    print("\nGenerating summary...\n")

    summary = generate_summary(context)

    summary = sanitize_output(summary)

    output_file = write_summary(summary)

    print("\n====================================")
    print(summary)
    print("====================================")

    print(f"\nSummary written to: {output_file}\n")


if __name__ == "__main__":
    main()