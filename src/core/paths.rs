use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::error::{PortalError, PortalResult};

pub const REGISTRY_FILE: &str = "installed.toml";

#[derive(Debug, Clone)]
pub struct MagicPaths {
    pub root: PathBuf,
    pub versions: PathBuf,
    pub packages: PathBuf,
    pub layouts: PathBuf,
    pub downloads: PathBuf,
    pub staging: PathBuf,
    pub registry: PathBuf,
}

impl MagicPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            versions: root.join("versions"),
            packages: root.join("packages"),
            layouts: root.join("layouts"),
            downloads: root.join("cache").join("downloads"),
            staging: root.join("cache").join("staging"),
            registry: root.join(REGISTRY_FILE),
            root,
        }
    }

    pub fn open() -> PortalResult<Self> {
        let home = dirs::home_dir().ok_or_else(|| {
            PortalError::NotFound("could not resolve the user home directory".into())
        })?;
        Ok(Self::new(home.join(".magic-bevy")))
    }

    pub fn version_dir(&self, version: &str) -> PortalResult<PathBuf> {
        validate_version(version)?;
        Ok(self.versions.join(version))
    }

    pub fn resolve_registry_path(&self, entry: &InstalledVersion) -> PortalResult<PathBuf> {
        validate_version(&entry.version)?;
        let path = Path::new(&entry.path);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        if !resolved.starts_with(&self.versions) {
            return Err(PortalError::InvalidInput(format!(
                "registry entry for {} points outside {}: {}",
                entry.version,
                self.versions.display(),
                resolved.display()
            )));
        }
        Ok(resolved)
    }

    pub fn ensure_layout(&self) -> PortalResult<()> {
        for directory in [
            &self.root,
            &self.versions,
            &self.packages,
            &self.layouts,
            &self.downloads,
            &self.staging,
        ] {
            fs::create_dir_all(directory).map_err(|error| PortalError::io(directory, error))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    pub version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub path: String,
    pub installed_at: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InstalledRegistry {
    #[serde(default)]
    pub versions: Vec<InstalledVersion>,
}

impl InstalledRegistry {
    pub fn load(path: &Path) -> PortalResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path).map_err(|error| PortalError::io(path, error))?;
        if content.trim().is_empty() {
            return Ok(Self::default());
        }
        toml::from_str(&content).map_err(|error| PortalError::InvalidManifest {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }

    pub fn save(&self, path: &Path) -> PortalResult<()> {
        let serialized =
            toml::to_string_pretty(self).map_err(|error| PortalError::InvalidManifest {
                path: path.to_path_buf(),
                message: format!("failed to serialize registry: {error}"),
            })?;
        atomic_write(path, serialized.as_bytes())
    }

    pub fn get(&self, version: &str) -> Option<&InstalledVersion> {
        self.versions.iter().find(|entry| entry.version == version)
    }

    pub fn upsert(&mut self, entry: InstalledVersion) {
        self.versions
            .retain(|existing| existing.version != entry.version);
        self.versions.push(entry);
    }

    pub fn remove(&mut self, version: &str) -> bool {
        let before = self.versions.len();
        self.versions.retain(|entry| entry.version != version);
        self.versions.len() != before
    }
}

pub fn validate_version(version: &str) -> PortalResult<()> {
    validate_component(version, "version")
}

pub fn validate_component(value: &str, label: &str) -> PortalResult<()> {
    if value.is_empty() {
        return Err(PortalError::InvalidInput(format!(
            "{label} must not be empty"
        )));
    }
    if value == "." || value == ".." {
        return Err(PortalError::InvalidInput(format!(
            "invalid {label}: {value}"
        )));
    }
    if value.contains(['/', '\\', '\0', ':', '<', '>', '"', '|', '?', '*']) {
        return Err(PortalError::InvalidInput(format!(
            "{label} contains characters that are not allowed in a path: {value}"
        )));
    }
    Ok(())
}

pub fn expand_path(input: &str) -> PortalResult<PathBuf> {
    let home = || {
        dirs::home_dir().ok_or_else(|| {
            PortalError::NotFound("could not resolve the user home directory".into())
        })
    };

    let expanded = if input == "~" {
        home()?
    } else if let Some(rest) = input.strip_prefix("~/") {
        home()?.join(rest)
    } else {
        PathBuf::from(input)
    };

    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        let current =
            std::env::current_dir().map_err(|error| PortalError::io(PathBuf::from("."), error))?;
        Ok(current.join(expanded))
    }
}

pub fn now_rfc3339() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let second_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second_of_day / 3_600,
        (second_of_day % 3_600) / 60,
        second_of_day % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn atomic_write(path: &Path, contents: &[u8]) -> PortalResult<()> {
    let parent = path.parent().ok_or_else(|| {
        PortalError::InvalidInput(format!("invalid registry path: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(REGISTRY_FILE);
    let temporary = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));

    fs::write(&temporary, contents).map_err(|error| PortalError::io(&temporary, error))?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|error| PortalError::io(path, error))?;
    }

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        PortalError::io(path, error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_roundtrip() {
        let root = crate::core::test_support::TempDir::new("registry");
        let path = root.path().join(REGISTRY_FILE);
        let mut registry = InstalledRegistry::default();

        registry.upsert(InstalledVersion {
            version: "0.1.0".into(),
            name: "nova".into(),
            description: Some("Early development build".into()),
            channel: Some("dev".into()),
            path: "versions/0.1.0".into(),
            installed_at: now_rfc3339(),
        });
        registry.save(&path).expect("save registry");

        let loaded = InstalledRegistry::load(&path).expect("load registry");
        assert_eq!(loaded.versions.len(), 1);
        assert_eq!(loaded.versions[0].version, "0.1.0");
        assert_eq!(loaded.versions[0].name, "nova");

        registry.remove("0.1.0");
        assert!(registry.versions.is_empty());
    }

    #[test]
    fn version_dir_rejects_traversal() {
        let paths = MagicPaths::new("/tmp/magic-portal-test");
        assert!(paths.version_dir("../escape").is_err());
        assert!(paths.version_dir("").is_err());
        assert!(paths.version_dir("0.1.0").is_ok());
    }

    #[test]
    fn registry_path_must_stay_inside_versions() {
        let paths = MagicPaths::new("/tmp/magic-portal-test");
        let entry = InstalledVersion {
            version: "0.1.0".into(),
            name: "nova".into(),
            description: None,
            channel: None,
            path: "../escape".into(),
            installed_at: now_rfc3339(),
        };
        assert!(paths.resolve_registry_path(&entry).is_err());
    }
}
