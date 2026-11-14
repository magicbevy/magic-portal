use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};

use super::error::{PortalError, PortalResult};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineReleasesFile {
    #[serde(default)]
    pub releases: Vec<EngineReleaseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineReleaseEntry {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub channel: String,
    pub portal: String,
}

impl EngineReleaseEntry {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.version)
    }

    pub fn display_description(&self) -> &str {
        self.description.as_deref().unwrap_or("-")
    }

    pub fn is_compatible(&self, portal_version: &str) -> bool {
        self.portal == portal_version
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManifest {
    pub version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub channel: String,
    pub portal: String,
}

pub fn sort_newest_first(releases: &mut [EngineReleaseEntry]) {
    releases.sort_by(|left, right| {
        match (
            Version::parse(&left.version),
            Version::parse(&right.version),
        ) {
            (Ok(left_version), Ok(right_version)) => right_version.cmp(&left_version),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => right.version.cmp(&left.version),
        }
    });
}

pub fn read_engine_manifest(version_dir: &Path) -> PortalResult<EngineManifest> {
    let candidates = [
        version_dir.join("magic.toml"),
        version_dir.join("magic-bevy").join("magic.toml"),
    ];
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|error| PortalError::io(&path, error))?;
        return toml::from_str(&content).map_err(|error| PortalError::InvalidManifest {
            path: path.to_path_buf(),
            message: error.to_string(),
        });
    }

    Err(PortalError::NotFound(format!(
        "installed engine is missing magic.toml (looked in {} and {})",
        version_dir.join("magic.toml").display(),
        version_dir.join("magic-bevy").join("magic.toml").display()
    )))
}

pub fn manifest_path(version_dir: &Path) -> PathBuf {
    let root = version_dir.join("magic.toml");
    if root.is_file() {
        root
    } else {
        version_dir.join("magic-bevy").join("magic.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str, portal: &str) -> EngineReleaseEntry {
        EngineReleaseEntry {
            version: version.into(),
            name: Some(format!("name-{version}")),
            description: Some(format!("description-{version}")),
            channel: "dev".into(),
            portal: portal.into(),
        }
    }

    #[test]
    fn parses_releases_file() {
        let toml = r#"
[[releases]]
version = "0.1.0"
name = "nova"
description = "Early development build"
channel = "dev"
portal = "0.1.0"
"#;
        let file: EngineReleasesFile = toml::from_str(toml).expect("parse releases");
        assert_eq!(file.releases.len(), 1);
        assert_eq!(file.releases[0].version, "0.1.0");
        assert_eq!(file.releases[0].name.as_deref(), Some("nova"));
        assert_eq!(file.releases[0].portal, "0.1.0");
    }

    #[test]
    fn filters_exact_portal_compatibility() {
        let file = EngineReleasesFile {
            releases: vec![release("0.1.0", "0.1.0"), release("0.2.0", "0.2.0")],
        };
        let compatible: Vec<_> = file
            .releases
            .iter()
            .filter(|entry| entry.is_compatible("0.1.0"))
            .collect();
        assert_eq!(compatible.len(), 1);
        assert_eq!(compatible[0].version, "0.1.0");
    }

    #[test]
    fn sorts_versions_newest_first() {
        let mut releases = vec![
            release("0.1.0", "0.1.0"),
            release("0.10.0", "0.1.0"),
            release("0.2.0", "0.1.0"),
            release("not-a-version", "0.1.0"),
        ];
        sort_newest_first(&mut releases);
        let versions: Vec<_> = releases
            .iter()
            .map(|entry| entry.version.as_str())
            .collect();
        assert_eq!(versions, ["0.10.0", "0.2.0", "0.1.0", "not-a-version"]);
    }
}
