use crate::util::{ensure_dir, write_text};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub fn run_command(command: &str, args: &[&str], cwd: &Path) -> Result<CommandOutput> {
    let output = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to run {command}"))?;
    let result = CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };
    if !output.status.success() {
        bail!(
            "{} {} failed:\n{}",
            command,
            args.join(" "),
            if result.stderr.trim().is_empty() {
                result.stdout.as_str()
            } else {
                result.stderr.as_str()
            }
        );
    }
    Ok(result)
}

pub fn is_git_repo(cwd: &Path) -> bool {
    run_command("git", &["rev-parse", "--show-toplevel"], cwd).is_ok()
}

pub fn prepare_task_workspace(
    cwd: &Path,
    run_dir: &Path,
    task_id: &str,
    writes: bool,
) -> Result<PathBuf> {
    if !writes {
        return Ok(cwd.to_path_buf());
    }
    if !is_git_repo(cwd) {
        bail!(
            "task {task_id} writes files, but {} is not a git repository",
            cwd.display()
        );
    }
    let worktree_root = run_dir.join("worktrees");
    ensure_dir(&worktree_root)?;
    let task_worktree = worktree_root.join(task_id);
    if task_worktree.exists() {
        return Ok(task_worktree);
    }
    let task_worktree_s = task_worktree.display().to_string();
    run_command(
        "git",
        &["worktree", "add", "--detach", &task_worktree_s, "HEAD"],
        cwd,
    )?;
    Ok(task_worktree)
}

pub fn capture_patch(worktree: &Path, patch_path: &Path) -> Result<bool> {
    let _ = run_command("git", &["add", "-N", "."], worktree);
    let diff = run_command("git", &["diff", "--binary"], worktree)?;
    write_text(patch_path, &diff.stdout)?;
    Ok(!diff.stdout.trim().is_empty())
}

pub fn apply_patch_file(cwd: &Path, patch_path: &Path, check_only: bool) -> Result<()> {
    let patch_s = patch_path.display().to_string();
    if check_only {
        run_command("git", &["apply", "--check", &patch_s], cwd)?;
    } else {
        run_command("git", &["apply", &patch_s], cwd)?;
    }
    Ok(())
}
