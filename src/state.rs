use crate::plan::WorkflowPlan;
use crate::util::{ensure_dir, make_run_id, now, read_json, write_json};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const FLOW_DIR: &str = ".openflow";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunState {
    pub version: u32,
    pub id: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub template: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub options: RunOptions,
    #[serde(default)]
    pub observations: Vec<RunObservation>,
    pub plan: Option<WorkflowPlan>,
    pub tasks: BTreeMap<String, TaskState>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOptions {
    pub concurrency: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    pub model: Option<String>,
    pub agent: String,
    pub agent_bin: String,
    pub agent_command: Option<String>,
    pub skip_git_repo_check: bool,
    #[serde(default)]
    pub brake_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunObservation {
    pub source: String,
    pub path: PathBuf,
    pub captured_at: String,
    pub content: String,
}

fn default_max_retries() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskState {
    pub id: String,
    pub title: String,
    pub status: String,
    pub attempts: usize,
    pub result_path: Option<PathBuf>,
    pub patch_path: Option<PathBuf>,
    pub verifier: Option<VerifierResult>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u128>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifierResult {
    pub status: String,
    pub summary: String,
    pub confidence: f64,
    #[serde(default)]
    pub accepted_findings: Vec<String>,
    #[serde(default)]
    pub rejected_findings: Vec<String>,
    #[serde(default)]
    pub required_changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub at: String,
    pub kind: String,
    pub task_id: Option<String>,
    pub message: String,
}

pub fn flow_dir(cwd: &Path) -> PathBuf {
    cwd.join(FLOW_DIR)
}
pub fn runs_dir(cwd: &Path) -> PathBuf {
    flow_dir(cwd).join("runs")
}
pub fn run_dir(cwd: &Path, id: &str) -> PathBuf {
    runs_dir(cwd).join(id)
}
pub fn state_file(run_dir: &Path) -> PathBuf {
    run_dir.join("state.json")
}

pub fn create_empty_run(
    cwd: PathBuf,
    prompt: String,
    template: Option<String>,
    options: RunOptions,
) -> Result<(RunState, PathBuf)> {
    let id = make_run_id(&prompt);
    let directory = run_dir(&cwd, &id);
    ensure_dir(&directory)?;
    let mut state = RunState {
        version: 1,
        id,
        cwd,
        prompt,
        template,
        status: "planning".to_string(),
        created_at: now(),
        updated_at: now(),
        options,
        observations: Vec::new(),
        plan: None,
        tasks: BTreeMap::new(),
        events: Vec::new(),
    };
    add_event(&mut state, "run.created", "Created run", None);
    save_state(&directory, &mut state)?;
    Ok((state, directory))
}

pub fn replace_observations(state: &mut RunState, observations: Vec<RunObservation>) {
    if observations.is_empty() {
        return;
    }
    let count = observations.len();
    state.observations = observations;
    add_event(
        state,
        "observations.updated",
        &format!("Captured {count} controller observation file(s)"),
        None,
    );
}

pub fn format_observations(observations: &[RunObservation], max_chars_per_file: usize) -> String {
    if observations.is_empty() {
        return "none".to_string();
    }
    observations
        .iter()
        .map(|observation| {
            let content = if observation.content.chars().count() <= max_chars_per_file {
                observation.content.clone()
            } else {
                let snippet = observation
                    .content
                    .chars()
                    .take(max_chars_per_file)
                    .collect::<String>();
                format!(
                    "{snippet}\n\n[truncated {} chars]",
                    observation.content.chars().count() - max_chars_per_file
                )
            };
            format!(
                "## {}: {}\nCaptured: {}\n\n{}",
                observation.source,
                observation.path.display(),
                observation.captured_at,
                content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn attach_plan(state: &mut RunState, plan: WorkflowPlan) {
    state.tasks = plan
        .tasks
        .iter()
        .map(|task| {
            (
                task.id.clone(),
                TaskState {
                    id: task.id.clone(),
                    title: task.title.clone(),
                    status: "pending".to_string(),
                    attempts: 0,
                    result_path: None,
                    patch_path: None,
                    verifier: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    error: None,
                },
            )
        })
        .collect();
    state.plan = Some(plan);
    state.status = "planned".to_string();
    add_event(state, "plan.created", "Created workflow plan", None);
}

pub fn add_event(state: &mut RunState, kind: &str, message: &str, task_id: Option<String>) {
    state.events.push(Event {
        at: now(),
        kind: kind.to_string(),
        task_id,
        message: message.to_string(),
    });
    state.updated_at = now();
}

pub fn save_state(run_dir: &Path, state: &mut RunState) -> Result<()> {
    state.updated_at = now();
    write_json(&state_file(run_dir), state)
}

pub fn load_state(run_dir: &Path) -> Result<RunState> {
    read_json(&state_file(run_dir))
}

pub fn latest_run_id(cwd: &Path) -> Result<Option<String>> {
    let directory = runs_dir(cwd);
    if !directory.exists() {
        return Ok(None);
    }
    let mut ids = fs::read_dir(&directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .file_type()
                .ok()?
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    ids.sort();
    Ok(ids.pop())
}

pub fn resolve_run_dir(cwd: &Path, maybe_run: Option<&str>) -> Result<PathBuf> {
    if let Some(run) = maybe_run {
        let path = if Path::new(run).is_absolute() {
            PathBuf::from(run)
        } else {
            run_dir(cwd, run)
        };
        if state_file(&path).exists() {
            return Ok(path);
        }
        bail!("run not found: {run}");
    }
    let id = latest_run_id(cwd)?
        .context("no openflow runs found. Start with `openflow run \"workflow: ...\"`.")?;
    Ok(run_dir(cwd, &id))
}
