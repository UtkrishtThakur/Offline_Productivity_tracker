from pathlib import Path
from datetime import datetime


OUTPUT_DIR = "outputs"


def write_summary(text):
    Path(OUTPUT_DIR).mkdir(exist_ok=True)

    today = datetime.now().strftime("%Y-%m-%d")

    file_path = Path(OUTPUT_DIR) / f"{today}.txt"

    with open(file_path, "w") as f:
        f.write(text)

    return file_path