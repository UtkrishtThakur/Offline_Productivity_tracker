from pathlib import Path
from datetime import datetime


def write_summary(text, cfg):
    output_dir = Path(cfg.output_dir)
    output_dir.mkdir(exist_ok=True)

    today = datetime.now().strftime("%Y-%m-%d")
    file_path = output_dir / f"{today}.txt"

    with open(file_path, "w") as f:
        f.write(text)

    return file_path
