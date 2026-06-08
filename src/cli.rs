use crate::agent::{AgentExec, run_agent_exec};
use crate::doctor::{DoctorOptions, run_doctor};
use crate::plan::{WorkflowPlan, normalize_plan, summarize_plan};
use crate::report::generate_report;
use crate::runner::{RunnerOptions, run_workflow};
use crate::schema::workflow_plan_schema;
use crate::skills::{default_skill_root, install_named_skills};
use crate::state::{
    RunObservation, RunOptions, attach_plan, create_empty_run, format_observations, load_state,
    replace_observations, resolve_run_dir, save_state,
};
use crate::templates::{install_project_templates, load_template, template_names};
use crate::util::{parse_json_object, read_snippet, relative_to, write_json, write_text};
use crate::validate::run_validation;
use crate::worktree::apply_patch_file;
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "openflow")]
#[command(about = "Open-source dynamic workflow orchestration for CLI agent harnesses.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init,
    Templates,
    Plan(PlanArgs),
    Approve(RunIdArg),
    Run(RunArgs),
    Resume(ResumeArgs),
    Status(RunIdArg),
    Report(ReportArgs),
    Apply(ApplyArgs),
    InstallSkill(InstallSkillArgs),
    Doctor(DoctorArgs),
    Validate(RunIdArg),
}

#[derive(Args)]
struct PlanArgs {
    #[arg(num_args = 0..)]
    prompt: Vec<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct RunArgs {
    #[arg(num_args = 0..)]
    prompt: Vec<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ResumeArgs {
    run_id: Option<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ReportArgs {
    run_id: Option<String>,
    #[arg(long)]
    print: bool,
}

#[derive(Args)]
struct ApplyArgs {
    run_id: Option<String>,
    #[arg(long)]
    yes: bool,
}

#[derive(Args)]
struct RunIdArg {
    run_id: Option<String>,
}

#[derive(Args)]
struct InstallSkillArgs {
    #[arg(
        long,
        default_value = "dynamic",
        help = "Skill to install: dynamic, openflow, or all"
    )]
    name: String,
    #[arg(
        long,
        help = "Skills root. Defaults to $CODEX_HOME/skills or ~/.codex/skills"
    )]
    dest: Option<PathBuf>,
    #[arg(long, help = "Overwrite existing skill files")]
    force: bool,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long, help = "Agent preset/name to check, default: codex")]
    agent: Option<String>,
    #[arg(long, help = "Agent executable to check, default: codex")]
    agent_bin: Option<String>,
    #[arg(long, help = "Custom agent command template to check")]
    agent_command: Option<String>,
    #[arg(
        long,
        help = "Skills root. Defaults to $CODEX_HOME/skills or ~/.codex/skills"
    )]
    skills_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct CommonArgs {
    #[arg(long, help = "Workflow template: audit, migration, or pr-review")]
    template: Option<String>,
    #[arg(long, help = "Maximum concurrent agent workers")]
    concurrency: Option<usize>,
    #[arg(long, help = "Verifier-driven retries per task")]
    max_retries: Option<usize>,
    #[arg(long, help = "Model name passed through to the selected harness")]
    model: Option<String>,
    #[arg(long, help = "Agent preset/name, default: codex")]
    agent: Option<String>,
    #[arg(long, help = "Executable for built-in presets, default: codex")]
    agent_bin: Option<String>,
    #[arg(long, help = "Custom shell command template for any harness")]
    agent_command: Option<String>,
    #[arg(long, hide = true)]
    codex_bin: Option<String>,
    #[arg(long)]
    skip_git_repo_check: bool,
    #[arg(
        long = "status-file",
        value_name = "PATH",
        help = "Attach a controller-maintained status/observation file to planner, worker, verifier, report, and validation artifacts. Repeatable."
    )]
    status_files: Vec<PathBuf>,
    #[arg(
        long = "brake-file",
        value_name = "PATH",
        help = "Stop before starting the next task batch when this file exists and contains non-whitespace text."
    )]
    brake_file: Option<PathBuf>,
    #[arg(long)]
    yes: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    match cli.command {
        Command::Init => init(&cwd),
        Command::Templates => {
            for name in template_names() {
                println!("{name}");
            }
            Ok(())
        }
        Command::Plan(args) => plan_command(&cwd, args),
        Command::Approve(args) => approve_command(&cwd, args.run_id.as_deref()),
        Command::Run(args) => run_command(&cwd, args),
        Command::Resume(args) => resume_command(&cwd, args),
        Command::Status(args) => status_command(&cwd, args.run_id.as_deref()),
        Command::Report(args) => report_command(&cwd, args),
        Command::Apply(args) => apply_command(&cwd, args),
        Command::InstallSkill(args) => install_skill_command(args),
        Command::Doctor(args) => doctor_command(&cwd, args),
        Command::Validate(args) => validate_command(&cwd, args.run_id.as_deref()),
    }
}

