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
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct Tag {
    name: String,
    commit: CommitRef,
}

#[derive(Debug, Deserialize)]
struct CommitRef {
    id: String,
}

impl Commit {}

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
        })
    }

    fn parse_version(tag: &str) -> Result<(u64, u64)> {
        let cleaned = tag.trim_start_matches('v');
        let parts: Vec<&str> = cleaned.split('.').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid tag format (expected vMAJOR.MINOR): {}", tag);
        }
        Ok((parts[0].parse()?, parts[1].parse()?))
    }

    pub fn intermediate_commits(&self, from: &str, to: &str) -> Result<Vec<Commit>> {
        let url = format!(
            "{}/projects/{}/repository/compare?from={}&to={}",
            GITLAB_API_URL,
            urlencoding::encode(&self.project_id),
            from,
            to
        );

        let response = CLIENT
            .get(&url)
            .headers(self.headers.clone())
            .send()?;

        let body = response.text()?;

        let parsed: serde_json::Value = serde_json::from_str(&body)?;
        let commits_json = parsed
            .get("commits")
            .ok_or_else(|| anyhow::anyhow!("Missing 'commits' in compare response"))?;

        let commits: Vec<Commit> = serde_json::from_value(commits_json.clone())?;
        Ok(commits)
    }

    pub fn current_tag_commit_id(&self) -> Result<Option<String>> {
        let url = format!("{}/projects/{}/repository/tags", GITLAB_API_URL, self.project_id);
        let tags: Vec<Tag> = CLIENT
            .get(&url)
            .headers(self.headers.clone())
            .send()?
            .json()?;

        let latest_tag = tags
            .iter()
            .find_map(|tag| Self::parse_version(&tag.name).ok().map(|v| (v, tag)))
            .map(|(_, tag)| tag.commit.id.clone());

        Ok(latest_tag)
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
