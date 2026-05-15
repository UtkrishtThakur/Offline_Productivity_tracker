use std::collections::BTreeMap;

use crate::models::activity::ActivityGroup;

/// Generate a chronological timeline summary for a single day.
/// This is the deterministic fallback — no AI, pure Rust.
pub fn format_timeline_summary(groups: &[ActivityGroup], date: &str) -> String {
    if groups.is_empty() {
        return format!("No activity recorded on {date}.\n");
    }

    let mut sorted = groups.to_vec();
    sorted.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    let mut output = String::new();
    output.push_str(&format!("=== Activity Timeline for {date} ===\n\n"));

    for group in &sorted {
        let start_hm = extract_hhmm(&group.start_time).unwrap_or_default();
        let end_hm = extract_hhmm(&group.end_time).unwrap_or_default();
        let project = group.project.as_deref().unwrap_or("(no project)");
        let dur_m = group.total_duration_sec / 60;
        let dur_s = group.total_duration_sec % 60;

        output.push_str(&format!("{:>5} - {:<5} | {:<20} | {}\n", start_hm, end_hm, group.app, project));
        if dur_m > 0 {
            output.push_str(&format!("       Duration: {}m {}s\n", dur_m, dur_s));
        } else {
            output.push_str(&format!("       Duration: {}s\n", dur_s));
        }

        if !group.files_touched.is_empty() {
            output.push_str(&format!("       Files: {}\n", group.files_touched.join(", ")));
        }

        if !group.languages.is_empty() {
            output.push_str(&format!("       Languages: {}\n", group.languages.join(", ")));
        }

        if !group.terminal_workflows.is_empty() {
            output.push_str(&format!("       Terminal: {}\n", group.terminal_workflows.join(", ")));
        }

        if let Some(ref git) = group.git_summary {
            let mut git_parts = Vec::new();
            if git.commit_count > 0 {
                git_parts.push(format!("{} commits", git.commit_count));
            }
            if git.unpushed > 0 {
                git_parts.push(format!("{} unpushed", git.unpushed));
            }
            if !git.dev_areas.is_empty() {
                git_parts.push(format!("areas: {}", git.dev_areas.join(", ")));
            }
            if !git_parts.is_empty() {
                output.push_str(&format!("       Git: {}\n", git_parts.join(", ")));
            }
        }

        output.push('\n');
    }

    // Summary statistics
    let total_sec: u64 = sorted.iter().map(|g| g.total_duration_sec).sum();
    let hours = total_sec / 3600;
    let minutes = (total_sec % 3600) / 60;
    let seconds = total_sec % 60;

    let mut project_times: BTreeMap<String, u64> = BTreeMap::new();
    for g in &sorted {
        let p = g.project.clone().unwrap_or_else(|| "(no project)".to_string());
        *project_times.entry(p).or_default() += g.total_duration_sec;
    }

    let mut all_languages: Vec<&str> = Vec::new();
    for g in &sorted {
        for lang in &g.languages {
            if !all_languages.contains(&lang.as_str()) {
                all_languages.push(lang);
            }
        }
    }

    let mut all_files: Vec<&str> = Vec::new();
    for g in &sorted {
        for f in &g.files_touched {
            if !all_files.contains(&f.as_str()) {
                all_files.push(f);
            }
        }
    }

    let mut all_apps: Vec<&str> = Vec::new();
    for g in &sorted {
        if !all_apps.contains(&g.app.as_str()) {
            all_apps.push(&g.app);
        }
    }

    output.push_str("─────────────────────────────────────\n");
    if hours > 0 {
        output.push_str(&format!("Total tracked: {}h {}m {}s\n", hours, minutes, seconds));
    } else if minutes > 0 {
        output.push_str(&format!("Total tracked: {}m {}s\n", minutes, seconds));
    } else {
        output.push_str(&format!("Total tracked: {}s\n", seconds));
    }

    output.push_str(&format!("Groups: {}\n", sorted.len()));
    output.push_str(&format!("Projects: {}\n", project_times.keys().cloned().collect::<Vec<_>>().join(", ")));
    output.push_str(&format!("Applications: {}\n", all_apps.join(", ")));

    if !all_languages.is_empty() {
        output.push_str(&format!("Languages: {}\n", all_languages.join(", ")));
    }
    if !all_files.is_empty() {
        output.push_str(&format!("Files touched: {}\n", all_files.join(", ")));
    }

    output.push_str(&format!("\n=== End of report for {date} ===\n"));
    output
}