fn init(cwd: &Path) -> Result<()> {
    let installed = install_project_templates(cwd)?;
    println!("Initialized .openflow in {}", cwd.display());
    if installed.is_empty() {
        println!("Templates already exist.");
    } else {
        println!("Installed templates:");
        for path in installed {
            println!("- {}", path.strip_prefix(cwd).unwrap_or(&path).display());
        }
    }
    Ok(())
}

fn plan_command(cwd: &Path, args: PlanArgs) -> Result<()> {
    let prompt = read_prompt(&args.prompt)?;
    let template = load_template(cwd, args.common.template.as_deref())?;
    let options = to_run_options(&args.common);
    let (mut state, run_dir) = create_empty_run(
        cwd.to_path_buf(),
        prompt.clone(),
        template.as_ref().map(|template| template.name.clone()),
        options,
    )?;
    replace_observations(&mut state, load_status_files(cwd, &args.common)?);
    save_state(&run_dir, &mut state)?;
    let plan = create_plan(
        cwd,
        &run_dir,
        &prompt,
        template.as_ref(),
        &args.common,
        &state.observations,
    )?;
    attach_plan(&mut state, plan);
    save_state(&run_dir, &mut state)?;
    print_plan_summary(state.plan.as_ref().unwrap(), &state.id);
    println!(
        "\nNext: openflow approve {} && openflow resume {}",
        state.id, state.id
    );
    Ok(())
}

fn approve_command(cwd: &Path, run_id: Option<&str>) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, run_id)?;
    let mut state = load_state(&run_dir)?;
    if state.status != "planned" {
        println!("Run {} is {}; nothing to approve.", state.id, state.status);
        return Ok(());
    }
    state.status = "approved".to_string();
    save_state(&run_dir, &mut state)?;
    println!("Approved {}", state.id);
    Ok(())
}

fn run_command(cwd: &Path, args: RunArgs) -> Result<()> {
    let has_prompt = !args.prompt.is_empty() || !io::stdin().is_terminal();
    let mut state;
    let run_dir;
    if has_prompt {
        let prompt = read_prompt(&args.prompt)?;
        let template = load_template(cwd, args.common.template.as_deref().or(Some("audit")))?;
        let options = to_run_options(&args.common);
        let created = create_empty_run(
            cwd.to_path_buf(),
            prompt.clone(),
            template.as_ref().map(|template| template.name.clone()),
            options,
        )?;
        state = created.0;
        run_dir = created.1;
        replace_observations(&mut state, load_status_files(cwd, &args.common)?);
        save_state(&run_dir, &mut state)?;
        let plan = create_plan(
            cwd,
            &run_dir,
            &prompt,
            template.as_ref(),
            &args.common,
            &state.observations,
        )?;
        attach_plan(&mut state, plan);
        save_state(&run_dir, &mut state)?;
    } else {
        run_dir = resolve_run_dir(cwd, None)?;
        state = load_state(&run_dir)?;
        replace_observations(&mut state, load_status_files(cwd, &args.common)?);
        save_state(&run_dir, &mut state)?;
    }

    if state.status == "planned" {
        print_plan_summary(state.plan.as_ref().context("missing plan")?, &state.id);
        confirm_plan(&args.common, state.plan.as_ref().unwrap())?;
        state.status = "approved".to_string();
        save_state(&run_dir, &mut state)?;
    }
    execute_and_report(&mut state, &run_dir, &args.common)
}

