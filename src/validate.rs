use crate::plan::{Task, WorkflowPlan, normalize_plan};
use crate::state::{RunState, TaskState, load_state, state_file};
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckLevel {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Check {
    level: CheckLevel,
    name: String,
    detail: String,
}

#[derive(Debug, Clone, Default)]
struct ValidationReport {
    checks: Vec<Check>,
}

#[derive(Debug, Clone, Default)]
struct ArtifactCounts {
    results: usize,
    patches: usize,
    verifier_passes: usize,
}

pub fn run_validation(run_dir: &Path) -> Result<()> {
    let report = build_report(run_dir);
    print_report(&report);
    if report.failures() > 0 {
        bail!("validation found {} failing check(s)", report.failures());
    }
    Ok(())
}

fn build_report(run_dir: &Path) -> ValidationReport {
    let mut report = ValidationReport::default();
    let state_path = state_file(run_dir);
    if !state_path.exists() {
        report.push_fail("state", format!("missing {}", state_path.display()));
        return report;
    }

    match load_state(run_dir) {
        Ok(state) => {
            report.push_ok("state", format!("loaded {}", state_path.display()));
            validate_state(&mut report, run_dir, &state);
        }
        Err(error) => report.push_fail("state", format!("failed to parse state.json: {error:#}")),
    }
    report
}

fn validate_state(report: &mut ValidationReport, run_dir: &Path, state: &RunState) {
    report.push_ok("run", format!("{} is {}", state.id, state.status));

    if state.cwd.exists() {
        report.push_ok("workspace", state.cwd.display().to_string());
    } else {
        report.push_warn(
            "workspace",
            format!("{} no longer exists on this machine", state.cwd.display()),
        );
    }

    if let Some(dir_name) = run_dir.file_name().and_then(|name| name.to_str())
        && dir_name != state.id
    {
        report.push_warn(
            "run-dir",
            format!(
                "directory name {dir_name:?} differs from state id {:?}",
                state.id
            ),
        );
    }

    let Some(plan) = &state.plan else {
        if state.status == "planning" {
            report.push_ok("plan", "not created yet".to_string());
        } else {
            report.push_fail("plan", "missing plan in state.json".to_string());
        }
        return;
    };

    let normalized_plan = match normalize_plan(plan.clone(), state.options.concurrency) {
        Ok(plan) => {
            report.push_ok(
                "plan",
                format!("{} task(s), risk {}", plan.tasks.len(), plan.risk_level),
            );
            plan
        }
        Err(error) => {
            report.push_fail("plan", format!("invalid plan: {error:#}"));
            plan.clone()
        }
    };

    validate_task_state(report, &normalized_plan, state);
    validate_observations(report, state);
    let counts = validate_artifacts(report, run_dir, &normalized_plan, state);
    if counts.results > 0 {
        report.push_ok("results", format!("{} result artifact(s)", counts.results));
    }
    if counts.verifier_passes > 0 {
        report.push_ok(
            "verifiers",
            format!("{} accepted verifier result(s)", counts.verifier_passes),
        );
    }
    if counts.patches > 0 {
        report.push_ok("patches", format!("{} patch artifact(s)", counts.patches));
    }
    validate_report_artifact(report, run_dir, state);
}

fn validate_observations(report: &mut ValidationReport, state: &RunState) {
    if state.observations.is_empty() {
        return;
    }
    let empty = state
        .observations
        .iter()
        .filter(|observation| observation.content.trim().is_empty())
        .map(|observation| observation.path.display().to_string())
        .collect::<Vec<_>>();
    if empty.is_empty() {
        report.push_ok(
            "observations",
            format!("{} captured status file(s)", state.observations.len()),
        );
    } else {
        report.push_warn(
            "observations",
            format!("empty captured observation(s): {}", empty.join(", ")),
        );
    }
}

fn validate_task_state(report: &mut ValidationReport, plan: &WorkflowPlan, state: &RunState) {
    let plan_ids = plan
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<BTreeSet<_>>();
    let state_ids = state
        .tasks
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    for id in plan_ids.difference(&state_ids) {
        report.push_fail("tasks", format!("plan task {id:?} is missing from state"));
    }
    for id in state_ids.difference(&plan_ids) {
        report.push_warn("tasks", format!("state task {id:?} is not in the plan"));
    }

    if state
        .tasks
        .values()
        .any(|task| task.status == "running" && task.completed_at.is_some())
    {
        report.push_warn("tasks", "a running task has completed_at set".to_string());
    }

    if state.status == "completed" {
        let unfinished = state
            .tasks
            .values()
            .filter(|task| !matches!(task.status.as_str(), "completed" | "skipped"))
            .map(|task| task.id.clone())
            .collect::<Vec<_>>();
        if unfinished.is_empty() {
            report.push_ok("tasks", format!("{} tracked task(s)", state.tasks.len()));
        } else {
            report.push_fail(
                "tasks",
                format!(
                    "run is completed but these tasks are not: {}",
                    unfinished.join(", ")
                ),
            );
        }
    } else {
        report.push_ok("tasks", format!("{} tracked task(s)", state.tasks.len()));
    }
}

fn validate_artifacts(
    report: &mut ValidationReport,
    run_dir: &Path,
    plan: &WorkflowPlan,
    state: &RunState,
) -> ArtifactCounts {
    let mut counts = ArtifactCounts::default();
    for task in &plan.tasks {
        let Some(task_state) = state.tasks.get(&task.id) else {
            continue;
        };
        validate_result_artifact(report, run_dir, task, task_state, &mut counts);
        validate_patch_artifact(report, run_dir, task_state, &mut counts);
        validate_attempt_log(report, run_dir, task_state);
        validate_verifier(report, plan, task, task_state, &mut counts);
    }
    counts
}

fn validate_result_artifact(
    report: &mut ValidationReport,
    run_dir: &Path,
    task: &Task,
    task_state: &TaskState,
    counts: &mut ArtifactCounts,
) {
    let Some(path) = &task_state.result_path else {
        if task_state.status == "completed" {
            report.push_fail(
                format!("task:{}", task.id),
                "completed task is missing result_path".to_string(),
            );
        }
        return;
    };

    if artifact_exists(run_dir, path) {
        counts.results += 1;
    } else {
        report.push_fail(
            format!("task:{}", task.id),
            format!("missing result artifact {}", run_dir.join(path).display()),
        );
    }
}

fn validate_patch_artifact(
    report: &mut ValidationReport,
    run_dir: &Path,
    task_state: &TaskState,
    counts: &mut ArtifactCounts,
) {
    let Some(path) = &task_state.patch_path else {
        return;
    };
    if artifact_exists(run_dir, path) {
        counts.patches += 1;
    } else {
        report.push_fail(
            format!("task:{}", task_state.id),
            format!("missing patch artifact {}", run_dir.join(path).display()),
        );
    }
}

fn validate_attempt_log(report: &mut ValidationReport, run_dir: &Path, task_state: &TaskState) {
    if task_state.attempts == 0 {
        return;
    }
    let path = run_dir
        .join("tasks")
        .join(&task_state.id)
        .join(format!("attempt-{}", task_state.attempts))
        .join("worker.log");
    if !path.exists() {
        report.push_warn(
            format!("task:{}", task_state.id),
            format!("missing latest worker log {}", path.display()),
        );
    }
}

fn validate_verifier(
    report: &mut ValidationReport,
    plan: &WorkflowPlan,
    task: &Task,
    task_state: &TaskState,
    counts: &mut ArtifactCounts,
) {
    let verifier_count = task
        .verifiers_per_task
        .unwrap_or(plan.verification.verifiers_per_task);
    let verification_expected =
        task.verify && plan.verification.strategy != "none" && verifier_count > 0;
    if !verification_expected || task_state.status != "completed" {
        return;
    }

    match &task_state.verifier {
        Some(verifier) if verifier.status == "pass" => counts.verifier_passes += 1,
        Some(verifier) => report.push_fail(
            format!("task:{}", task.id),
            format!(
                "completed task has non-passing verifier status {}",
                verifier.status
            ),
        ),
        None => report.push_fail(
            format!("task:{}", task.id),
            "completed task is missing verifier result".to_string(),
        ),
    }
}

fn validate_report_artifact(report: &mut ValidationReport, run_dir: &Path, state: &RunState) {
    let report_path = run_dir.join("report.md");
    if report_path.exists() {
        report.push_ok("report", report_path.display().to_string());
    } else if matches!(state.status.as_str(), "completed" | "failed" | "blocked") {
        report.push_warn(
            "report",
            format!(
                "{} is missing; run `openflow report {}`",
                report_path.display(),
                state.id
            ),
        );
    }
}

fn artifact_exists(run_dir: &Path, path: &Path) -> bool {
    if path.is_absolute() {
        path.exists()
    } else {
        run_dir.join(path).exists()
    }
}

fn print_report(report: &ValidationReport) {
    println!("Openflow validate");
    for check in &report.checks {
        let label = match check.level {
            CheckLevel::Ok => "ok",
            CheckLevel::Warn => "warn",
            CheckLevel::Fail => "fail",
        };
        println!("{label:>4} {:<16} {}", check.name, check.detail);
    }
    println!(
        "\nSummary: {} ok, {} warn, {} fail",
        report.count(CheckLevel::Ok),
        report.count(CheckLevel::Warn),
        report.count(CheckLevel::Fail)
    );
}

impl ValidationReport {
    fn push_ok(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.push(CheckLevel::Ok, name, detail);
    }

    fn push_warn(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.push(CheckLevel::Warn, name, detail);
    }

    fn push_fail(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.push(CheckLevel::Fail, name, detail);
    }

    fn push(&mut self, level: CheckLevel, name: impl Into<String>, detail: impl Into<String>) {
        self.checks.push(Check {
            level,
            name: name.into(),
            detail: detail.into(),
        });
    }

    fn count(&self, level: CheckLevel) -> usize {
        self.checks
            .iter()
            .filter(|check| check.level == level)
            .count()
    }

    fn failures(&self) -> usize {
        self.count(CheckLevel::Fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{RunOptions, VerifierResult, attach_plan, create_empty_run, save_state};
    use crate::util::write_text;
    use serde_json::json;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn validates_completed_run_artifacts() {
        let temp = TempDir::new().unwrap();
        let (mut state, run_dir) = prepared_run(&temp);
        let result_path = PathBuf::from("tasks/inspect/attempt-1/result.md");
        write_text(&run_dir.join(&result_path), "inspected\n").unwrap();
        write_text(
            &run_dir.join("tasks/inspect/attempt-1/worker.log"),
            "worker log\n",
        )
        .unwrap();
        write_text(&run_dir.join("report.md"), "report\n").unwrap();
        let task = state.tasks.get_mut("inspect").unwrap();
        task.status = "completed".to_string();
        task.attempts = 1;
        task.result_path = Some(result_path);
        task.verifier = Some(VerifierResult {
            status: "pass".to_string(),
            summary: "evidence-backed".to_string(),
            confidence: 0.9,
            accepted_findings: vec![],
            rejected_findings: vec![],
            required_changes: vec![],
        });
        state.status = "completed".to_string();
        save_state(&run_dir, &mut state).unwrap();

        let report = build_report(&run_dir);

        assert_eq!(report.failures(), 0);
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "results" && check.level == CheckLevel::Ok)
        );
    }

    #[test]
    fn fails_completed_task_missing_result() {
        let temp = TempDir::new().unwrap();
        let (mut state, run_dir) = prepared_run(&temp);
        let task = state.tasks.get_mut("inspect").unwrap();
        task.status = "completed".to_string();
        task.attempts = 1;
        task.verifier = Some(VerifierResult {
            status: "pass".to_string(),
            summary: "ok".to_string(),
            confidence: 0.9,
            accepted_findings: vec![],
            rejected_findings: vec![],
            required_changes: vec![],
        });
        state.status = "completed".to_string();
        save_state(&run_dir, &mut state).unwrap();

        let report = build_report(&run_dir);

        assert!(report.failures() > 0);
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.detail.contains("missing result_path"))
        );
    }

    fn prepared_run(temp: &TempDir) -> (RunState, PathBuf) {
        let (mut state, run_dir) = create_empty_run(
            temp.path().to_path_buf(),
            "workflow: validate artifacts".to_string(),
            Some("audit".to_string()),
            RunOptions {
                concurrency: 1,
                max_retries: 1,
                model: None,
                agent: "custom".to_string(),
                agent_bin: "custom".to_string(),
                agent_command: Some("fake".to_string()),
                skip_git_repo_check: true,
                brake_file: None,
            },
        )
        .unwrap();
        attach_plan(&mut state, one_task_plan());
        save_state(&run_dir, &mut state).unwrap();
        (state, run_dir)
    }

    fn one_task_plan() -> WorkflowPlan {
        serde_json::from_value(json!({
            "version": 1,
            "name": "Validate artifacts",
            "objective": "Check a completed run",
            "riskLevel": "low",
            "maxConcurrency": 1,
            "tasks": [{
                "id": "inspect",
                "title": "Inspect",
                "kind": "explore",
                "prompt": "Inspect the repo.",
                "expectedOutput": "markdown",
                "writes": false,
                "verify": true
            }],
            "verification": {
                "strategy": "independent",
                "verifiersPerTask": 1,
                "maxRetries": 1,
                "prompt": "Check evidence."
            }
        }))
        .unwrap()
    }
}
