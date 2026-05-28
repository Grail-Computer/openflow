use crate::agent::{AgentExec, run_agent_exec};
use crate::plan::{Task, normalize_plan};
use crate::schema::verifier_schema;
use crate::state::{RunState, TaskState, VerifierResult, add_event, save_state};
use crate::util::{
    ensure_dir, now, parse_json_object, read_snippet, relative_to, write_json, write_text,
};
use crate::worktree::{capture_patch, prepare_task_workspace};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

#[derive(Debug, Clone)]
pub struct RunnerOptions {
    pub concurrency: usize,
    pub max_retries: usize,
    pub model: Option<String>,
    pub agent: String,
    pub agent_bin: String,
    pub agent_command: Option<String>,
    pub skip_git_repo_check: bool,
}

#[derive(Debug)]
struct TaskOutcome {
    task_id: String,
    task_state: TaskState,
    events: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct StepAgentConfig {
    agent: String,
    agent_bin: String,
    agent_command: Option<String>,
    model: Option<String>,
    sandbox: String,
}

pub fn run_workflow(state: &mut RunState, run_dir: &Path, options: &RunnerOptions) -> Result<()> {
    let plan = state.plan.clone().context("run has no plan")?;
    let normalized = normalize_plan(plan, options.concurrency)?;
    state.plan = Some(normalized);
    state.status = "running".to_string();
    add_event(state, "run.started", "Workflow execution started", None);
    save_state(run_dir, state)?;

    loop {
        let ready = ready_tasks(state);
        if ready.is_empty() {
            break;
        }
        let limit = options
            .concurrency
            .min(state.plan.as_ref().unwrap().max_concurrency)
            .max(1);
        let batch = ready.into_iter().take(limit).collect::<Vec<_>>();
        for task in &batch {
            if let Some(task_state) = state.tasks.get_mut(&task.id) {
                task_state.status = "running".to_string();
                task_state.started_at = Some(now());
            }
            println!("started {}: {}", task.id, task.title);
        }
        save_state(run_dir, state)?;

        let mut failed = None;
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for task in batch {
                let task_id = task.id.clone();
                let snapshot = state.clone();
                let run_dir = run_dir.to_path_buf();
                let options = options.clone();
                handles.push((
                    task_id,
                    scope.spawn(move || execute_task(snapshot, run_dir, task, options)),
                ));
            }
            for (task_id, handle) in handles {
                match handle.join().expect("worker thread panicked") {
                    Ok(outcome) => {
                        println!("{} {}", outcome.task_state.status, outcome.task_id);
                        for (kind, message) in outcome.events {
                            add_event(state, &kind, &message, Some(outcome.task_id.clone()));
                        }
                        state.tasks.insert(outcome.task_id, outcome.task_state);
                    }
                    Err(error) => {
                        let message = format!("{error:#}");
                        println!("failed {task_id}");
                        mark_task_failed(state, &task_id, message.clone());
                        add_event(state, "task.failed", &message, Some(task_id));
                        failed = Some(message);
                    }
                }
            }
        });
        save_state(run_dir, state)?;
        if let Some(error) = failed {
            state.status = "failed".to_string();
            add_event(state, "run.failed", &error, None);
            save_state(run_dir, state)?;
            return Ok(());
        }
    }

    if let Some(task) = state.tasks.values().find(|task| task.status == "failed") {
        state.status = "failed".to_string();
        add_event(
            state,
            "run.failed",
            &format!("Task {} failed", task.id),
            None,
        );
    } else if state
        .tasks
        .values()
        .all(|task| task.status == "completed" || task.status == "skipped")
    {
        state.status = "completed".to_string();
        add_event(state, "run.completed", "Workflow execution completed", None);
    } else {
        state.status = "blocked".to_string();
        add_event(state, "run.blocked", "No runnable tasks remain", None);
    }
    save_state(run_dir, state)?;
    Ok(())
}

fn mark_task_failed(state: &mut RunState, task_id: &str, error: String) {
    if let Some(task_state) = state.tasks.get_mut(task_id) {
        task_state.status = "failed".to_string();
        task_state.error = Some(error);
        task_state.completed_at = Some(now());
    }
}

pub fn ready_tasks(state: &RunState) -> Vec<Task> {
    let Some(plan) = &state.plan else {
        return Vec::new();
    };
    plan.tasks
        .iter()
        .filter(|task| {
            state
                .tasks
                .get(&task.id)
                .is_some_and(|task_state| task_state.status == "pending")
                && task.depends_on.iter().all(|dep| {
                    state
                        .tasks
                        .get(dep)
                        .is_some_and(|task_state| task_state.status == "completed")
                })
        })
        .cloned()
        .collect()
}