fn resume_command(cwd: &Path, args: ResumeArgs) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, args.run_id.as_deref())?;
    let mut state = load_state(&run_dir)?;
    if state.status == "completed" {
        println!("Run {} is already completed.", state.id);
        return Ok(());
    }
    if state.status == "planned" {
        print_plan_summary(state.plan.as_ref().context("missing plan")?, &state.id);
        confirm_plan(&args.common, state.plan.as_ref().unwrap())?;
        state.status = "approved".to_string();
    }
    replace_observations(&mut state, load_status_files(cwd, &args.common)?);
    for task in state.tasks.values_mut() {
        if matches!(task.status.as_str(), "running" | "failed" | "blocked") {
            task.status = "pending".to_string();
            task.error = None;
        }
    }
    save_state(&run_dir, &mut state)?;
    execute_and_report(&mut state, &run_dir, &args.common)
}

fn execute_and_report(
    state: &mut crate::state::RunState,
    run_dir: &Path,
    args: &CommonArgs,
) -> Result<()> {
    let runner_options = to_runner_options(args, &state.options);
    run_workflow(state, run_dir, &runner_options)?;
    let report = generate_report(state, run_dir)?;
    println!("\nRun {}: {}", state.status, state.id);
    println!("Report: {}", report.display());
    if matches!(state.status.as_str(), "failed" | "blocked") {
        bail!("workflow {}. See {}", state.status, report.display());
    }
    Ok(())
}

fn status_command(cwd: &Path, run_id: Option<&str>) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, run_id)?;
    let state = load_state(&run_dir)?;
    println!("{}: {}", state.id, state.status);
    for task in state.tasks.values() {
        println!(
            "  {}: {} ({} attempts)",
            task.id, task.status, task.attempts
        );
    }
    Ok(())
}

fn report_command(cwd: &Path, args: ReportArgs) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, args.run_id.as_deref())?;
    let state = load_state(&run_dir)?;
    let report = generate_report(&state, &run_dir)?;
    if args.print {
        print!("{}", fs::read_to_string(report)?);
    } else {
        println!("{}", report.display());
    }
    Ok(())
}

fn apply_command(cwd: &Path, args: ApplyArgs) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, args.run_id.as_deref())?;
    let state = load_state(&run_dir)?;
    let patches = state
        .tasks
        .values()
        .filter_map(|task| {
            task.patch_path
                .as_ref()
                .map(|path| (task.id.as_str(), run_dir.join(path)))
        })
        .collect::<Vec<_>>();
    if patches.is_empty() {
        println!("No patches found for this run.");
        return Ok(());
    }
    println!("Patch files for {}:", state.id);
    for (task_id, path) in &patches {
        println!("- {task_id}: {}", path.display());
    }
    if !args.yes && !ask_yes("Apply these patches to the current workspace? [y/N] ")? {
        println!("Aborted.");
        return Ok(());
    }
    for (_, patch) in &patches {
        apply_patch_file(cwd, patch, true)?;
    }
    for (task_id, patch) in &patches {
        apply_patch_file(cwd, patch, false)?;
        println!("Applied {task_id}");
    }
    Ok(())
}

