use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};

use super::error::{PortalError, PortalResult};
use super::paths::atomic_write;

pub const PORTAL_MANIFEST_FILE: &str = "portal.toml";
pub const PORTAL_RELEASES_FILE: &str = "versions.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalManifest {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalReleaseEntry {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortalReleasesFile {
    #[serde(default)]
    pub releases: Vec<PortalReleaseEntry>,
}

#[derive(Debug, Clone)]
pub struct BumpResult {
    pub version: String,
    pub previous_latest: Option<String>,
    pub added: bool,
    pub path: PathBuf,
}

pub fn bump(root: &Path) -> PortalResult<BumpResult> {
    let manifest_path = root.join(PORTAL_MANIFEST_FILE);
    let releases_path = root.join(PORTAL_RELEASES_FILE);

    let manifest = read_manifest(&manifest_path)?;
    let mut releases = read_releases(&releases_path)?.releases;
    let current = parse_version(&manifest.version)?;
    let latest = latest_version(&releases)?;

    if let Some(latest) = &latest {
        if current < *latest {
            return Err(PortalError::InvalidInput(format!(
                "portal.toml version {} is older than the latest release {}; refusing to downgrade",
                manifest.version, latest
            )));
        }
        if current == *latest {
            return Ok(BumpResult {
                version: manifest.version,
                previous_latest: Some(latest.to_string()),
                added: false,
                path: releases_path,
            });
        }
    }

    if releases
        .iter()
        .any(|entry| entry.version == manifest.version)
    {
        return Err(PortalError::InvalidInput(format!(
            "portal.toml version {} is already present in {}",
            manifest.version,
            releases_path.display()
        )));
    }

    let previous_latest = latest.map(|version| version.to_string());
    releases.insert(
        0,
        PortalReleaseEntry {
            version: manifest.version.clone(),
            channel: manifest.channel.clone(),
        },
    );

    let serialized = toml::to_string_pretty(&PortalReleasesFile { releases }).map_err(|error| {
        PortalError::InvalidManifest {
            path: releases_path.clone(),
            message: format!("failed to serialize portal versions: {error}"),
        }
    })?;
    atomic_write(&releases_path, serialized.as_bytes())?;

    Ok(BumpResult {
        version: manifest.version,
        previous_latest,
        added: true,
        path: releases_path,
    })
}

fn read_manifest(path: &Path) -> PortalResult<PortalManifest> {
    if !path.is_file() {
        return Err(PortalError::NotFound(format!(
            "portal manifest not found: {}",
            path.display()
        )));
    }
    let content = fs::read_to_string(path).map_err(|error| PortalError::io(path, error))?;
    toml::from_str(&content).map_err(|error| PortalError::InvalidManifest {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn read_releases(path: &Path) -> PortalResult<PortalReleasesFile> {
    if !path.exists() {
        return Ok(PortalReleasesFile::default());
    }
    let content = fs::read_to_string(path).map_err(|error| PortalError::io(path, error))?;
    if content.trim().is_empty() {
        return Ok(PortalReleasesFile::default());
    }
    toml::from_str(&content).map_err(|error| PortalError::InvalidManifest {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn latest_version(releases: &[PortalReleaseEntry]) -> PortalResult<Option<Version>> {
    let mut latest: Option<Version> = None;
    for entry in releases {
        let version = parse_version(&entry.version)?;
        if latest.as_ref().is_none_or(|current| version > *current) {
            latest = Some(version);
        }
    }
    Ok(latest)
}

fn parse_version(value: &str) -> PortalResult<Version> {
    Version::parse(value).map_err(|error| {
        PortalError::InvalidInput(format!("invalid semantic version {value:?}: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_portal(root: &Path, version: &str) {
        fs::write(
            root.join(PORTAL_MANIFEST_FILE),
            format!("version = \"{version}\"\nchannel = \"dev\"\n"),
        )
        .expect("write portal.toml");
    }

    fn write_versions(root: &Path, versions: &[&str]) {
        let mut text = String::new();
        for version in versions {
            text.push_str(&format!(
                "[[releases]]\nversion = \"{version}\"\nchannel = \"dev\"\n\n"
            ));
        }
        fs::write(root.join(PORTAL_RELEASES_FILE), text).expect("write versions.toml");
    }

    #[test]
    fn adds_a_newer_version() {
        let root = crate::core::test_support::TempDir::new("bump-add");
        write_portal(root.path(), "0.2.0");
        write_versions(root.path(), &["0.1.0"]);

        let result = bump(root.path()).expect("bump");
        assert!(result.added);
        assert_eq!(result.version, "0.2.0");
        assert_eq!(result.previous_latest.as_deref(), Some("0.1.0"));

        let updated = read_releases(&root.path().join(PORTAL_RELEASES_FILE)).expect("read");
        assert_eq!(updated.releases[0].version, "0.2.0");
        assert_eq!(updated.releases[1].version, "0.1.0");
    }

    #[test]
    fn does_nothing_when_already_latest() {
        let root = crate::core::test_support::TempDir::new("bump-equal");
        write_portal(root.path(), "0.1.0");
        write_versions(root.path(), &["0.1.0"]);

        let result = bump(root.path()).expect("bump");
        assert!(!result.added);
        assert_eq!(result.version, "0.1.0");
    }

    #[test]
    fn rejects_downgrades() {
        let root = crate::core::test_support::TempDir::new("bump-down");
        write_portal(root.path(), "0.1.0");
        write_versions(root.path(), &["0.2.0"]);
        assert!(bump(root.path()).is_err());
    }

    #[test]
    fn creates_versions_file_when_missing() {
        let root = crate::core::test_support::TempDir::new("bump-create");
        write_portal(root.path(), "0.1.0");

        let result = bump(root.path()).expect("bump");
        assert!(result.added);
        assert!(root.path().join(PORTAL_RELEASES_FILE).is_file());
    }

    #[test]
    fn missing_portal_manifest_is_an_error() {
        let root = crate::core::test_support::TempDir::new("bump-missing");
        assert!(bump(root.path()).is_err());
    }
}
