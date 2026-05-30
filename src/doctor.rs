use crate::skills::{SkillInstallState, default_skill_root, skill_statuses};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DoctorOptions {
    pub cwd: PathBuf,
    pub agent: String,
    pub agent_bin: String,
    pub agent_command: Option<String>,
    pub skill_root: Option<PathBuf>,
}

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
struct DoctorReport {
    checks: Vec<Check>,
}

pub fn run_doctor(options: DoctorOptions) -> Result<()> {
    let report = build_report(&options);
    print_report(&report);
    if report.failures() > 0 {
        bail!("doctor found {} failing check(s)", report.failures());
    }
    Ok(())
}

fn build_report(options: &DoctorOptions) -> DoctorReport {
    let mut report = DoctorReport::default();
    report.push_ok(
        "openflow",
        format!(
            "version {} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| "current executable unknown".to_string())
        ),
    );

    check_git(&mut report, &options.cwd);
    check_agent(&mut report, options);
    check_templates(&mut report, &options.cwd);
    check_skills(&mut report, options.skill_root.as_deref());

    report
}

fn check_git(report: &mut DoctorReport, cwd: &Path) {
    match command_output("git", ["--version"], cwd) {
        Ok(version) => report.push_ok("git", one_line(&version)),
        Err(error) => {
            report.push_fail("git", error);
            return;
        }
    }

    match command_output("git", ["rev-parse", "--show-toplevel"], cwd) {
        Ok(root) => report.push_ok("repository", one_line(&root)),
        Err(_) => report.push_warn(
            "repository",
            format!(
                "{} is not a git repository; read-only workflows can run, but write tasks need git",
                cwd.display()
            ),
        ),
    }
}

fn check_agent(report: &mut DoctorReport, options: &DoctorOptions) {
    if options.agent_command.is_some() {
        report.push_ok(
            "agent",
            format!(
                "{} uses --agent-command; Openflow will pass prompts through the custom template",
                options.agent
            ),
        );
        return;
    }

    if options.agent != "codex" {
        report.push_fail(
            "agent",
            format!(
                "agent {:?} requires --agent-command because only the codex preset is built in",
                options.agent
            ),
        );
        return;
    }

    match command_output(&options.agent_bin, ["--version"], &options.cwd) {
        Ok(version) => report.push_ok("codex", one_line(&version)),
        Err(error) => report.push_fail("codex", error),
    }
}

fn check_templates(report: &mut DoctorReport, cwd: &Path) {
    let workflow_dir = cwd.join(".openflow").join("workflows");
    if workflow_dir.exists() {
        report.push_ok(
            "templates",
            format!(
                "custom templates directory exists at {}",
                workflow_dir.display()
            ),
        );
    } else {
        report.push_ok(
            "templates",
            "built-in templates available; run `openflow init` to customize them".to_string(),
        );
    }
}

fn check_skills(report: &mut DoctorReport, skill_root: Option<&Path>) {
    let root = match skill_root
        .map(Path::to_path_buf)
        .or_else(|| default_skill_root().ok())
    {
        Some(root) => root,
        None => {
            report.push_warn(
                "skills",
                "could not resolve skill directory; pass `openflow doctor --skills-root <path>`"
                    .to_string(),
            );
            return;
        }
    };

    for status in skill_statuses(&root) {
        match status.state {
            SkillInstallState::Current => report.push_ok(
                format!("skill:{}", status.name),
                format!("installed at {}", status.path.display()),
            ),
            SkillInstallState::Missing => report.push_warn(
                format!("skill:{}", status.name),
                format!(
                    "not installed at {}; run `openflow install-skill --name {}`",
                    status.path.display(),
                    status.name
                ),
            ),
            SkillInstallState::Modified => report.push_warn(
                format!("skill:{}", status.name),
                format!(
                    "{} differs from the bundled skill; use `openflow install-skill --name {} --force` only if you want to overwrite local edits",
                    status.path.display(),
                    status.name
                ),
            ),
        }
    }
}

fn command_output<const N: usize>(
    command: &str,
    args: [&str; N],
    cwd: &Path,
) -> std::result::Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to run {command}: {error}"))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        Err(format!(
            "{command} exited with status {}: {message}",
            output.status.code().unwrap_or(-1)
        ))
    }
}

fn one_line(value: &str) -> String {
    value.lines().next().unwrap_or("").trim().to_string()
}

fn print_report(report: &DoctorReport) {
    println!("Openflow doctor");
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

impl DoctorReport {
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
    use crate::skills::install_named_skills;
    use tempfile::TempDir;

    #[test]
    fn doctor_reports_installed_and_missing_skills() {
        let temp = TempDir::new().unwrap();
        let skills_root = temp.path().join("skills");
        install_named_skills("dynamic", &skills_root, false).unwrap();

        let mut report = DoctorReport::default();
        check_skills(&mut report, Some(&skills_root));

        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "skill:dynamic" && check.level == CheckLevel::Ok)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "skill:openflow" && check.level == CheckLevel::Warn)
        );
        assert_eq!(report.failures(), 0);
    }
}
