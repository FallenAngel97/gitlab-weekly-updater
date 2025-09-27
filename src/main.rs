mod gitlab;

use gitlab::{GitLab, Commit};
use anyhow::Result;

fn is_meaningful_commit(commit: &Commit) -> bool {
    let msg = commit.message.to_lowercase();
    !msg.contains("update dependency gradle to")
        && !msg.contains("bump versionCode to")
        && !msg.contains("update dependency com.android.tools.build:gradle to")
}

fn main() -> Result<()> {
    let gl = GitLab::new()?;

    println!("Fetching latest commit...");
    let commit = match gl.latest_commit()? {
        Some(commit) => commit,
        None => {
            println!("No commits found.");
            return Ok(());
        }
    };

    println!("Latest commit: {} at {}", commit.id, commit.committed_date);
    if let Some(current_tag_commit_id) = gl.current_tag_commit_id()? {
        println!("Current tag commit ID: {}", current_tag_commit_id);

        let commits = gl.intermediate_commits(&current_tag_commit_id, &commit.id)?;
        let meaningful_commits: Vec<_> = commits
            .iter()
            .filter(|c| is_meaningful_commit(c))
            .collect();

        if meaningful_commits.is_empty() {
            println!("No meaningful commits since current tag. Skipping pipeline.");
            return Ok(());
        }
    } else {
        println!("No current tag found. Proceeding by default.");
    }

    println!("Triggering pipeline...");
    let pipeline_id = gl.trigger_pipeline()?;
    println!("Pipeline {} created.", pipeline_id);

    if let Some(job_id) = gl.first_job(pipeline_id)? {
        println!("Starting job {}...", job_id);
        gl.play_job(job_id)?;
        println!("Job started successfully.");
    } else {
        println!("No jobs found in the pipeline.");
    }

    Ok(())
}

