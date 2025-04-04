require("dotenv").config();
const https = require("https");

// Load environment variables
const GITLAB_API_URL = "gitlab.com";
const PROJECT_ID = process.env.GITLAB_PROJECT_ID;
const PRIVATE_TOKEN = process.env.GITLAB_PRIVATE_TOKEN;
const BRANCH = "master";
const WEEKS_LIMIT = 2;

// Helper function for making GET/POST requests
function makeRequest(method, path, data = null) {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: GITLAB_API_URL,
      path,
      method,
      headers: {
        "Private-Token": PRIVATE_TOKEN,
        "Content-Type": "application/json",
      },
    };

    const req = https.request(options, (res) => {
      let body = "";
      res.on("data", (chunk) => (body += chunk));
      res.on("end", () => {
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(new Error(`Invalid JSON response: ${body}`));
        }
      });
    });

    req.on("error", reject);
    if (data) req.write(JSON.stringify(data));
    req.end();
  });
}

// Function to fetch the latest commit
async function getLatestCommit() {
  const path = `/api/v4/projects/${PROJECT_ID}/repository/commits?ref_name=${BRANCH}&per_page=1`;
  const commits = await makeRequest("GET", path);
  return commits.length ? commits[0] : null;
}

// Function to check if commit is recent
function isRecentCommit(commitDate) {
  const commitTime = new Date(commitDate).getTime();
  const weeksAgo = Date.now() - WEEKS_LIMIT * 7 * 24 * 60 * 60 * 1000;
  return commitTime > weeksAgo;
}

// Function to trigger a pipeline
async function triggerPipeline() {
  const path = `/api/v4/projects/${PROJECT_ID}/pipeline`;
  const response = await makeRequest("POST", path, { ref: BRANCH });
  return response.id;
}

// Function to get first job in a pipeline
async function getFirstJob(pipelineId) {
  const path = `/api/v4/projects/${PROJECT_ID}/pipelines/${pipelineId}/jobs`;
  const jobs = await makeRequest("GET", path);
  return jobs.length ? jobs[0].id : null;
}

// Function to play a job
async function playJob(jobId) {
  const path = `/api/v4/projects/${PROJECT_ID}/jobs/${jobId}/play`;
  await makeRequest("POST", path);
  console.log(`Job ${jobId} started successfully.`);
}

// Main function
(async () => {
  try {
    console.log("Fetching latest commit...");
    const latestCommit = await getLatestCommit();

    if (!latestCommit) {
      console.log("No commits found.");
      return;
    }

    console.log(`Latest commit: ${latestCommit.id} on ${latestCommit.committed_date}`);
    if (!isRecentCommit(latestCommit.committed_date)) {
      console.log("Latest commit is too old. No action taken.");
      return;
    }

    console.log("Triggering pipeline...");
    const pipelineId = await triggerPipeline();
    console.log(`Pipeline ${pipelineId} triggered.`);

    console.log("Fetching first job...");
    const jobId = await getFirstJob(pipelineId);

    if (!jobId) {
      console.log("No jobs found in the pipeline.");
      return;
    }

    console.log(`Playing job ${jobId}...`);
    await playJob(jobId);
  } catch (error) {
    console.error("Error:", error.message);
  }
})();