/// Extract HH:MM from an RFC 3339 timestamp.
fn extract_hhmm(rfc3339: &str) -> Option<String> {
    if rfc3339.len() >= 16 {
        Some(rfc3339[11..16].to_string())
    } else {
        None
    }
}

/// Extract YYYY-MM-DD from an RFC 3339 timestamp.
pub fn extract_date(rfc3339: &str) -> Option<&str> {
    if rfc3339.len() >= 10 {
        Some(&rfc3339[..10])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_group(
        start: &str,
        end: &str,
        app: &str,
        project: Option<&str>,
        dur_sec: u64,
        files: Vec<&str>,
        langs: Vec<&str>,
    ) -> ActivityGroup {
        ActivityGroup {
            start_time: start.to_string(),
            end_time: end.to_string(),
            project: project.map(|s| s.to_string()),
            app: app.to_string(),
            total_duration_sec: dur_sec,
            files_touched: files.into_iter().map(|s| s.to_string()).collect(),
            languages: langs.into_iter().map(|s| s.to_string()).collect(),
            terminal_workflows: vec![],
            git_summary: None,
        }
    }

    #[test]
    fn test_empty_groups() {
        let s = format_timeline_summary(&[], "2026-05-14");
        assert!(s.contains("No activity recorded"));
    }

    #[test]
    fn test_single_group() {
        let groups = vec![make_group(
            "2026-05-14T09:15:00+00:00",
            "2026-05-14T10:30:00+00:00",
            "antigravity",
            Some("tracker"),
            4500,
            vec!["main.rs"],
            vec!["rust"],
        )];
        let s = format_timeline_summary(&groups, "2026-05-14");
        assert!(s.contains("09:15"));
        assert!(s.contains("10:30"));
        assert!(s.contains("antigravity"));
        assert!(s.contains("tracker"));
        assert!(s.contains("main.rs"));
        assert!(s.contains("rust"));
        assert!(s.contains("1h 15m"));
    }

    #[test]
    fn test_multiple_groups_sorted() {
        let groups = vec![
            make_group(
                "2026-05-14T10:00:00+00:00", "2026-05-14T10:30:00+00:00",
                "firefox", None, 1800, vec![], vec![],
            ),
            make_group(
                "2026-05-14T09:00:00+00:00", "2026-05-14T09:30:00+00:00",
                "antigravity", Some("tracker"), 1800, vec![], vec![],
            ),
        ];
        let s = format_timeline_summary(&groups, "2026-05-14");
        // First group in output should be the 09:00 one (sorted)
        let pos_09 = s.find("09:00").unwrap();
        let pos_10 = s.find("10:00").unwrap();
        assert!(pos_09 < pos_10);
    }

    #[test]
    fn test_summary_stats() {
        let groups = vec![
            make_group(
                "2026-05-14T09:00:00+00:00", "2026-05-14T10:00:00+00:00",
                "antigravity", Some("tracker"), 3600, vec!["main.rs"], vec!["rust"],
            ),
            make_group(
                "2026-05-14T10:00:00+00:00", "2026-05-14T11:00:00+00:00",
                "code-oss", Some("py-analyzer"), 3600, vec!["analyzer.py"], vec!["python"],
            ),
        ];
        let s = format_timeline_summary(&groups, "2026-05-14");
        assert!(s.contains("2h 0m 0s"));
        assert!(s.contains("tracker"));
        assert!(s.contains("py-analyzer"));
        assert!(s.contains("rust"));
        assert!(s.contains("python"));
    }

    #[test]
    fn test_extract_date() {
        assert_eq!(extract_date("2026-05-14T09:15:00+00:00"), Some("2026-05-14"));
        assert_eq!(extract_date("short"), None);
    }
}
