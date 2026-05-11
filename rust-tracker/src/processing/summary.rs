// processing/summary.rs
//
// Formats ActivityGroup data into neutral, semantic summaries.
// No judgment, no scores — just observable activity reconstruction.

use crate::models::activity::ActivityGroup;

/// Formats a list of activity groups into a human-readable report.
pub fn format_report(groups: &[ActivityGroup]) -> String {
    if groups.is_empty() {
        return "No activities recorded in the current session.".to_string();
    }

    let mut report = String::new();
    report.push_str("--- Activity Reconstruction Report ---\n\n");

    for group in groups {
        report.push_str(&format_group(group));
        report.push_str("\n---\n\n");
    }

    report
}

/// Formats a single ActivityGroup into the requested semantic format.
pub fn format_group(group: &ActivityGroup) -> String {
    let mut out = String::new();

    // 1. Context / Project
    if let Some(ref project) = group.project {
        out.push_str(&format!("Used {} in:\n{}\n\n", group.app, project));
    } else {
        out.push_str(&format!("Used {}\n\n", group.app));
    }

    // 2. Files Worked On
    if !group.files_touched.is_empty() {
        out.push_str("Worked on:\n");
        for file in &group.files_touched {
            out.push_str(&format!("- {}\n", file));
        }
        out.push_str("\n");
    }

    // 3. Time Spent
    let minutes = group.total_duration_sec / 60;
    let seconds = group.total_duration_sec % 60;
    out.push_str("Time spent:\n");
    if minutes > 0 {
        out.push_str(&format!("{} minutes", minutes));
        if seconds > 0 {
            out.push_str(&format!(", {} seconds", seconds));
        }
    } else {
        out.push_str(&format!("{} seconds", seconds));
    }
    out.push_str("\n\n");

    // 4. Terminal Activity
    if !group.terminal_workflows.is_empty() {
        out.push_str("Terminal activity:\n");
        for wf in &group.terminal_workflows {
            out.push_str(&format!("- {}\n", wf));
        }
        out.push_str("\n");
    }

    // 5. Git Activity
    if let Some(ref git) = group.git_summary {
        out.push_str("Git activity:\n");
        if git.commit_count > 0 {
            out.push_str(&format!("- {} commits\n", git.commit_count));
        }
        if git.unpushed > 0 {
            out.push_str(&format!("- {} unpushed commits\n", git.unpushed));
        }
        if !git.dev_areas.is_empty() {
            out.push_str("- Development areas: ");
            out.push_str(&git.dev_areas.join(", "));
            out.push_str("\n");
        }
    }

    out
}
