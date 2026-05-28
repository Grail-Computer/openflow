use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

pub fn read_snippet(path: &Path, max_chars: usize) -> Result<String> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if content.chars().count() <= max_chars {
        return Ok(content);
    }
    let snippet: String = content.chars().take(max_chars).collect();
    Ok(format!(
        "{snippet}\n\n[truncated {} chars]",
        content.chars().count() - max_chars
    ))
}

pub fn slugify(value: &str, fallback: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= 48 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

pub fn make_run_id(prompt: &str) -> String {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut hasher = DefaultHasher::new();
    prompt.hash(&mut hasher);
    format!(
        "{stamp}-{}-{:08x}",
        slugify(prompt, "workflow"),
        hasher.finish() as u32
    )
}

pub fn parse_json_object<T: DeserializeOwned>(text: &str, label: &str) -> Result<T> {
    if let Ok(value) = serde_json::from_str(text) {
        return Ok(value);
    }
    let first = text
        .find('{')
        .with_context(|| format!("could not parse {label}: no JSON object found"))?;
    let last = text
        .rfind('}')
        .with_context(|| format!("could not parse {label}: no JSON object found"))?;
    if last <= first {
        bail!("could not parse {label}: invalid JSON object bounds");
    }
    serde_json::from_str(&text[first..=last]).with_context(|| format!("could not parse {label}"))
}

pub fn relative_to(from: &Path, target: &Path) -> PathBuf {
    pathdiff(from, target).unwrap_or_else(|| target.to_path_buf())
}

fn pathdiff(from: &Path, target: &Path) -> Option<PathBuf> {
    let from = from.components().collect::<Vec<_>>();
    let target = target.components().collect::<Vec<_>>();
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &target[common..] {
        result.push(component.as_os_str());
    }
    Some(if result.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        result
    })
}

pub fn duration_label(ms: Option<u128>) -> String {
    let Some(ms) = ms else {
        return "unknown".to_string();
    };
    let seconds = (ms / 1000) as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}
