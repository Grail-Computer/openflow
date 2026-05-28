use crate::state::RunState;
use crate::util::{duration_label, read_snippet, write_text};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn generate_report(state: &RunState, run_dir: &Path) -> Result<PathBuf> {
    let mut lines = Vec::new();
    lines.push(format!(
        "# Openflow Report: {}",
        state
            .plan
            .as_ref()
            .map(|plan| plan.name.as_str())
            .unwrap_or(&state.id)
    ));
    lines.push(String::new());
    lines.push(format!("Run: `{}`", state.id));
    lines.push(format!("Status: `{}`", state.status));
    lines.push(format!("Created: {}", state.created_at));
    lines.push(format!("Updated: {}", state.updated_at));
    lines.push(String::new());

    if let Some(plan) = &state.plan {
        lines.push("## Objective".to_string());
        lines.push(String::new());
        lines.push(plan.objective.clone());
        lines.push(String::new());

        lines.push("## Summary".to_string());
        lines.push(String::new());
        lines.push("| Task | Status | Attempts | Duration | Verifier |".to_string());
        lines.push("| --- | --- | ---: | --- | --- |".to_string());
        for task in &plan.tasks {
            let task_state = &state.tasks[&task.id];
            let verifier = task_state
                .verifier
                .as_ref()
                .map(|verifier| {
                    format!(
                        "{} ({}%)",
                        verifier.status,
                        (verifier.confidence * 100.0).round()
                    )
                })
                .unwrap_or_else(|| "n/a".to_string());
            lines.push(format!(
                "| `{}` | {} | {} | {} | {} |",
                task.id,
                task_state.status,
                task_state.attempts,
                duration_label(task_state.duration_ms),
                verifier
            ));
        }
        lines.push(String::new());

        let patches = state
            .tasks
            .values()
            .filter_map(|task| {
                task.patch_path
                    .as_ref()
                    .map(|path| (task.id.as_str(), path))
            })
            .collect::<Vec<_>>();
        if !patches.is_empty() {
            lines.push("## Patch Queue".to_string());
            lines.push(String::new());
            for (task_id, path) in patches {
                lines.push(format!("- `{task_id}`: `{}`", path.display()));
            }
            lines.push(String::new());
        }

        lines.push("## Task Details".to_string());
        lines.push(String::new());
        for task in &plan.tasks {
            let task_state = &state.tasks[&task.id];
            lines.push(format!("### {}: {}", task.id, task.title));
            lines.push(String::new());
            lines.push(format!("Kind: `{}`", task.kind));
            lines.push(format!("Role: `{}`", task.role));
            lines.push(format!("Writes: `{}`", task.writes));
            let overrides = task_overrides(task);
            if !overrides.is_empty() {
                lines.push(format!("Overrides: {}", overrides.join(", ")));
            }
            if !task.scope.is_empty() {
                lines.push(format!(
                    "Scope: {}",
                    task.scope
                        .iter()
                        .map(|item| format!("`{item}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(error) = &task_state.error {
                lines.push(String::new());
                lines.push("Error:".to_string());
                lines.push(String::new());
                lines.push("```text".to_string());
                lines.push(error.clone());
                lines.push("```".to_string());
            }
            if let Some(verifier) = &task_state.verifier {
                lines.push(String::new());
                lines.push("Verifier:".to_string());
                lines.push(String::new());
                lines.push(format!("- Status: `{}`", verifier.status));
                lines.push(format!("- Confidence: {}", verifier.confidence));
                lines.push(format!("- Summary: {}", verifier.summary));
            }
            if let Some(result_path) = &task_state.result_path {
                lines.push(String::new());
                lines.push("Result:".to_string());
                lines.push(String::new());
                lines.push("```markdown".to_string());
                lines.push(read_snippet(&run_dir.join(result_path), 20_000)?);
                lines.push("```".to_string());
            }
            lines.push(String::new());
        }
    }

    let report_path = run_dir.join("report.md");
    write_text(&report_path, &format!("{}\n", lines.join("\n").trim()))?;
    Ok(report_path)
}

fn task_overrides(task: &crate::plan::Task) -> Vec<String> {
    let mut overrides = Vec::new();
    if let Some(value) = &task.agent {
        overrides.push(format!("agent=`{value}`"));
    }
    if let Some(value) = &task.agent_bin {
        overrides.push(format!("agentBin=`{value}`"));
    }
    if task.agent_command.is_some() {
        overrides.push("agentCommand=`<set>`".to_string());
    }
    if let Some(value) = &task.model {
        overrides.push(format!("model=`{value}`"));
    }
    if let Some(value) = &task.sandbox {
        overrides.push(format!("sandbox=`{value}`"));
    }
    if let Some(value) = task.max_retries {
        overrides.push(format!("maxRetries=`{value}`"));
    }
    if let Some(value) = task.verifiers_per_task {
        overrides.push(format!("verifiersPerTask=`{value}`"));
    }
    if let Some(value) = &task.verifier_model {
        overrides.push(format!("verifierModel=`{value}`"));
    }
    if let Some(value) = &task.verifier_agent {
        overrides.push(format!("verifierAgent=`{value}`"));
    }
    overrides
}
