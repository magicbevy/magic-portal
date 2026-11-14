use std::path::PathBuf;

use serde::Serialize;

use crate::platform::Os;

use super::paths::{InstalledRegistry, MagicPaths};
use super::progress::ProgressEvent;
use super::templates;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorVersion {
    pub version: String,
    pub name: String,
    pub path: PathBuf,
    pub directory_exists: bool,
    pub spell_exists: bool,
    pub cargo_config_exists: bool,
    pub template_count: usize,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub root_exists: bool,
    pub registry_path: PathBuf,
    pub registry_valid: bool,
    pub issues: Vec<String>,
    pub versions: Vec<DoctorVersion>,
}

impl DoctorReport {
    pub fn is_ok(&self) -> bool {
        self.root_exists
            && self.registry_valid
            && self.versions.iter().all(|version| {
                version.directory_exists
                    && version.spell_exists
                    && version.cargo_config_exists
                    && version.template_count > 0
            })
    }
}

pub fn check(paths: &MagicPaths) -> DoctorReport {
    let mut issues = Vec::new();
    let root_exists = paths.root.is_dir();
    if !root_exists {
        issues.push(format!(
            "engine root does not exist: {}",
            paths.root.display()
        ));
    }

    let mut registry_valid = true;
    let mut versions = Vec::new();
    match InstalledRegistry::load(&paths.registry) {
        Ok(registry) => {
            if !paths.registry.is_file() {
                registry_valid = false;
                issues.push(format!(
                    "registry does not exist yet: {}",
                    paths.registry.display()
                ));
            }

            for entry in &registry.versions {
                let path = match paths.resolve_registry_path(entry) {
                    Ok(path) => path,
                    Err(error) => {
                        registry_valid = false;
                        issues.push(format!(
                            "invalid registry entry for {}: {error}",
                            entry.version
                        ));
                        continue;
                    }
                };

                let directory_exists = path.is_dir();
                if !directory_exists {
                    issues.push(format!(
                        "version {} is registered but its directory is missing: {}",
                        entry.version,
                        path.display()
                    ));
                }

                let spell_path = path.join(Os::current().spell_relative_path());
                let spell_exists = spell_path.is_file();
                if !spell_exists {
                    issues.push(format!(
                        "version {} is missing its prerequisite spell: {}",
                        entry.version,
                        spell_path.display()
                    ));
                }

                let cargo_config = path.join(Os::current().cargo_config_relative_path());
                let cargo_config_exists = cargo_config.is_file();
                if !cargo_config_exists {
                    issues.push(format!(
                        "version {} is missing its cargo configuration: {}",
                        entry.version,
                        cargo_config.display()
                    ));
                }

                let mut reporter = |_event: ProgressEvent| {};
                let template_count = match templates::discover_templates(&path, &mut reporter) {
                    Ok(templates) => templates.len(),
                    Err(error) => {
                        issues.push(format!(
                            "version {} has unreadable templates: {error}",
                            entry.version
                        ));
                        0
                    }
                };
                if template_count == 0 {
                    issues.push(format!(
                        "version {} has no templates at {}",
                        entry.version,
                        path.join("portal").join("templates").display()
                    ));
                }

                versions.push(DoctorVersion {
                    version: entry.version.clone(),
                    name: entry.name.clone(),
                    path,
                    directory_exists,
                    spell_exists,
                    cargo_config_exists,
                    template_count,
                });
            }
        }
        Err(error) => {
            registry_valid = false;
            issues.push(format!("registry is invalid: {error}"));
        }
    }

    DoctorReport {
        root: paths.root.clone(),
        root_exists,
        registry_path: paths.registry.clone(),
        registry_valid,
        issues,
        versions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reports_missing_root() {
        let root = crate::core::test_support::TempDir::new("doctor");
        let paths = MagicPaths::new(root.path().join("missing"));
        let report = check(&paths);
        assert!(!report.root_exists);
        assert!(!report.is_ok());
    }

    #[test]
    fn reports_healthy_install() {
        let root = crate::core::test_support::TempDir::new("doctor-ok");
        let paths = MagicPaths::new(root.path().join(".magic-bevy"));
        paths.ensure_layout().expect("layout");
        let version_dir = paths.versions.join("0.1.0");
        let template = version_dir.join("portal").join("templates").join("empty");
        fs::create_dir_all(template.join("assets")).expect("create template");
        let spell_path = version_dir.join(Os::current().spell_relative_path());
        fs::create_dir_all(spell_path.parent().expect("spell parent")).expect("create spells");
        let cargo_config = version_dir.join(Os::current().cargo_config_relative_path());
        fs::create_dir_all(cargo_config.parent().expect("cargo config parent"))
            .expect("create config");
        fs::write(template.join("template.toml"), "name = \"Empty\"\n")
            .expect("write template manifest");
        fs::write(template.join("assets").join("icon.png"), b"icon").expect("write icon");
        fs::write(spell_path, b"").expect("write spell");
        fs::write(cargo_config, b"").expect("write cargo config");

        let mut registry = InstalledRegistry::default();
        registry.upsert(super::super::paths::InstalledVersion {
            version: "0.1.0".into(),
            name: "nova".into(),
            description: None,
            channel: None,
            path: "versions/0.1.0".into(),
            installed_at: super::super::paths::now_rfc3339(),
        });
        registry.save(&paths.registry).expect("save registry");

        let report = check(&paths);
        assert!(report.is_ok(), "issues: {:?}", report.issues);
        assert_eq!(report.versions.len(), 1);
        assert_eq!(report.versions[0].template_count, 1);
    }
}
