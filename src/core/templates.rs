use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::{PortalError, PortalResult};
use super::paths::validate_component;
use super::progress::{ProgressEvent, Reporter};

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub directory: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<PathBuf>,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct TemplateManifest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

pub fn discover_templates(
    version_dir: &Path,
    reporter: &mut dyn Reporter,
) -> PortalResult<Vec<TemplateInfo>> {
    let templates_dir = version_dir.join("portal").join("templates");
    if !templates_dir.is_dir() {
        return Err(PortalError::NotFound(format!(
            "templates directory not found at {}",
            templates_dir.display()
        )));
    }

    let mut templates = Vec::new();
    let entries =
        fs::read_dir(&templates_dir).map_err(|error| PortalError::io(&templates_dir, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| PortalError::io(&templates_dir, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| PortalError::io(entry.path(), error))?;
        if !file_type.is_dir() {
            continue;
        }

        let manifest_path = entry.path().join("template.toml");
        if !manifest_path.is_file() {
            continue;
        }

        let content = match fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(error) => {
                reporter.report(ProgressEvent::log(format!(
                    "skipping template {}: {error}",
                    entry.path().display()
                )));
                continue;
            }
        };

        let manifest: TemplateManifest = match toml::from_str(&content) {
            Ok(manifest) => manifest,
            Err(error) => {
                reporter.report(ProgressEvent::log(format!(
                    "skipping invalid template {}: {error}",
                    entry.path().display()
                )));
                continue;
            }
        };

        let directory = entry.file_name().to_string_lossy().into_owned();
        let icon_path = entry.path().join("assets").join("icon.png");
        let icon = icon_path.is_file().then_some(icon_path);

        templates.push(TemplateInfo {
            directory,
            name: manifest.name,
            description: manifest.description.unwrap_or_default(),
            category: manifest.category.unwrap_or_else(|| "none".to_string()),
            icon,
            path: entry.path(),
        });
    }

    templates.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.directory.cmp(&right.directory))
    });

    Ok(templates)
}

pub fn template_dir(version_dir: &Path, template: &str) -> PortalResult<PathBuf> {
    validate_component(template, "template name")?;
    let path = version_dir.join("portal").join("templates").join(template);
    if !path.join("template.toml").is_file() {
        return Err(PortalError::NotFound(format!(
            "template {template} was not found at {}",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_templates_and_reads_metadata() {
        let root = crate::core::test_support::TempDir::new("templates");
        let version_dir = root.path().join("versions").join("0.1.0");
        let template = version_dir.join("portal").join("templates").join("empty");
        fs::create_dir_all(template.join("assets")).expect("create template");
        fs::write(
            template.join("template.toml"),
            "name = \"Empty\"\ndescription = \"A clean starting point.\"\ncategory = \"none\"\n",
        )
        .expect("write manifest");
        fs::write(template.join("assets").join("icon.png"), b"icon").expect("write icon");

        let mut reporter = |_event: ProgressEvent| {};
        let templates = discover_templates(&version_dir, &mut reporter).expect("discover");
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].directory, "empty");
        assert_eq!(templates[0].name, "Empty");
        assert_eq!(templates[0].category, "none");
        assert!(templates[0].icon.is_some());
    }

    #[test]
    fn missing_template_is_an_error() {
        let root = crate::core::test_support::TempDir::new("template-missing");
        assert!(template_dir(root.path(), "empty").is_err());
    }
}
