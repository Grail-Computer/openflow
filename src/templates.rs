use crate::state::flow_dir;
use crate::util::{ensure_dir, write_text};
use anyhow::{Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

const AUDIT: &str = include_str!("../templates/audit.md");
const MIGRATION: &str = include_str!("../templates/migration.md");
const PR_REVIEW: &str = include_str!("../templates/pr-review.md");

#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub content: String,
}

pub fn template_names() -> Vec<&'static str> {
    vec!["audit", "migration", "pr-review"]
}

pub fn load_template(cwd: &Path, name: Option<&str>) -> Result<Option<Template>> {
    let Some(name) = name else {
        return Ok(None);
    };
    let project_path = flow_dir(cwd).join("workflows").join(format!("{name}.md"));
    if project_path.exists() {
        return Ok(Some(Template {
            name: name.to_string(),
            content: fs::read_to_string(&project_path)?,
        }));
    }
    let content = match name {
        "audit" => AUDIT,
        "migration" => MIGRATION,
        "pr-review" => PR_REVIEW,
        _ => bail!("unknown workflow template: {name}"),
    };
    Ok(Some(Template {
        name: name.to_string(),
        content: content.to_string(),
    }))
}

pub fn install_project_templates(cwd: &Path) -> Result<Vec<PathBuf>> {
    let workflow_dir = flow_dir(cwd).join("workflows");
    ensure_dir(&workflow_dir)?;
    let mut installed = Vec::new();
    for (name, content) in [
        ("audit", AUDIT),
        ("migration", MIGRATION),
        ("pr-review", PR_REVIEW),
    ] {
        let target = workflow_dir.join(format!("{name}.md"));
        if !target.exists() {
            write_text(&target, content)?;
            installed.push(target);
        }
    }
    let gitignore = flow_dir(cwd).join(".gitignore");
    if !gitignore.exists() {
        write_text(&gitignore, "runs/\ntmp/\n*.log\n")?;
    }
    Ok(installed)
}
