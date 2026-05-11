// processing/enrich.rs
//
// Deterministic context enrichment.
// Extracts project names, files, languages, and normalized
// app names from raw telemetry — no AI, pure string parsing.

use crate::models::enriched::EnrichedEvent;
use crate::models::event::Event;

/// Known file extensions → language mapping.
const LANGUAGE_MAP: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("js", "javascript"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("jsx", "javascript"),
    ("go", "go"),
    ("java", "java"),
    ("c", "c"),
    ("cpp", "cpp"),
    ("h", "c"),
    ("hpp", "cpp"),
    ("cs", "csharp"),
    ("rb", "ruby"),
    ("php", "php"),
    ("swift", "swift"),
    ("kt", "kotlin"),
    ("scala", "scala"),
    ("lua", "lua"),
    ("sh", "shell"),
    ("bash", "shell"),
    ("zsh", "shell"),
    ("fish", "shell"),
    ("html", "html"),
    ("css", "css"),
    ("scss", "scss"),
    ("sass", "sass"),
    ("json", "json"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("toml", "toml"),
    ("xml", "xml"),
    ("md", "markdown"),
    ("sql", "sql"),
    ("r", "r"),
    ("dart", "dart"),
    ("ex", "elixir"),
    ("exs", "elixir"),
    ("erl", "erlang"),
    ("zig", "zig"),
    ("nim", "nim"),
    ("vue", "vue"),
    ("svelte", "svelte"),
];

/// Known app name normalizations.
const APP_NAME_MAP: &[(&str, &str)] = &[
    ("Antigravity", "antigravity"),
    ("Code - OSS", "vscode"),
    ("code-oss", "vscode"),
    ("Code", "vscode"),
    ("Visual Studio Code", "vscode"),
    ("Firefox", "firefox"),
    ("firefox", "firefox"),
    ("Firefox ESR", "firefox"),
    ("Google Chrome", "chrome"),
    ("google-chrome", "chrome"),
    ("Chromium", "chromium"),
    ("Brave Browser", "brave"),
    ("brave-browser", "brave"),
    ("Alacritty", "alacritty"),
    ("kitty", "kitty"),
    ("gnome-terminal", "gnome-terminal"),
    ("Konsole", "konsole"),
    ("Terminal", "terminal"),
    ("iTerm2", "iterm"),
    ("Nautilus", "file-manager"),
    ("Thunar", "file-manager"),
    ("Dolphin", "file-manager"),
    ("Slack", "slack"),
    ("Discord", "discord"),
    ("Telegram", "telegram"),
    ("Obsidian", "obsidian"),
    ("Notion", "notion"),
    ("IntelliJ IDEA", "intellij"),
    ("PyCharm", "pycharm"),
    ("WebStorm", "webstorm"),
    ("CLion", "clion"),
    ("Postman", "postman"),
    ("Insomnia", "insomnia"),
    ("Spotify", "spotify"),
];

/// Detect programming language from a filename or extension.
pub fn detect_language(filename: &str) -> Option<String> {

    let ext = filename
        .rsplit('.')
        .next()?;

    let ext_lower = ext.to_lowercase();

    LANGUAGE_MAP
        .iter()
        .find(|(e, _)| *e == ext_lower.as_str())
        .map(|(_, lang)| lang.to_string())
}

/// Normalize a raw application name to a canonical form.
pub fn normalize_app_name(raw: &str) -> String {

    // Check exact match first
    for (from, to) in APP_NAME_MAP {
        if raw == *from {
            return to.to_string();
        }
    }

    // Check case-insensitive contains
    let lower = raw.to_lowercase();
    for (from, to) in APP_NAME_MAP {
        if lower.contains(&from.to_lowercase()) {
            return to.to_string();
        }
    }

    // Fallback: lowercase + trim
    lower.trim().to_string()
}

/// Extract project name and active file from an editor window title.
///
/// Common patterns:
///   "tracker - Antigravity - main.rs"
///   "main.rs - tracker - Visual Studio Code"
///   "project_name — File Browser"
///
/// Strategy:
///   1. Split on " - " or " — "
///   2. Find the segment that looks like a file (contains a dot + known extension)
///   3. The project is typically the segment adjacent to the app or file
pub fn extract_from_title(
    title: &str,
    app: &str,
) -> (Option<String>, Option<String>) {

    let normalized_app = normalize_app_name(app);

    // Split on common delimiters
    let segments: Vec<&str> = title
        .split(" - ")
        .flat_map(|s| s.split(" — "))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return (None, None);
    }

    let mut file: Option<String> = None;
    let mut project: Option<String> = None;

    // Find file segment — contains a dot and a known extension
    for seg in &segments {
        if looks_like_file(seg) {
            file = Some(seg.to_string());
            break;
        }
    }

    // Find project segment — not the app name, not the file
    for seg in &segments {
        let seg_str = seg.to_string();
        let seg_lower = seg.to_lowercase();

        // Skip the app name
        if seg_lower == normalized_app
            || seg_lower == app.to_lowercase()
        {
            continue;
        }

        // Skip the file
        if file.as_deref() == Some(seg) {
            continue;
        }

        // Skip generic filler
        if seg_lower == "untitled"
            || seg_lower == "new tab"
            || seg_lower == "welcome"
        {
            continue;
        }

        project = Some(seg_str);
        break;
    }

    (project, file)
}

