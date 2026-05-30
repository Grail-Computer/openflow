use crate::util::write_text;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct SkillFile {
    path: &'static str,
    content: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillInstallState {
    Current,
    Missing,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStatus {
    pub name: &'static str,
    pub path: PathBuf,
    pub state: SkillInstallState,
}

const OPENFLOW_SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path: "SKILL.md",
        content: include_str!("../skills/openflow/SKILL.md"),
    },
    SkillFile {
        path: "agents/openai.yaml",
        content: include_str!("../skills/openflow/agents/openai.yaml"),
    },
];

const DYNAMIC_SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path: "SKILL.md",
        content: include_str!("../skills/dynamic/SKILL.md"),
    },
    SkillFile {
        path: "agents/openai.yaml",
        content: include_str!("../skills/dynamic/agents/openai.yaml"),
    },
];

pub fn install_named_skills(name: &str, root: &Path, force: bool) -> Result<Vec<&'static str>> {
    match name {
        "dynamic" => {
            install_skill_files(root, "dynamic", DYNAMIC_SKILL_FILES, force)?;
            Ok(vec!["dynamic"])
        }
        "openflow" => {
            install_skill_files(root, "openflow", OPENFLOW_SKILL_FILES, force)?;
            Ok(vec!["openflow"])
        }
        "all" => {
            install_skill_files(root, "dynamic", DYNAMIC_SKILL_FILES, force)?;
            install_skill_files(root, "openflow", OPENFLOW_SKILL_FILES, force)?;
            Ok(vec!["dynamic", "openflow"])
        }
        other => bail!("unknown skill {other:?}; expected dynamic, openflow, or all"),
    }
}

pub fn skill_statuses(root: &Path) -> Vec<SkillStatus> {
    [
        ("dynamic", DYNAMIC_SKILL_FILES),
        ("openflow", OPENFLOW_SKILL_FILES),
    ]
    .into_iter()
    .map(|(name, files)| SkillStatus {
        name,
        path: root.join(name),
        state: skill_install_state(root, name, files),
    })
    .collect()
}

pub fn default_skill_root() -> Result<PathBuf> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home).join("skills"));
    }
    let home = std::env::var_os("HOME").context("HOME is not set; pass --dest explicitly")?;
    Ok(PathBuf::from(home).join(".codex").join("skills"))
}

fn install_skill_files(
    root: &Path,
    skill_name: &str,
    files: &[SkillFile],
    force: bool,
) -> Result<()> {
    for file in files {
        let path = root.join(skill_name).join(file.path);
        if path.exists() {
            let existing = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if existing == file.content {
                continue;
            }
            if !force {
                bail!(
                    "{} already exists and differs; rerun with --force to overwrite",
                    path.display()
                );
            }
        }
        write_text(&path, file.content)?;
    }
    Ok(())
}

fn skill_install_state(root: &Path, skill_name: &str, files: &[SkillFile]) -> SkillInstallState {
    let mut saw_modified = false;
    for file in files {
        let path = root.join(skill_name).join(file.path);
        let Ok(existing) = fs::read_to_string(path) else {
            return SkillInstallState::Missing;
        };
        if existing != file.content {
            saw_modified = true;
        }
    }
    if saw_modified {
        SkillInstallState::Modified
    } else {
        SkillInstallState::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn install_skill_writes_dynamic_alias_and_preserves_local_edits() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("skills");

        let installed = install_named_skills("dynamic", &root, false).unwrap();
        assert_eq!(installed, vec!["dynamic"]);
        let skill = root.join("dynamic").join("SKILL.md");
        let metadata = root.join("dynamic").join("agents").join("openai.yaml");
        assert!(
            fs::read_to_string(&skill)
                .unwrap()
                .contains("name: dynamic")
        );
        assert!(
            fs::read_to_string(&metadata)
                .unwrap()
                .contains("Dynamic Workflow")
        );
        assert_eq!(skill_statuses(&root)[0].state, SkillInstallState::Current);

        fs::write(&skill, "local edit\n").unwrap();
        assert_eq!(skill_statuses(&root)[0].state, SkillInstallState::Modified);
        let error = install_named_skills("dynamic", &root, false).unwrap_err();
        assert!(error.to_string().contains("--force"));

        install_named_skills("dynamic", &root, true).unwrap();
        assert_eq!(skill_statuses(&root)[0].state, SkillInstallState::Current);
    }
}
