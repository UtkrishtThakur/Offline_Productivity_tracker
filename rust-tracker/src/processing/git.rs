// processing/git.rs
//
// Deterministic git workflow reconstruction.
// Extracts structured summaries from raw git collector data
// and infers active development areas from changed file paths.

use crate::models::activity::GitSummary;
use serde_json::Value;

/// Build a structured GitSummary from raw git collector output.
pub fn build_git_summary(
    data: &Value,
) -> Option<GitSummary> {

    let repo = data
        .get("repo")
        .and_then(|v| v.as_str())?
        .to_string();

    let branch = data
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let changed_files: Vec<String> = data
        .get("changed_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let unpushed = data
        .get("unpushed_commits")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // Count commits from the data if available,
    // otherwise infer from unpushed count
    let commit_count = data
        .get("commit_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let dev_areas = detect_dev_areas(&changed_files);

    Some(GitSummary {
        repo,
        branch,
        commit_count,
        unpushed,
        changed_files,
        dev_areas,
    })
}

/// Infer active development areas from changed file paths.
///
/// Extracts the parent directory of each changed file and
/// produces a deduplicated list of directories being worked on.
///
/// Example:
///   ["M src/main.rs", "M src/session/manager.rs"]
///   → ["src", "src/session"]
pub fn detect_dev_areas(
    changed_files: &[String],
) -> Vec<String> {

    let mut areas: Vec<String> = Vec::new();

    for entry in changed_files {

        // Git porcelain format: "M  src/main.rs" or "?? new_file.rs"
        let path = entry
            .trim()
            .split_whitespace()
            .last()
            .unwrap_or("");

        if path.is_empty() {
            continue;
        }

        // Extract parent directory
        if let Some(slash_pos) = path.rfind('/') {
            let dir = &path[..slash_pos];
            if !dir.is_empty()
                && !areas.contains(&dir.to_string())
            {
                areas.push(dir.to_string());
            }
        }
    }

    areas
}

/// Detect if a burst of commits happened.
/// A burst is defined as multiple commits within the
/// tracked interval (typically 3 seconds polling).
pub fn is_commit_burst(
    commit_count: u32,
) -> bool {
    commit_count >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_git_summary() {
        let data = json!({
            "repo": "/mnt/ai/Projects/tracker",
            "branch": "main",
            "changed_files": ["M src/main.rs", "M src/session/manager.rs"],
            "changed_count": 2,
            "last_commit_hash": "abc123",
            "last_commit_message": "added normalization",
            "unpushed_commits": 2
        });

        let summary = build_git_summary(&data).unwrap();

        assert_eq!(summary.repo, "/mnt/ai/Projects/tracker");
        assert_eq!(summary.branch, "main");
        assert_eq!(summary.unpushed, 2);
        assert_eq!(summary.changed_files.len(), 2);
        assert_eq!(summary.dev_areas, vec!["src", "src/session"]);
    }

    #[test]
    fn test_detect_dev_areas() {
        let files = vec![
            "M src/main.rs".to_string(),
            "M src/models/event.rs".to_string(),
            "?? tests/test_enrich.rs".to_string(),
        ];

        let areas = detect_dev_areas(&files);

        assert_eq!(areas, vec![
            "src",
            "src/models",
            "tests",
        ]);
    }

    #[test]
    fn test_detect_dev_areas_root_file() {
        let files = vec![
            "M Cargo.toml".to_string(),
        ];

        let areas = detect_dev_areas(&files);
        assert!(areas.is_empty());
    }

    #[test]
    fn test_commit_burst() {
        assert!(!is_commit_burst(1));
        assert!(!is_commit_burst(2));
        assert!(is_commit_burst(3));
        assert!(is_commit_burst(10));
    }
}