fn execute_task(
    state: RunState,
    run_dir: PathBuf,
    task: Task,
    options: RunnerOptions,
) -> Result<TaskOutcome> {
    let mut task_state = state
        .tasks
        .get(&task.id)
        .cloned()
        .with_context(|| format!("missing task state for {}", task.id))?;
    let mut events = Vec::new();
    let mut verifier_feedback = None;
    let max_retries = task.max_retries.unwrap_or_else(|| {
        state
            .plan
            .as_ref()
            .map(|plan| plan.verification.max_retries)
            .unwrap_or(options.max_retries)
    });

    while task_state.attempts <= max_retries {
        task_state.attempts += 1;
        task_state.status = "running".to_string();
        task_state.error = None;
        task_state.started_at = Some(now());
        let started = std::time::Instant::now();
        events.push((
            "task.started".to_string(),
            format!("Started {}", task.title),
        ));

        let attempt_dir = run_dir
            .join("tasks")
            .join(&task.id)
            .join(format!("attempt-{}", task_state.attempts));
        ensure_dir(&attempt_dir)?;
        let task_workspace = prepare_task_workspace(&state.cwd, &run_dir, &task.id, task.writes)?;
        let result_path = attempt_dir.join("result.md");
        let prompt = build_worker_prompt(&state, &run_dir, &task, verifier_feedback.as_deref())?;
        write_text(&attempt_dir.join("prompt.md"), &prompt)?;
        let worker_config = worker_agent_config(&state, &task, &options)?;

        run_agent_exec(&AgentExec {
            agent: worker_config.agent,
            agent_bin: worker_config.agent_bin,
            agent_command: worker_config.agent_command,
            cwd: task_workspace.clone(),
            prompt,
            prompt_file: Some(attempt_dir.join("prompt.md")),
            sandbox: worker_config.sandbox,
            model: worker_config.model,
            output_file: Some(result_path.clone()),
            schema_file: None,
            log_file: Some(attempt_dir.join("worker.log")),
            skip_git_repo_check: options.skip_git_repo_check,
        })?;

        task_state.result_path = Some(relative_to(&run_dir, &result_path));
        if task.writes {
            let patch_path = run_dir.join("patches").join(format!("{}.diff", task.id));
            if capture_patch(&task_workspace, &patch_path)? {
                task_state.patch_path = Some(relative_to(&run_dir, &patch_path));
            }
        }

        let verifier = maybe_verify_task(
            &state,
            &run_dir,
            &task,
            &result_path,
            &attempt_dir,
            &options,
        )?;
        task_state.verifier = verifier.clone();
        task_state.duration_ms = Some(started.elapsed().as_millis());

        if verifier
            .as_ref()
            .is_none_or(|verifier| verifier.status == "pass")
        {
            task_state.status = "completed".to_string();
            task_state.completed_at = Some(now());
            events.push((
                "task.completed".to_string(),
                format!("Completed {}", task.title),
            ));
            return Ok(TaskOutcome {
                task_id: task.id,
                task_state,
                events,
            });
        }

        let verifier = verifier.unwrap();
        verifier_feedback = Some(verifier.summary.clone());
        if task_state.attempts > max_retries {
            task_state.status = "failed".to_string();
            task_state.error = Some(format!(
                "Verifier returned {}: {}",
                verifier.status, verifier.summary
            ));
            events.push((
                "task.verifier_failed".to_string(),
                task_state.error.clone().unwrap(),
            ));
            return Ok(TaskOutcome {
                task_id: task.id,
                task_state,
                events,
            });
        }
        events.push((
            "task.retry".to_string(),
            format!("Retrying after verifier feedback: {}", verifier.summary),
        ));
    }

    Ok(TaskOutcome {
        task_id: task.id,
        task_state,
        events,
    })
}