fn install_skill_command(args: InstallSkillArgs) -> Result<()> {
    let root = args.dest.unwrap_or(default_skill_root()?);
    let installed = install_named_skills(&args.name, &root, args.force)?;
    for skill in installed {
        println!("Installed {skill}: {}", root.join(skill).display());
    }
    println!(
        "\nNext: restart Codex, then invoke `Use $dynamic ...` or `/dynamic` if your client exposes skill shortcuts."
    );
    Ok(())
}

fn doctor_command(cwd: &Path, args: DoctorArgs) -> Result<()> {
    let agent = args
        .agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("codex")
        .to_string();
    let agent_bin = args
        .agent_bin
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            if agent == "codex" {
                "codex".to_string()
            } else {
                agent.clone()
            }
        });
    run_doctor(DoctorOptions {
        cwd: cwd.to_path_buf(),
        agent,
        agent_bin,
        agent_command: args.agent_command,
        skill_root: args.skills_root,
    })
}

fn validate_command(cwd: &Path, run_id: Option<&str>) -> Result<()> {
    let run_dir = resolve_run_dir(cwd, run_id)?;
    run_validation(&run_dir)
}

fn create_plan(
    cwd: &Path,
    run_dir: &Path,
    prompt: &str,
    template: Option<&crate::templates::Template>,
    args: &CommonArgs,
    observations: &[RunObservation],
) -> Result<WorkflowPlan> {
    let schema_dir = run_dir.join("schemas");
    write_json(
        &schema_dir.join("workflow-plan.schema.json"),
        &workflow_plan_schema(),
    )?;
    let run_options = to_run_options(args);
    let planner_prompt = build_planner_prompt(prompt, template, &run_options, observations);
    write_text(&run_dir.join("planner-prompt.md"), &planner_prompt)?;
    let output_path = run_dir.join("plan.raw.json");
    run_agent_exec(&AgentExec {
        agent: run_options.agent.clone(),
        agent_bin: run_options.agent_bin.clone(),
        agent_command: run_options.agent_command.clone(),
        cwd: cwd.to_path_buf(),
        prompt: planner_prompt,
        prompt_file: Some(run_dir.join("planner-prompt.md")),
        sandbox: "read-only".to_string(),
        model: run_options.model.clone(),
        output_file: Some(output_path.clone()),
        schema_file: Some(schema_dir.join("workflow-plan.schema.json")),
        log_file: Some(run_dir.join("planner.log")),
        skip_git_repo_check: run_options.skip_git_repo_check,
    })?;
    let raw = fs::read_to_string(&output_path)?;
    let mut plan = parse_json_object::<WorkflowPlan>(&raw, "workflow plan")?;
    apply_explicit_cli_defaults(&mut plan, args);
    let plan = normalize_plan(plan, run_options.concurrency)?;
    write_json(&run_dir.join("plan.json"), &plan)?;
    Ok(plan)
}

