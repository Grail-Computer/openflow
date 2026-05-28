use crate::util::{ensure_dir, now, write_text};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AgentRun {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct AgentExec {
    pub agent: String,
    pub agent_bin: String,
    pub agent_command: Option<String>,
    pub cwd: PathBuf,
    pub prompt: String,
    pub prompt_file: Option<PathBuf>,
    pub sandbox: String,
    pub model: Option<String>,
    pub output_file: Option<PathBuf>,
    pub schema_file: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub skip_git_repo_check: bool,
}

pub fn run_agent_exec(input: &AgentExec) -> Result<AgentRun> {
    if input.prompt.trim().is_empty() {
        bail!("agent prompt cannot be empty");
    }
    if let Some(prompt_file) = &input.prompt_file {
        write_text(prompt_file, &input.prompt)?;
    }
    if let Some(output_file) = &input.output_file
        && let Some(parent) = output_file.parent()
    {
        ensure_dir(parent)?;
    }

    let started = std::time::Instant::now();
    let (command_label, output) = if let Some(template) = &input.agent_command {
        let rendered = render_command_template(template, input);
        let output = shell_command(&rendered)
            .current_dir(&input.cwd)
            .env("OPENFLOW_AGENT", &input.agent)
            .env("OPENFLOW_SANDBOX", &input.sandbox)
            .env("OPENFLOW_PROMPT", &input.prompt)
            .env(
                "OPENFLOW_PROMPT_FILE",
                input
                    .prompt_file
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            )
            .env(
                "OPENFLOW_OUTPUT_FILE",
                input
                    .output_file
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            )
            .env(
                "OPENFLOW_SCHEMA_FILE",
                input
                    .schema_file
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            )
            .env("OPENFLOW_MODEL", input.model.clone().unwrap_or_default())
            .output()
            .with_context(|| "failed to run custom agent command")?;
        (rendered, output)
    } else if input.agent == "codex" {
        let args = codex_args(input);
        let output = Command::new(&input.agent_bin)
            .args(&args)
            .current_dir(&input.cwd)
            .output()
            .with_context(|| format!("failed to run {}", input.agent_bin))?;
        (
            format!(
                "{} {}",
                input.agent_bin,
                args.iter()
                    .map(|arg| shellish(arg))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            output,
        )
    } else {
        bail!(
            "agent '{}' requires --agent-command. The built-in preset is currently 'codex'.",
            input.agent
        );
    };

    let run = AgentRun {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: started.elapsed().as_millis(),
    };

    if output.status.success() {
        persist_stdout_output(input, &run)?;
    }

    if let Some(log_file) = &input.log_file {
        write_log(log_file, input, &command_label, &run)?;
    }

    if !output.status.success() {
        bail!(
            "agent command exited with status {}:\n{}",
            run.code,
            if run.stderr.trim().is_empty() {
                run.stdout.as_str()
            } else {
                run.stderr.as_str()
            }
        );
    }

    Ok(run)
}

fn codex_args(input: &AgentExec) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "--sandbox".to_string(),
        input.sandbox.clone(),
    ];
    if let Some(model) = &input.model {
        args.push("-m".to_string());
        args.push(model.clone());
    }
    if input.skip_git_repo_check {
        args.push("--skip-git-repo-check".to_string());
    }
    if let Some(schema_file) = &input.schema_file {
        args.push("--output-schema".to_string());
        args.push(schema_file.display().to_string());
    }
    if let Some(output_file) = &input.output_file {
        args.push("-o".to_string());
        args.push(output_file.display().to_string());
    }
    args.push(input.prompt.clone());
    args
}

fn persist_stdout_output(input: &AgentExec, run: &AgentRun) -> Result<()> {
    let Some(output_file) = &input.output_file else {
        return Ok(());
    };
    if output_file.exists() {
        return Ok(());
    }
    if run.stdout.trim().is_empty() {
        return Ok(());
    }
    write_text(output_file, &run.stdout)
}

fn render_command_template(template: &str, input: &AgentExec) -> String {
    let schema_file = input
        .schema_file
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let output_file = input
        .output_file
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let prompt_file = input
        .prompt_file
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let model = input.model.clone().unwrap_or_default();

    template
        .replace("{agent}", &shell_quote(&input.agent))
        .replace("{agent_bin}", &shell_quote(&input.agent_bin))
        .replace("{cwd}", &shell_quote(&input.cwd.display().to_string()))
        .replace("{prompt}", &shell_quote(&input.prompt))
        .replace("{prompt_file}", &shell_quote(&prompt_file))
        .replace("{output_file}", &shell_quote(&output_file))
        .replace("{schema_file}", &shell_quote(&schema_file))
        .replace("{sandbox}", &shell_quote(&input.sandbox))
        .replace("{model}", &shell_quote(&model))
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()));
    shell.arg("-lc").arg(command);
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

fn write_log(path: &Path, input: &AgentExec, command_label: &str, run: &AgentRun) -> Result<()> {
    let content = format!(
        "# openflow agent log\nstarted_at: {}\nagent: {}\ncwd: {}\ncommand: {}\nexit_code: {}\nduration_ms: {}\n\n## stdout\n{}\n\n## stderr\n{}\n",
        now(),
        input.agent,
        input.cwd.display(),
        command_label,
        run.code,
        run.duration_ms,
        run.stdout,
        run.stderr
    );
    write_text(path, &content)
}

fn shellish(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "_./:=@-".contains(ch))
    {
        value.to_string()
    } else {
        format!("{value:?}")
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
