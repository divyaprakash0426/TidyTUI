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

pub fn load_definitions<P: AsRef<Path>>(path: P) -> Result<Definitions> {
    let content = fs::read_to_string(path)?;
    let definitions: Definitions = serde_yaml::from_str(&content)?;
    Ok(definitions)
}

pub fn filter_rules(definitions: &Definitions, os_type: &OsType) -> Vec<(String, String, String)> {
    let mut cleanable_paths = Vec::new();
    let os_id = match os_type {
        OsType::Arch => "arch",
        OsType::Ubuntu => "ubuntu",
        OsType::Debian => "debian",
        OsType::Unknown(_) => "any", // Default fallback if needed, or handle specifically
    };

    for group in &definitions.groups {
        for rule in &group.rules {
            if rule.os == os_id || rule.os == "any" {
                cleanable_paths.push((group.id.clone(), group.name.clone(), rule.path.clone()));
            }
        }
    }
    cleanable_paths
}