fn build_planner_prompt(
    prompt: &str,
    template: Option<&crate::templates::Template>,
    options: &RunOptions,
    observations: &[RunObservation],
) -> String {
    format!(
        "You are the planner for Openflow, an open-source dynamic workflow runner for CLI agent harnesses.\n\n\
Create a strict, executable workflow plan as JSON only. The plan will be validated and then executed by separate agent workers.\n\n\
Planning rules:\n\
- Break the work into independently useful tasks with explicit dependencies.\n\
- Default to closed-loop workflows: clear goal, bounded tasks, explicit eval gates, and concrete stop or handoff conditions.\n\
- Use open-loop exploration only when the user asks for broad discovery or the task genuinely needs unknown-path research; keep it bounded by budget, risk, and verification.\n\
- Prefer read-only exploration and verification tasks unless the user clearly asks for code changes.\n\
- Set writes=false for audit, research, review, and planning tasks.\n\
- Set writes=true only when a worker must edit files, and make write tasks reversible by producing focused patches rather than applying broad changes directly.\n\
- Keep the default UX simple: use null for optional override fields unless a task truly needs a different model, agent harness, sandbox, retry count, or verifier setup.\n\
- Use workflow defaults for settings that should apply to many tasks, and task overrides only for exceptions.\n\
- To use a different model for one step, set that task's model field.\n\
- Keep each task prompt scoped enough for a fresh worker with no conversation history.\n\
- Treat controller observations as the current status object. Use them to plan from observed reality, call out stale or missing observations, and make stop/verification/brake conditions explicit.\n\
- Include adversarial verifier guidance so another worker fails closed on weak, stale, risky, or unsupported results.\n\
- Use roles deliberately: the planner is the orchestrator, tasks are specialist workers, verifier tasks are independent gates, and reports should preserve only decision-relevant output.\n\
- Use stable kebab-case task ids.\n\n\
Runtime defaults:\n\
- agent: {}\n\
- agentBin: {}\n\
- agentCommand: {}\n\
- model: {}\n\
- maxConcurrency: {}\n\
- maxRetries: {}\n\n\
Template guidance:\n{}\n\n\
Controller observations/status object:\n{}\n\n\
User request:\n{}\n\n\
Return a JSON object matching the provided schema. Do not wrap it in markdown.\n",
        options.agent,
        options.agent_bin,
        if options.agent_command.is_some() {
            "configured in runner options; leave agentCommand null unless overriding"
        } else {
            "none"
        },
        options.model.as_deref().unwrap_or("none"),
        options.concurrency,
        options.max_retries,
        template
            .map(|template| template.content.as_str())
            .unwrap_or("none"),
        format_observations(observations, 16_000),
        prompt
    )
}

fn print_plan_summary(plan: &WorkflowPlan, run_id: &str) {
    let summary = summarize_plan(plan);
    println!("Run: {run_id}");
    println!("Plan: {}", plan.name);
    println!("Objective: {}", plan.objective);
    println!(
        "Tasks: {} ({} write tasks)",
        summary.task_count, summary.write_tasks
    );
    println!("Verifier runs: {}", summary.estimated_verifier_runs);
    println!("Concurrency: {}", plan.max_concurrency);
    println!("Risk: {}", plan.risk_level);
    println!();
    for task in &plan.tasks {
        let deps = if task.depends_on.is_empty() {
            String::new()
        } else {
            format!(" after {}", task.depends_on.join(", "))
        };
        println!(
            "- {}: {} [{}, {}]{}",
            task.id,
            task.title,
            task.kind,
            if task.writes { "writes" } else { "read-only" },
            deps
        );
    }
}

fn confirm_plan(args: &CommonArgs, plan: &WorkflowPlan) -> Result<()> {
    if args.yes {
        return Ok(());
    }
    let writes = plan.tasks.iter().filter(|task| task.writes).count();
    let prompt = if writes == 0 {
        "Run this workflow now? [y/N] ".to_string()
    } else {
        format!(
            "Run this workflow now? This plan includes {writes} write task(s) in isolated git worktrees. [y/N] "
        )
    };
    if ask_yes(&prompt)? {
        Ok(())
    } else {
        bail!("aborted before execution")
    }
}

fn ask_yes(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn read_prompt(parts: &[String]) -> Result<String> {
    let joined = parts.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    if !io::stdin().is_terminal() {
        let mut prompt = String::new();
        io::stdin().read_to_string(&mut prompt)?;
        let prompt = prompt.trim().to_string();
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }
    bail!("missing workflow prompt")
}

fn load_status_files(cwd: &Path, args: &CommonArgs) -> Result<Vec<RunObservation>> {
    args.status_files
        .iter()
        .map(|path| {
            let absolute = if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            };
            let content = read_snippet(&absolute, 32_000)
                .with_context(|| format!("failed to read status file {}", absolute.display()))?;
            Ok(RunObservation {
                source: "status-file".to_string(),
                path: relative_to(cwd, &absolute),
                captured_at: crate::util::now(),
                content,
            })
        })
        .collect()
}

