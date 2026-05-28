use crate::util::{ensure_dir, now, write_text};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CodexRun {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct CodexExec {
    pub codex_bin: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub sandbox: String,
    pub model: Option<String>,
    pub output_file: Option<PathBuf>,
    pub schema_file: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub skip_git_repo_check: bool,
}

pub fn run_codex_exec(input: &CodexExec) -> Result<CodexRun> {
    if input.prompt.trim().is_empty() {
        bail!("codex prompt cannot be empty");
    }

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
        if let Some(parent) = output_file.parent() {
            ensure_dir(parent)?;
        }
        args.push("-o".to_string());
        args.push(output_file.display().to_string());
    }
    args.push(input.prompt.clone());

    let started = std::time::Instant::now();
    let output = Command::new(&input.codex_bin)
        .args(&args)
        .current_dir(&input.cwd)
        .output()
        .with_context(|| format!("failed to run {}", input.codex_bin))?;

    let run = CodexRun {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: started.elapsed().as_millis(),
    };

    if let Some(log_file) = &input.log_file {
        write_log(log_file, input, &args, &run)?;
    }

    if !output.status.success() {
        bail!(
            "codex exited with status {}:\n{}",
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

fn write_log(path: &Path, input: &CodexExec, args: &[String], run: &CodexRun) -> Result<()> {
    let content = format!(
        "# openflow codex log\nstarted_at: {}\ncwd: {}\ncommand: {} {}\nexit_code: {}\nduration_ms: {}\n\n## stdout\n{}\n\n## stderr\n{}\n",
        now(),
        input.cwd.display(),
        input.codex_bin,
        args.iter()
            .map(|arg| shellish(arg))
            .collect::<Vec<_>>()
            .join(" "),
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
