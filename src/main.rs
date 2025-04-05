mod gitlab;

use gitlab::GitLab;
use anyhow::Result;

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
    if !commit.is_recent(gl.commit_age_weeks) {
        println!("Commit is too old.");
        return Ok(());
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