fn to_run_options(args: &CommonArgs) -> RunOptions {
    let agent = effective_agent(args, None);
    let agent_bin = effective_agent_bin(args, None, &agent);
    RunOptions {
        concurrency: effective_concurrency(args, None),
        max_retries: effective_max_retries(args, None),
        model: args.model.clone(),
        agent,
        agent_bin,
        agent_command: args.agent_command.clone(),
        skip_git_repo_check: args.skip_git_repo_check,
        brake_file: args.brake_file.clone(),
    }
}

fn to_runner_options(args: &CommonArgs, stored: &RunOptions) -> RunnerOptions {
    let agent = effective_agent(args, Some(stored));
    let agent_bin = effective_agent_bin(args, Some(stored), &agent);
    RunnerOptions {
        concurrency: effective_concurrency(args, Some(stored)),
        max_retries: effective_max_retries(args, Some(stored)),
        model: args.model.clone().or_else(|| stored.model.clone()),
        agent,
        agent_bin,
        agent_command: args
            .agent_command
            .clone()
            .or_else(|| stored.agent_command.clone()),
        skip_git_repo_check: args.skip_git_repo_check || stored.skip_git_repo_check,
        brake_file: args
            .brake_file
            .clone()
            .or_else(|| stored.brake_file.clone()),
    }
}

fn apply_explicit_cli_defaults(plan: &mut WorkflowPlan, args: &CommonArgs) {
    if let Some(max_retries) = args.max_retries {
        plan.verification.max_retries = max_retries;
    }
    if let Some(model) = args.model.as_ref().filter(|value| !value.trim().is_empty())
        && plan.defaults.model.is_none()
    {
        plan.defaults.model = Some(model.trim().to_string());
    }
    if let Some(agent) = args.agent.as_ref().filter(|value| !value.trim().is_empty())
        && plan.defaults.agent.is_none()
    {
        plan.defaults.agent = Some(agent.trim().to_string());
    }
    if (args.agent_bin.is_some() || args.codex_bin.is_some()) && plan.defaults.agent_bin.is_none() {
        let agent = effective_agent(args, None);
        plan.defaults.agent_bin = Some(effective_agent_bin(args, None, &agent));
    }
}

fn effective_concurrency(args: &CommonArgs, stored: Option<&RunOptions>) -> usize {
    args.concurrency
        .or_else(|| stored.map(|options| options.concurrency))
        .unwrap_or(4)
        .clamp(1, 50)
}

fn effective_max_retries(args: &CommonArgs, stored: Option<&RunOptions>) -> usize {
    args.max_retries
        .or_else(|| stored.map(|options| options.max_retries))
        .unwrap_or(1)
        .clamp(0, 5)
}

fn effective_agent(args: &CommonArgs, stored: Option<&RunOptions>) -> String {
    args.agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| stored.map(|options| options.agent.clone()))
        .unwrap_or_else(|| "codex".to_string())
}

