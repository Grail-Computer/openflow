use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPlan {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    pub objective: String,
    #[serde(default = "default_risk")]
    pub risk_level: String,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: usize,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub verification: Verification,
    #[serde(default)]
    pub final_report: FinalReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub expected_output: String,
    #[serde(default)]
    pub writes: bool,
    #[serde(default = "default_true")]
    pub verify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    #[serde(default = "default_verification_strategy")]
    pub strategy: String,
    #[serde(default = "default_verifier_count")]
    pub verifiers_per_task: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default = "default_verification_prompt")]
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalReport {
    #[serde(default = "default_report_format")]
    pub format: String,
    #[serde(default = "default_report_sections")]
    pub sections: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlanSummary {
    pub task_count: usize,
    pub write_tasks: usize,
    pub estimated_verifier_runs: usize,
}

pub fn normalize_plan(mut plan: WorkflowPlan, default_concurrency: usize) -> Result<WorkflowPlan> {
    if plan.tasks.is_empty() {
        bail!("workflow plan must contain at least one task");
    }
    if !matches!(plan.risk_level.as_str(), "low" | "medium" | "high") {
        plan.risk_level = "medium".to_string();
    }
    plan.max_concurrency = plan.max_concurrency.clamp(1, 50);
    if default_concurrency > 0 {
        plan.max_concurrency = plan.max_concurrency.min(default_concurrency.clamp(1, 50));
    }
    if !matches!(plan.verification.strategy.as_str(), "none" | "independent") {
        plan.verification.strategy = "independent".to_string();
    }
    if plan.verification.strategy == "none" {
        plan.verification.verifiers_per_task = 0;
    } else {
        plan.verification.verifiers_per_task = plan.verification.verifiers_per_task.clamp(1, 3);
    }
    plan.verification.max_retries = plan.verification.max_retries.clamp(0, 5);

    let mut ids = BTreeSet::new();
    for (index, task) in plan.tasks.iter_mut().enumerate() {
        task.id = crate::util::slugify(
            if task.id.trim().is_empty() {
                &task.title
            } else {
                &task.id
            },
            &format!("task-{}", index + 1),
        );
        if !ids.insert(task.id.clone()) {
            bail!("duplicate task id: {}", task.id);
        }
        if !matches!(
            task.kind.as_str(),
            "explore" | "implement" | "verify" | "fix" | "synthesize"
        ) {
            task.kind = "explore".to_string();
        }
        if task.agent.trim().is_empty() {
            task.agent = match task.kind.as_str() {
                "implement" | "fix" => "worker",
                "verify" => "reviewer",
                _ => "explorer",
            }
            .to_string();
        }
        if task.expected_output.trim().is_empty() {
            task.expected_output = if matches!(task.kind.as_str(), "implement" | "fix") {
                "patch"
            } else {
                "markdown"
            }
            .to_string();
        }
        if !matches!(
            task.expected_output.as_str(),
            "markdown" | "json" | "patch" | "diff" | "notes"
        ) {
            task.expected_output = "markdown".to_string();
        }
        if matches!(task.kind.as_str(), "implement" | "fix") {
            task.writes = true;
        }
        if task.kind == "verify" {
            task.verify = false;
        }
    }

    let known = ids;
    for task in &plan.tasks {
        for dependency in &task.depends_on {
            if !known.contains(dependency) {
                bail!("task {} depends on unknown task {}", task.id, dependency);
            }
        }
    }
    topological_batches(&plan.tasks)?;
    Ok(plan)
}

pub fn topological_batches(tasks: &[Task]) -> Result<Vec<Vec<Task>>> {
    let mut incoming: BTreeMap<String, BTreeSet<String>> = tasks
        .iter()
        .map(|task| (task.id.clone(), task.depends_on.iter().cloned().collect()))
        .collect();
    let mut completed = BTreeSet::new();
    let mut batches = Vec::new();

    while completed.len() < tasks.len() {
        let ready = tasks
            .iter()
            .filter(|task| {
                !completed.contains(&task.id)
                    && incoming
                        .get(&task.id)
                        .is_some_and(|deps| deps.iter().all(|dep| completed.contains(dep)))
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let blocked = tasks
                .iter()
                .filter(|task| !completed.contains(&task.id))
                .map(|task| task.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            bail!("workflow plan contains a dependency cycle: {blocked}");
        }
        for task in &ready {
            completed.insert(task.id.clone());
            for deps in incoming.values_mut() {
                deps.remove(&task.id);
            }
        }
        batches.push(ready);
    }
    Ok(batches)
}

pub fn summarize_plan(plan: &WorkflowPlan) -> PlanSummary {
    let write_tasks = plan.tasks.iter().filter(|task| task.writes).count();
    let estimated_verifier_runs = if plan.verification.strategy == "none" {
        0
    } else {
        plan.tasks.iter().filter(|task| task.verify).count() * plan.verification.verifiers_per_task
    };
    PlanSummary {
        task_count: plan.tasks.len(),
        write_tasks,
        estimated_verifier_runs,
    }
}

fn default_version() -> u32 {
    1
}
fn default_risk() -> String {
    "medium".to_string()
}
fn default_concurrency() -> usize {
    4
}
fn default_kind() -> String {
    "explore".to_string()
}
fn default_true() -> bool {
    true
}
fn default_verification_strategy() -> String {
    "independent".to_string()
}
fn default_verifier_count() -> usize {
    1
}
fn default_max_retries() -> usize {
    1
}
fn default_verification_prompt() -> String {
    "Verify that the task result is accurate, evidence-backed, scoped to the task, and free of unsupported claims.".to_string()
}
fn default_report_format() -> String {
    "markdown".to_string()
}
fn default_report_sections() -> Vec<String> {
    vec![
        "Summary".to_string(),
        "Verified findings".to_string(),
        "Rejected findings".to_string(),
        "Task details".to_string(),
        "Patch queue".to_string(),
    ]
}

impl Default for Verification {
    fn default() -> Self {
        Self {
            strategy: default_verification_strategy(),
            verifiers_per_task: default_verifier_count(),
            max_retries: default_max_retries(),
            prompt: default_verification_prompt(),
        }
    }
}

impl Default for FinalReport {
    fn default() -> Self {
        Self {
            format: default_report_format(),
            sections: default_report_sections(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_plan() -> WorkflowPlan {
        WorkflowPlan {
            version: 1,
            name: "Audit".to_string(),
            objective: "Find auth bugs".to_string(),
            risk_level: "medium".to_string(),
            max_concurrency: 4,
            tasks: vec![
                Task {
                    id: "map-auth".to_string(),
                    title: "Map auth".to_string(),
                    kind: "explore".to_string(),
                    agent: String::new(),
                    depends_on: vec![],
                    scope: vec![],
                    prompt: "Inspect auth entrypoints.".to_string(),
                    expected_output: String::new(),
                    writes: false,
                    verify: true,
                },
                Task {
                    id: "audit-auth".to_string(),
                    title: "Audit auth".to_string(),
                    kind: "explore".to_string(),
                    agent: String::new(),
                    depends_on: vec!["map-auth".to_string()],
                    scope: vec![],
                    prompt: "Find auth bugs.".to_string(),
                    expected_output: String::new(),
                    writes: false,
                    verify: true,
                },
            ],
            verification: Verification::default(),
            final_report: FinalReport::default(),
        }
    }

    #[test]
    fn normalizes_minimal_plan() {
        let plan = normalize_plan(minimal_plan(), 4).unwrap();
        assert!(!plan.tasks[0].writes);
        assert_eq!(plan.tasks[0].expected_output, "markdown");
        let batches = topological_batches(&plan.tasks).unwrap();
        assert_eq!(batches[0][0].id, "map-auth");
        assert_eq!(batches[1][0].id, "audit-auth");
    }

    #[test]
    fn rejects_unknown_dependencies() {
        let mut plan = minimal_plan();
        plan.tasks[0].depends_on = vec!["missing".to_string()];
        assert!(
            normalize_plan(plan, 4)
                .unwrap_err()
                .to_string()
                .contains("unknown task")
        );
    }

    #[test]
    fn rejects_cycles() {
        let mut plan = minimal_plan();
        plan.tasks[0].depends_on = vec!["audit-auth".to_string()];
        assert!(
            normalize_plan(plan, 4)
                .unwrap_err()
                .to_string()
                .contains("dependency cycle")
        );
    }

    #[test]
    fn summarizes_counts() {
        let mut plan = minimal_plan();
        plan.tasks[0].kind = "implement".to_string();
        plan.verification.verifiers_per_task = 2;
        let plan = normalize_plan(plan, 4).unwrap();
        let summary = summarize_plan(&plan);
        assert_eq!(summary.task_count, 2);
        assert_eq!(summary.write_tasks, 1);
        assert_eq!(summary.estimated_verifier_runs, 4);
    }
}
