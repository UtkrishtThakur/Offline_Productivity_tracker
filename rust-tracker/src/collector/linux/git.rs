use serde_json::json;

use std::process::Command;

pub fn get_git_activity() -> Option<serde_json::Value> {

    // Repo root
    let repo_output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    let repo =
        String::from_utf8_lossy(
            &repo_output.stdout
        )
        .trim()
        .to_string();

    if repo.is_empty() {
        return None;
    }

    // Branch
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;

    let branch =
        String::from_utf8_lossy(
            &branch_output.stdout
        )
        .trim()
        .to_string();

    // Git status
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()?;

    let changes =
        String::from_utf8_lossy(
            &status_output.stdout
        );

    let changed_files:
        Vec<String> =
            changes
                .lines()
                .map(|l| l.to_string())
                .collect();

    // Last commit
    let commit_output = Command::new("git")
        .args([
            "log",
            "-1",
            "--pretty=%H|%s"
        ])
        .output()
        .ok()?;

    let commit_raw =
        String::from_utf8_lossy(
            &commit_output.stdout
        );

    let parts:
        Vec<&str> =
            commit_raw
                .trim()
                .split('|')
                .collect();

    let commit_hash =
        parts.get(0)
            .unwrap_or(&"")
            .to_string();

    let commit_message =
        parts.get(1)
            .unwrap_or(&"")
            .to_string();

    // Upstream difference
    let push_output = Command::new("git")
        .args([
            "rev-list",
            "--left-right",
            "--count",
            "HEAD...@{upstream}"
        ])
        .output()
        .ok();

    let mut ahead = 0;

    if let Some(out) = push_output {

        let txt =
            String::from_utf8_lossy(
                &out.stdout
            );

        let nums:
            Vec<&str> =
                txt.trim()
                    .split_whitespace()
                    .collect();

        if nums.len() == 2 {

            ahead =
                nums[0]
                    .parse::<i32>()
                    .unwrap_or(0);
        }
    }

    Some(json!({

        "repo": repo,

        "branch": branch,

        "changed_files":
            changed_files,

        "changed_count":
            changed_files.len(),

        "last_commit_hash":
            commit_hash,

        "last_commit_message":
            commit_message,

        "unpushed_commits":
            ahead,
    }))
}