fn effective_agent_bin(args: &CommonArgs, stored: Option<&RunOptions>, agent: &str) -> String {
    if let Some(codex_bin) = args
        .codex_bin
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return codex_bin.trim().to_string();
    }
    if let Some(agent_bin) = args
        .agent_bin
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return agent_bin.trim().to_string();
    }
    if args.agent.is_none()
        && let Some(stored) = stored
        && stored.agent == agent
    {
        return stored.agent_bin.clone();
    }
    if agent == "codex" {
        "codex".to_string()
    } else {
        agent.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn planner_prompt_includes_controller_observations() {
        let options = RunOptions {
            concurrency: 1,
            max_retries: 0,
            model: None,
            agent: "codex".to_string(),
            agent_bin: "codex".to_string(),
            agent_command: None,
            skip_git_repo_check: true,
            brake_file: None,
        };
        let observations = vec![RunObservation {
            source: "status-file".to_string(),
            path: PathBuf::from(".codex-loop/status.md"),
            captured_at: "2026-06-09T00:00:00Z".to_string(),
            content: "Observed: narrow verifier fails on missing status propagation.".to_string(),
        }];

        let prompt = build_planner_prompt(
            "workflow: prove local Codex control-loop status propagation",
            None,
            &options,
            &observations,
        );

        assert!(prompt.contains("Controller observations/status object"));
        assert!(prompt.contains(".codex-loop/status.md"));
        assert!(prompt.contains("missing status propagation"));
    }

    #[test]
    fn fake_agent_e2e_completes() {
        let temp = TempDir::new().unwrap();
        let fake = temp.path().join("fake-agent");
        fs::write(
            &fake,
            r#"#!/bin/sh
set -eu
out=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-o" ]; then out="$arg"; break; fi
  prev="$arg"
done
mkdir -p "$(dirname "$out")"
case "$out" in
  *plan.raw.json)
    cat > "$out" <<'JSON'
{"version":1,"name":"Fake audit","objective":"Exercise Openflow plumbing","riskLevel":"low","maxConcurrency":2,"tasks":[{"id":"inspect-repo","title":"Inspect repo","kind":"explore","prompt":"Return a fake result.","expectedOutput":"markdown","writes":false}],"verification":{"strategy":"independent","verifiersPerTask":1,"maxRetries":1,"prompt":"Pass fake results."}}
JSON
    ;;
  *verifier-*.json)
    cat > "$out" <<'JSON'
{"status":"pass","summary":"Fake verifier accepted the result.","confidence":1,"acceptedFindings":["Fake finding"],"rejectedFindings":[],"requiredChanges":[]}
JSON
    ;;
  *)
    echo "Fake worker result with concrete evidence." > "$out"
    ;;
esac
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake, perms).unwrap();

        let args = CommonArgs {
            template: Some("audit".to_string()),
            concurrency: Some(2),
            max_retries: Some(0),
            model: None,
            agent: Some("codex".to_string()),
            agent_bin: Some(fake.display().to_string()),
            agent_command: None,
            codex_bin: None,
            skip_git_repo_check: true,
            status_files: Vec::new(),
            brake_file: None,
            yes: true,
        };
        let cwd = temp.path();
        let prompt = "workflow: fake audit".to_string();
        let template = load_template(cwd, Some("audit")).unwrap();
        let (mut state, run_dir) = create_empty_run(
            cwd.to_path_buf(),
            prompt.clone(),
            Some("audit".to_string()),
            to_run_options(&args),
        )
        .unwrap();
        let plan = create_plan(cwd, &run_dir, &prompt, template.as_ref(), &args, &[]).unwrap();
        assert_eq!(plan.verification.max_retries, 0);
        attach_plan(&mut state, plan);
        save_state(&run_dir, &mut state).unwrap();
        execute_and_report(&mut state, &run_dir, &args).unwrap();
        let report = fs::read_to_string(run_dir.join("report.md")).unwrap();
        assert!(report.contains("Fake worker result"));
        assert!(report.contains("Fake verifier accepted"));
    }

    #[test]
    fn custom_agent_command_e2e_completes() {
        let temp = TempDir::new().unwrap();
        let fake = temp.path().join("custom-agent");
        fs::write(
            &fake,
            r#"#!/bin/sh
set -eu
case "$OPENFLOW_OUTPUT_FILE" in
  *plan.raw.json)
    cat > "$OPENFLOW_OUTPUT_FILE" <<'JSON'
{"version":1,"name":"Custom agent audit","objective":"Exercise custom harness plumbing","riskLevel":"low","maxConcurrency":2,"defaults":{"model":"default-model"},"tasks":[{"id":"inspect-repo","title":"Inspect repo","kind":"explore","model":"worker-model","verifierModel":"verifier-model","prompt":"Return a custom result.","expectedOutput":"markdown","writes":false}],"verification":{"strategy":"independent","verifiersPerTask":1,"maxRetries":0,"prompt":"Pass custom results."}}
JSON
    ;;
  *verifier-*.json)
    printf '{"status":"pass","summary":"Custom verifier accepted the result with model %s.","confidence":1,"acceptedFindings":["Custom finding"],"rejectedFindings":[],"requiredChanges":[]}\n' "$OPENFLOW_MODEL" > "$OPENFLOW_OUTPUT_FILE"
    ;;
  *)
    echo "Custom worker result with model $OPENFLOW_MODEL and concrete evidence." > "$OPENFLOW_OUTPUT_FILE"
    ;;
