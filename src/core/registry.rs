use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;
use crate::core::discovery::OsType;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Rule {
    pub os: String,
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Definitions {
    pub groups: Vec<Group>,
}

pub fn load_definitions() -> Result<Definitions> {
    let mut paths = vec![
        Path::new("definitions.yaml").to_path_buf(),
        Path::new("/usr/share/tidytui/definitions.yaml").to_path_buf(),
    ];

    if let Some(config_dir) = dirs::config_dir() {
        paths.insert(1, config_dir.join("tidytui").join("definitions.yaml"));
    }

    // Try to find the first path that exists
    for path in paths {
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let definitions: Definitions = serde_yaml::from_str(&content)?;
            return Ok(definitions);
        }
    }

    Err(anyhow::anyhow!("Changes definitions.yaml not found in any of the search paths."))
}

pub fn filter_rules(definitions: &Definitions, os_type: &OsType) -> Vec<(String, String, String)> {
    let mut cleanable_paths = Vec::new();
    let os_id = match os_type {
        OsType::Arch => "arch",
        OsType::Ubuntu => "ubuntu",
        OsType::Debian => "debian",
        OsType::Fedora => "fedora",
        OsType::OpenSuse => "opensuse",
        OsType::Unknown(_) => "any", // Default fallback if needed, or handle specifically
    };

    for group in &definitions.groups {
        for rule in &group.rules {
            if rule.os == os_id || rule.os == "any" {
                cleanable_paths.push((group.name.clone(), group.name.clone(), rule.path.clone()));
            }
        }
    }
    cleanable_paths
}