fn maybe_verify_task(
    state: &RunState,
    run_dir: &Path,
    task: &Task,
    result_path: &Path,
    attempt_dir: &Path,
    options: &RunnerOptions,
) -> Result<Option<VerifierResult>> {
    let plan = state.plan.as_ref().context("missing plan")?;
    let verifier_count = task
        .verifiers_per_task
        .unwrap_or(plan.verification.verifiers_per_task);
    if !task.verify || plan.verification.strategy == "none" || verifier_count == 0 {
        return Ok(None);
    }

    let schema_path = run_dir.join("schemas").join("verifier.schema.json");
    write_json(&schema_path, &verifier_schema())?;
    let mut results = Vec::new();

    let verifier_config = verifier_agent_config(state, task, options)?;
    for index in 0..verifier_count {
        let prompt = build_verifier_prompt(state, task, result_path, index + 1, verifier_count)?;
        write_text(
            &attempt_dir.join(format!("verifier-{}-prompt.md", index + 1)),
            &prompt,
        )?;
        let output_path = attempt_dir.join(format!("verifier-{}.json", index + 1));
        run_agent_exec(&AgentExec {
            agent: verifier_config.agent.clone(),
            agent_bin: verifier_config.agent_bin.clone(),
            agent_command: verifier_config.agent_command.clone(),
            cwd: state.cwd.clone(),
            prompt,
            prompt_file: Some(attempt_dir.join(format!("verifier-{}-prompt.md", index + 1))),
            sandbox: verifier_config.sandbox.clone(),
            model: verifier_config.model.clone(),
            output_file: Some(output_path.clone()),
            schema_file: Some(schema_path.clone()),
            log_file: Some(attempt_dir.join(format!("verifier-{}.log", index + 1))),
            skip_git_repo_check: options.skip_git_repo_check,
        })?;
        let raw = fs::read_to_string(&output_path)?;
        results.push(normalize_verifier(parse_json_object::<VerifierResult>(
            &raw,
            "verifier result",
        )?));
    }

    Ok(Some(aggregate_verifiers(results)))
}

fn build_worker_prompt(
    state: &RunState,
    run_dir: &Path,
    task: &Task,
    verifier_feedback: Option<&str>,
) -> Result<String> {
    let plan = state.plan.as_ref().context("missing plan")?;
    let mut dependencies = Vec::new();
    for dep in &task.depends_on {
        if let Some(path) = state
            .tasks
            .get(dep)
            .and_then(|task| task.result_path.as_ref())
        {
            dependencies.push(format!(
                "## Dependency {dep}\n{}",
                read_snippet(&run_dir.join(path), 8_000)?
            ));
        }
    }

    Ok(format!(
        "You are an agent worker in an Openflow dynamic workflow.\n\n\
Workflow objective: {}\n\
Task id: {}\n\
Task title: {}\n\
Task kind: {}\n\
Task role: {}\n\
Expected output: {}\n\
Allowed to edit files: {}\n\
Scope: {}\n\n\
Task prompt:\n{}\n\n\
{}\n\n\
Previous verifier feedback: {}\n\n\
Return only the task result. Cite concrete files, commands, and evidence when relevant.\n",
        plan.objective,
        task.id,
        task.title,
        task.kind,
        task.role,
        task.expected_output,
        if task.writes { "yes" } else { "no" },
        if task.scope.is_empty() {
            "inspect the repository as needed".to_string()
        } else {
            task.scope.join(", ")
        },
        task.prompt,
        if dependencies.is_empty() {
            "Dependency outputs: none.".to_string()
        } else {
            dependencies.join("\n\n")
        },
        verifier_feedback.unwrap_or("none")
    ))
}

fn build_verifier_prompt(
    state: &RunState,
    task: &Task,
    result_path: &Path,
    verifier_index: usize,
    verifier_count: usize,
) -> Result<String> {
    let plan = state.plan.as_ref().context("missing plan")?;
    Ok(format!(
        "You are an independent verifier in an Openflow dynamic workflow.\n\
Verifier {verifier_index} of {verifier_count}.\n\n\
Your job is to check the worker result before it is accepted.\n\n\
Workflow objective: {}\n\
Verification policy: {}\n\
Task id: {}\n\
Task title: {}\n\
Task prompt: {}\n\n\
Worker result:\n{}\n\n\
Return JSON only with status, summary, confidence, acceptedFindings, rejectedFindings, and requiredChanges.\n",
        plan.objective,
        task.verification_prompt
            .as_deref()
            .unwrap_or(&plan.verification.prompt),
        task.id,
        task.title,
        task.prompt,
        read_snippet(result_path, 24_000)?
    ))
}

fn normalize_verifier(mut verifier: VerifierResult) -> VerifierResult {
    if !matches!(verifier.status.as_str(), "pass" | "revise" | "fail") {
        verifier.status = "fail".to_string();
    }
    verifier.confidence = verifier.confidence.clamp(0.0, 1.0);
    verifier
}

