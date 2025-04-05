use std::{env, fs};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build reqwest client")
});

const GITLAB_API_URL: &str = "https://gitlab.com/api/v4";
const DEFAULT_BRANCH: &str = "master";

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub id: String,
    pub committed_date: DateTime<Utc>,
}

impl Commit {
    pub fn is_recent(&self, weeks: i64) -> bool {
        Utc::now() - self.committed_date < Duration::weeks(weeks)
    }
}

#[derive(Debug, Deserialize)]
struct PipelineResponse {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct Job {
    id: u64,
}

pub struct GitLab {
    project_id: String,
    token: String,
    branch: String,
    headers: HeaderMap,
    pub commit_age_weeks: i64,
}

impl GitLab {
    pub fn new() -> Result<Self> {
        if !Self::is_running_in_docker() {
            dotenv::dotenv().ok();
        }

        let token = env::var("GITLAB_PRIVATE_TOKEN").context("Missing GITLAB_PRIVATE_TOKEN")?;
        let project_id = env::var("GITLAB_PROJECT_ID").context("Missing GITLAB_PROJECT_ID")?;
        let branch = env::var("GITLAB_BRANCH").unwrap_or_else(|_| DEFAULT_BRANCH.into());

        let mut headers = HeaderMap::new();
        headers.insert("PRIVATE-TOKEN", HeaderValue::from_str(&token)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        Ok(Self {
            project_id,
            token,
            branch,
            headers,
            commit_age_weeks: 2,
        })
    }

    fn is_running_in_docker() -> bool {
        env::var("DOCKER").is_ok() || fs::read_to_string("/proc/1/cgroup").is_ok()
    }

    pub fn latest_commit(&self) -> Result<Option<Commit>> {
        let url = format!(
            "{}/projects/{}/repository/commits?ref_name={}&per_page=1",
            GITLAB_API_URL, self.project_id, self.branch
        );

        let commits: Vec<Commit> = CLIENT.get(&url)
            .headers(self.headers.clone())
            .send()?
            .json()?;

        Ok(commits.into_iter().next())
    }

    pub fn trigger_pipeline(&self) -> Result<u64> {
        let url = format!("{}/projects/{}/pipeline", GITLAB_API_URL, self.project_id);
        let payload = serde_json::json!({ "ref": self.branch });

        let pipeline: PipelineResponse = CLIENT.post(&url)
            .headers(self.headers.clone())
            .json(&payload)
            .send()?
            .json()?;

        Ok(pipeline.id)
    }

    pub fn first_job(&self, pipeline_id: u64) -> Result<Option<u64>> {
        let url = format!(
            "{}/projects/{}/pipelines/{}/jobs",
            GITLAB_API_URL, self.project_id, pipeline_id
        );

        let jobs: Vec<Job> = CLIENT.get(&url)
            .headers(self.headers.clone())
            .send()?
            .json()?;

        Ok(jobs.first().map(|j| j.id))
    }

    pub fn play_job(&self, job_id: u64) -> Result<()> {
        let url = format!("{}/projects/{}/jobs/{}/play", GITLAB_API_URL, self.project_id, job_id);

        CLIENT.post(&url)
            .headers(self.headers.clone())
            .send()?
            .error_for_status()?; // ensure success

        Ok(())
    }
}
