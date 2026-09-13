use crate::core::discovery::OsType;
use crate::core::CleanMode;
use anyhow::{Context, Result};
use bytesize::ByteSize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Rule {
    pub os: String,
    pub path: String,
    #[serde(default)]
    pub mode: CleanMode,
    /// Only remove entries last modified at least this many days ago.
    #[serde(default)]
    pub keep_days: Option<u64>,
    /// Hide the item unless it is at least this large (e.g. `10 MiB`).
    #[serde(default)]
    pub min_size: Option<ByteSize>,
}

fn default_category() -> String {
    "Other".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Definitions {
    pub groups: Vec<Group>,
}

/// A single scannable path resolved from the definitions for the current OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub group_id: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub path: String,
    pub mode: CleanMode,
    pub keep_days: Option<u64>,
    pub min_size: Option<ByteSize>,
}

pub fn parse_definitions(yaml: &str) -> Result<Definitions> {
    Ok(serde_yaml::from_str(yaml)?)
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
            let content =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            return parse_definitions(&content)
                .with_context(|| format!("parsing {}", path.display()));
        }
    }

    Err(anyhow::anyhow!(
        "definitions.yaml not found. Searched: ./definitions.yaml, \
         ~/.config/tidytui/definitions.yaml, /usr/share/tidytui/definitions.yaml"
    ))
}

pub fn os_id(os_type: &OsType) -> &'static str {
    match os_type {
        OsType::Arch => "arch",
        OsType::Ubuntu => "ubuntu",
        OsType::Debian => "debian",
        OsType::Fedora => "fedora",
        OsType::OpenSuse => "opensuse",
        OsType::Unknown(_) => "any",
    }
}

pub fn filter_rules(definitions: &Definitions, os_type: &OsType) -> Vec<Target> {
    let os_id = os_id(os_type);
    definitions
        .groups
        .iter()
        .flat_map(|group| {
            group
                .rules
                .iter()
                .filter(move |rule| rule.os == os_id || rule.os == "any")
                .map(move |rule| Target {
                    group_id: group.id.clone(),
                    name: group.name.clone(),
                    category: group.category.clone(),
                    description: group.description.clone(),
                    path: rule.path.clone(),
                    mode: rule.mode,
                    keep_days: rule.keep_days,
                    min_size: rule.min_size,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_parse_keep_days_and_min_size() {
        let yaml = r#"
groups:
  - id: trash
    name: Trash
    rules:
      - os: any
        path: ~/.local/share/Trash
        keep_days: 30
        min_size: 10 MiB
      - os: any
        path: ~/x
"#;
        let defs = parse_definitions(yaml).unwrap();
        let r = &defs.groups[0].rules;
        assert_eq!(r[0].keep_days, Some(30));
        assert_eq!(r[0].min_size, Some(bytesize::ByteSize::mib(10)));
        assert_eq!(r[1].keep_days, None);
        assert_eq!(r[1].min_size, None);

        let targets = filter_rules(&defs, &OsType::Arch);
        assert_eq!(targets[0].keep_days, Some(30));
        assert_eq!(targets[0].group_id, "trash");
    }
    use crate::core::discovery::OsType;

    const YAML: &str = r#"
groups:
  - id: pkg
    name: Package Cache
    category: System
    rules:
      - os: arch
        path: /var/cache/pacman/pkg/
        mode: contents
      - os: ubuntu
        path: /var/cache/apt/archives/
  - id: npm
    name: NPM Cache
    description: Node cache
    rules:
      - os: any
        path: ~/.npm
        mode: dir
"#;

    #[test]
    fn parses_optional_fields_with_defaults() {
        let defs = parse_definitions(YAML).unwrap();
        assert_eq!(defs.groups[0].category, "System");
        assert_eq!(defs.groups[1].category, "Other");
        assert_eq!(defs.groups[0].rules[1].mode, CleanMode::Contents);
        assert_eq!(defs.groups[1].rules[0].mode, CleanMode::Dir);
    }

    #[test]
    fn filters_by_os_and_any() {
        let defs = parse_definitions(YAML).unwrap();
        let targets = filter_rules(&defs, &OsType::Arch);
        let paths: Vec<&str> = targets.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, vec!["/var/cache/pacman/pkg/", "~/.npm"]);
        assert_eq!(targets[0].category, "System");
        assert_eq!(targets[1].description.as_deref(), Some("Node cache"));
        assert_eq!(targets[1].mode, CleanMode::Dir);
    }

    #[test]
    fn bundled_definitions_are_categorised_and_have_no_test_group() {
        let defs = parse_definitions(include_str!("../../definitions.yaml")).unwrap();
        let uncategorised: Vec<&str> = defs
            .groups
            .iter()
            .filter(|g| g.category == "Other")
            .map(|g| g.id.as_str())
            .collect();
        assert!(
            uncategorised.is_empty(),
            "missing category: {uncategorised:?}"
        );
        assert!(defs.groups.iter().all(|g| g.id != "test_group"));
    }

    #[test]
    fn unknown_os_gets_only_any_rules() {
        let defs = parse_definitions(YAML).unwrap();
        let targets = filter_rules(&defs, &OsType::Unknown("nix".into()));
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].group_id, "npm");
    }
}