esac
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake, perms).unwrap();

        let args = CommonArgs {
            template: Some("audit".to_string()),
            concurrency: Some(2),
            max_retries: Some(0),
            model: None,
            agent: Some("custom".to_string()),
            agent_bin: Some("custom".to_string()),
            agent_command: Some(format!("{}", fake.display())),
            codex_bin: None,
            skip_git_repo_check: true,
            status_files: Vec::new(),
            brake_file: None,
            yes: true,
        };
        let cwd = temp.path();
        let prompt = "workflow: fake custom audit".to_string();
        let template = load_template(cwd, Some("audit")).unwrap();
        let (mut state, run_dir) = create_empty_run(
            cwd.to_path_buf(),
            prompt.clone(),
            Some("audit".to_string()),
            to_run_options(&args),
        )
        .unwrap();
        let plan = create_plan(cwd, &run_dir, &prompt, template.as_ref(), &args, &[]).unwrap();
        attach_plan(&mut state, plan);
        save_state(&run_dir, &mut state).unwrap();
        let resume_args = CommonArgs {
            template: None,
            concurrency: None,
            max_retries: None,
            model: None,
            agent: None,
            agent_bin: None,
            agent_command: None,
            codex_bin: None,
            skip_git_repo_check: false,
            status_files: Vec::new(),
            brake_file: None,
            yes: true,
        };
        execute_and_report(&mut state, &run_dir, &resume_args).unwrap();
        let report = fs::read_to_string(run_dir.join("report.md")).unwrap();
        assert!(report.contains("Custom worker result with model worker-model"));
        assert!(report.contains("Custom verifier accepted the result with model verifier-model"));
    }

    #[test]
    fn resume_options_preserve_stored_custom_harness() {
        let plan_args = CommonArgs {
            template: Some("audit".to_string()),
            concurrency: Some(9),
            max_retries: Some(0),
            model: Some("planner-model".to_string()),
            agent: Some("kimi-k2".to_string()),
            agent_bin: Some("kimi-k2-cli".to_string()),
            agent_command: Some("kimi-k2-cli run --prompt-file {prompt_file}".to_string()),
            codex_bin: None,
            skip_git_repo_check: true,
            status_files: Vec::new(),
            brake_file: Some(PathBuf::from(".codex-loop/brake")),
            yes: true,
        };
        let stored = to_run_options(&plan_args);

        let resume_args = CommonArgs {
            template: None,
            concurrency: None,
            max_retries: None,
            model: None,
            agent: None,
            agent_bin: None,
            agent_command: None,
            codex_bin: None,
            skip_git_repo_check: false,
            status_files: Vec::new(),
            brake_file: None,
            yes: true,
        };
        let runner = to_runner_options(&resume_args, &stored);

        assert_eq!(runner.concurrency, 9);
        assert_eq!(runner.max_retries, 0);
        assert_eq!(runner.model.as_deref(), Some("planner-model"));
        assert_eq!(runner.agent, "kimi-k2");
        assert_eq!(runner.agent_bin, "kimi-k2-cli");
        assert_eq!(
            runner.agent_command.as_deref(),
            Some("kimi-k2-cli run --prompt-file {prompt_file}")
        );
        assert!(runner.skip_git_repo_check);
        assert_eq!(runner.brake_file, Some(PathBuf::from(".codex-loop/brake")));
    }
}