fn worker_agent_config(
    state: &RunState,
    task: &Task,
    options: &RunnerOptions,
) -> Result<StepAgentConfig> {
    let plan = state.plan.as_ref().context("missing plan")?;
    let agent = first_nonempty([
        task.agent.as_deref(),
        plan.defaults.agent.as_deref(),
        Some(options.agent.as_str()),
    ])
    .unwrap_or("codex")
    .to_string();
    let fallback_agent_bin = default_agent_bin(&agent, options);
    let agent_bin = first_nonempty([
        task.agent_bin.as_deref(),
        plan.defaults.agent_bin.as_deref(),
        Some(fallback_agent_bin.as_str()),
    ])
    .unwrap_or("codex")
    .to_string();
    let agent_command = first_nonempty_owned([
        task.agent_command.clone(),
        plan.defaults.agent_command.clone(),
        options.agent_command.clone(),
    ]);
    let model = first_nonempty_owned([
        task.model.clone(),
        plan.defaults.model.clone(),
        options.model.clone(),
    ]);
    let sandbox = task
        .sandbox
        .clone()
        .or_else(|| {
            if task.writes {
                plan.defaults.write_sandbox.clone()
            } else {
                plan.defaults.sandbox.clone()
            }
        })
        .unwrap_or_else(|| {
            if task.writes {
                "workspace-write".to_string()
            } else {
                "read-only".to_string()
            }
        });
    Ok(StepAgentConfig {
        agent,
        agent_bin,
        agent_command,
        model,
        sandbox,
    })
}

fn verifier_agent_config(
    state: &RunState,
    task: &Task,
    options: &RunnerOptions,
) -> Result<StepAgentConfig> {
    let plan = state.plan.as_ref().context("missing plan")?;
    let agent = first_nonempty([
        task.verifier_agent.as_deref(),
        plan.defaults.verifier_agent.as_deref(),
        task.agent.as_deref(),
        plan.defaults.agent.as_deref(),
        Some(options.agent.as_str()),
    ])
    .unwrap_or("codex")
    .to_string();
    let fallback_agent_bin = default_agent_bin(&agent, options);
    let agent_bin = first_nonempty([
        task.verifier_agent_bin.as_deref(),
        plan.defaults.verifier_agent_bin.as_deref(),
        task.agent_bin.as_deref(),
        plan.defaults.agent_bin.as_deref(),
        Some(fallback_agent_bin.as_str()),
    ])
    .unwrap_or("codex")
    .to_string();
    let agent_command = first_nonempty_owned([
        task.verifier_agent_command.clone(),
        plan.defaults.verifier_agent_command.clone(),
        task.agent_command.clone(),
        plan.defaults.agent_command.clone(),
        options.agent_command.clone(),
    ]);
    let model = first_nonempty_owned([
        task.verifier_model.clone(),
        plan.defaults.verifier_model.clone(),
        task.model.clone(),
        plan.defaults.model.clone(),
        options.model.clone(),
    ]);
    let sandbox = task
        .verifier_sandbox
        .clone()
        .or_else(|| plan.defaults.verifier_sandbox.clone())
        .unwrap_or_else(|| "read-only".to_string());
    Ok(StepAgentConfig {
        agent,
        agent_bin,
        agent_command,
        model,
        sandbox,
    })
}

fn first_nonempty<const N: usize>(values: [Option<&str>; N]) -> Option<&str> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn first_nonempty_owned<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn default_agent_bin(agent: &str, options: &RunnerOptions) -> String {
    if agent == options.agent {
        options.agent_bin.clone()
    } else {
        agent.to_string()
    }
}