/// Check if a string segment looks like a filename.
fn looks_like_file(s: &str) -> bool {

    if let Some(dot_pos) = s.rfind('.') {
        let ext = &s[dot_pos + 1..];
        if ext.is_empty() || ext.len() > 10 {
            return false;
        }
        // Check against known extensions
        let ext_lower = ext.to_lowercase();
        LANGUAGE_MAP
            .iter()
            .any(|(e, _)| *e == ext_lower.as_str())
    } else {
        false
    }
}

/// Enrich a raw Event with deterministic context.
pub fn enrich_event(event: &Event) -> EnrichedEvent {

    let raw_app = event.app
        .as_deref()
        .unwrap_or("unknown");

    let raw_title = event.title
        .as_deref()
        .unwrap_or("");

    let normalized_app = normalize_app_name(raw_app);

    let (project, file) =
        extract_from_title(raw_title, raw_app);

    let language = file
        .as_deref()
        .and_then(detect_language);

    // Extract git context from event data if present
    let repo = event.data
        .get("repo")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let branch = event.data
        .get("branch")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    EnrichedEvent {
        event: event.clone(),
        project,
        file,
        language,
        normalized_app,
        repo,
        branch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(
            detect_language("main.rs"),
            Some("rust".to_string())
        );
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(
            detect_language("script.py"),
            Some("python".to_string())
        );
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language("README"), None);
    }

    #[test]
    fn test_normalize_app() {
        assert_eq!(
            normalize_app_name("Antigravity"),
            "antigravity"
        );
        assert_eq!(
            normalize_app_name("Code - OSS"),
            "vscode"
        );
        assert_eq!(
            normalize_app_name("unknown-app"),
            "unknown-app"
        );
    }

    #[test]
    fn test_extract_title_antigravity() {
        let (project, file) = extract_from_title(
            "tracker - Antigravity - main.rs",
            "Antigravity",
        );
        assert_eq!(project, Some("tracker".to_string()));
        assert_eq!(file, Some("main.rs".to_string()));
    }

    #[test]
    fn test_extract_title_vscode() {
        let (project, file) = extract_from_title(
            "main.rs - tracker - Visual Studio Code",
            "Visual Studio Code",
        );
        assert_eq!(file, Some("main.rs".to_string()));
        assert_eq!(project, Some("tracker".to_string()));
    }

    #[test]
    fn test_extract_title_no_file() {
        let (project, file) = extract_from_title(
            "Firefox",
            "Firefox",
        );
        assert_eq!(project, None);
        assert_eq!(file, None);
    }
}
