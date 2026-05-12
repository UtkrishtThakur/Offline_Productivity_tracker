BANNED_WORDS = [
    "productive",
    "lazy",
    "efficient",
    "inefficient",
    "excellent",
    "bad",
]


def sanitize_output(text):
    for word in BANNED_WORDS:
        text = text.replace(word, "")

    return text.strip()