fn aggregate_verifiers(results: Vec<VerifierResult>) -> VerifierResult {
    if results.len() == 1 {
        return results.into_iter().next().unwrap();
    }
    let status = if results.iter().any(|result| result.status == "fail") {
        "fail"
    } else if results.iter().any(|result| result.status == "revise") {
        "revise"
    } else {
        "pass"
    }
    .to_string();
    VerifierResult {
        status,
        summary: results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                format!(
                    "Verifier {}: {} - {}",
                    index + 1,
                    result.status,
                    result.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        confidence: results.iter().map(|result| result.confidence).sum::<f64>()
            / results.len() as f64,
        accepted_findings: dedupe(
            results
                .iter()
                .flat_map(|result| result.accepted_findings.clone())
                .collect(),
        ),
        rejected_findings: dedupe(
            results
                .iter()
                .flat_map(|result| result.rejected_findings.clone())
                .collect(),
        ),
        required_changes: dedupe(
            results
                .iter()
                .flat_map(|result| result.required_changes.clone())
                .collect(),
        ),
    }
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::WorkflowPlan;
    use crate::state::{RunOptions, attach_plan, create_empty_run, save_state};
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn verifier_rejection_marks_task_and_run_failed() {
        let temp = TempDir::new().unwrap();
        let fake = executable_agent(
            temp.path(),
            r#"#!/bin/sh
set -eu
case "$OPENFLOW_OUTPUT_FILE" in
  *verifier-*.json)
    cat > "$OPENFLOW_OUTPUT_FILE" <<'JSON'
{"status":"fail","summary":"The worker result is unsupported.","confidence":0.92,"acceptedFindings":[],"rejectedFindings":["Unsupported claim"],"requiredChanges":["Cite concrete evidence"]}
JSON
    ;;
  *)
    echo "Speculative worker result." > "$OPENFLOW_OUTPUT_FILE"
    ;;
esac
"#,
        );
        let (mut state, run_dir) = prepared_run(&temp, true);
        let options = runner_options(&fake);

        run_workflow(&mut state, &run_dir, &options).unwrap();

        let task = state.tasks.get("inspect").unwrap();
        assert_eq!(state.status, "failed");
        assert_eq!(task.status, "failed");
        assert_eq!(task.attempts, 1);
        assert!(
            task.error
                .as_deref()
                .unwrap()
                .contains("Verifier returned fail")
        );
        assert!(
            state
                .events
                .iter()
                .any(|event| event.kind == "run.failed" && event.message.contains("inspect"))
        );
    }

    #[test]
    fn worker_command_error_is_persisted_on_task() {
        let temp = TempDir::new().unwrap();
        let fake = executable_agent(
            temp.path(),
            r#"#!/bin/sh
set -eu
echo "worker exploded" >&2
exit 42
"#,
        );
        let (mut state, run_dir) = prepared_run(&temp, false);
        let options = runner_options(&fake);

        run_workflow(&mut state, &run_dir, &options).unwrap();

        let task = state.tasks.get("inspect").unwrap();
        assert_eq!(state.status, "failed");
        assert_eq!(task.status, "failed");
        assert!(task.error.as_deref().unwrap().contains("status 42"));
        assert!(task.error.as_deref().unwrap().contains("worker exploded"));
    }

    fn prepared_run(temp: &TempDir, verify: bool) -> (RunState, PathBuf) {
        let (mut state, run_dir) = create_empty_run(
            temp.path().to_path_buf(),
            "workflow: test failure semantics".to_string(),
            None,
            RunOptions {
                concurrency: 1,
                max_retries: 0,
                model: None,
                agent: "custom".to_string(),
                agent_bin: "custom".to_string(),
                agent_command: Some("fake".to_string()),
                skip_git_repo_check: true,
            },
        )
        .unwrap();
        attach_plan(&mut state, one_task_plan(verify));
        save_state(&run_dir, &mut state).unwrap();
        (state, run_dir)
    }

    fn one_task_plan(verify: bool) -> WorkflowPlan {
        serde_json::from_value(json!({
            "version": 1,
            "name": "Failure semantics",
            "objective": "Exercise failed task handling",
            "riskLevel": "low",
            "maxConcurrency": 1,
            "tasks": [{
                "id": "inspect",
                "title": "Inspect",
                "kind": "explore",
                "prompt": "Return a result.",
                "expectedOutput": "markdown",
                "writes": false,
                "verify": verify
            }],
            "verification": {
                "strategy": if verify { "independent" } else { "none" },
                "verifiersPerTask": if verify { 1 } else { 0 },
                "maxRetries": 0,
                "prompt": "Reject unsupported results."
            }
        }))
        .unwrap()
    }

    fn runner_options(fake: &Path) -> RunnerOptions {
        RunnerOptions {
            concurrency: 1,
            max_retries: 0,
            model: None,
            agent: "custom".to_string(),
            agent_bin: "custom".to_string(),
            agent_command: Some(fake.display().to_string()),
            skip_git_repo_check: true,
        }
    }

    fn executable_agent(root: &Path, script: &str) -> PathBuf {
        let path = root.join("fake-agent");
        fs::write(&path, script).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }
}
