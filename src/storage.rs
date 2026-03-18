use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::model::Project;

pub fn default_project_path(project_name: &str) -> PathBuf {
    PathBuf::from(format!("{project_name}.tck.json"))
}

pub fn load_project(path: &Path) -> Result<Project> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read project file {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse project file {}", path.display()))
}

pub fn save_project(path: &Path, project: &Project) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(project).context("failed to serialize project")?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}
