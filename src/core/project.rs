use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::platform::Os;

use super::error::{PortalError, PortalResult};
use super::paths::{InstalledRegistry, MagicPaths, validate_component};
use super::progress::{CancelFlag, ProgressEvent, Reporter};
use super::templates;
use super::version::{self, EngineManifest};

#[derive(Debug, Clone)]
pub struct CreatePlan {
    pub engine_version: String,
    pub engine_name: String,
    pub template: String,
    pub template_name: String,
    pub engine_dir: PathBuf,
    pub template_dir: PathBuf,
    pub destination: PathBuf,
    pub cargo_config_source: PathBuf,
    pub manifest: EngineManifest,
}

pub fn build_create_plan(
    engine_version: &str,
    template: &str,
    destination: PathBuf,
    paths: &MagicPaths,
) -> PortalResult<CreatePlan> {
    validate_component(engine_version, "engine version")?;
    validate_component(template, "template name")?;

    let registry = InstalledRegistry::load(&paths.registry)?;
    if registry.get(engine_version).is_none() {
        return Err(PortalError::NotFound(format!(
            "engine version {engine_version} is not installed in {}",
            paths.registry.display()
        )));
    }

    let version_dir = paths.version_dir(engine_version)?;
    if !version_dir.is_dir() {
        return Err(PortalError::NotFound(format!(
            "engine version {engine_version} is registered but its directory is missing: {}",
            version_dir.display()
        )));
    }

    let template_dir = templates::template_dir(&version_dir, template)?;
    let template_info = {
        let mut reporter = |_event: ProgressEvent| {};
        let templates = templates::discover_templates(&version_dir, &mut reporter)?;
        templates
            .into_iter()
            .find(|entry| entry.path == template_dir)
            .ok_or_else(|| {
                PortalError::NotFound(format!(
                    "template {template} has invalid or missing metadata at {}",
                    template_dir.display()
                ))
            })?
    };
    let cargo_config_source = version_dir.join(Os::current().cargo_config_relative_path());
    if !cargo_config_source.is_file() {
        return Err(PortalError::NotFound(format!(
            "cargo configuration for {} is missing: {}",
            Os::current().name(),
            cargo_config_source.display()
        )));
    }

    let manifest = version::read_engine_manifest(&version_dir)?;
    if manifest.version != engine_version {
        return Err(PortalError::InvalidManifest {
            path: version::manifest_path(&version_dir),
            message: format!(
                "magic.toml reports version {} but {} was requested",
                manifest.version, engine_version
            ),
        });
    }

    validate_destination(&destination)?;

    let protected = [
        &paths.versions,
        &paths.packages,
        &paths.layouts,
        &paths.downloads,
        &paths.staging,
    ];
    if let Some(base) = protected
        .iter()
        .find(|base| destination.starts_with(base.as_path()))
    {
        return Err(PortalError::InvalidInput(format!(
            "destination {} is inside the protected Magic Bevy directory {}",
            destination.display(),
            base.display()
        )));
    }

    Ok(CreatePlan {
        engine_version: engine_version.to_string(),
        engine_name: manifest.name.clone(),
        template: template.to_string(),
        template_name: template_info.name,
        engine_dir: version_dir,
        template_dir,
        destination,
        cargo_config_source,
        manifest,
    })
}

pub fn create_project(
    plan: &CreatePlan,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<PathBuf> {
    let parent = plan.destination.parent().ok_or_else(|| {
        PortalError::InvalidInput(format!(
            "destination has no parent directory: {}",
            plan.destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;

    let staging = parent.join(staging_name());
    fs::create_dir_all(&staging).map_err(|error| PortalError::io(&staging, error))?;

    let result = create_project_inner(plan, &staging, reporter, cancel);
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;

    if plan.destination.exists() {
        if !plan.destination.is_dir() {
            return Err(PortalError::Install(format!(
                "destination {} exists and is not a directory",
                plan.destination.display()
            )));
        }
        let mut entries = fs::read_dir(&plan.destination)
            .map_err(|error| PortalError::io(&plan.destination, error))?;
        if entries.next().is_some() {
            return Err(PortalError::Install(format!(
                "destination {} is not empty",
                plan.destination.display()
            )));
        }
        fs::remove_dir(&plan.destination)
            .map_err(|error| PortalError::io(&plan.destination, error))?;
    }

    if let Err(error) = fs::rename(&staging, &plan.destination) {
        let _ = fs::remove_dir_all(&staging);
        if !plan.destination.exists() {
            let _ = fs::create_dir_all(&plan.destination);
        }
        return Err(PortalError::io(&plan.destination, error));
    }

    reporter.report(ProgressEvent::step(format!(
        "created project at {}",
        plan.destination.display()
    )));
    Ok(plan.destination.clone())
}

fn create_project_inner(
    plan: &CreatePlan,
    staging: &Path,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    reporter.report(ProgressEvent::step(format!(
        "copying engine {} into {}",
        plan.engine_version,
        staging.display()
    )));
    copy_engine_version(&plan.engine_dir, staging, cancel)?;

    reporter.report(ProgressEvent::step(format!(
        "copying template {} into {}",
        plan.template,
        staging.display()
    )));
    copy_template(&plan.template_dir, staging, cancel)?;

    let layouts = staging.join("layouts");
    fs::create_dir_all(&layouts).map_err(|error| PortalError::io(&layouts, error))?;

    let packages_dir = staging.join("packages");
    fs::create_dir_all(&packages_dir).map_err(|error| PortalError::io(&packages_dir, error))?;
    let packages_manifest = packages_dir.join("packages.toml");
    if !packages_manifest.is_file() {
        fs::write(&packages_manifest, "[dependencies]\n")
            .map_err(|error| PortalError::io(&packages_manifest, error))?;
    }

    cancel.check()?;
    let cargo_config = staging.join(".cargo").join("config.toml");
    if let Some(parent) = cargo_config.parent() {
        fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
    }
    fs::copy(&plan.cargo_config_source, &cargo_config)
        .map_err(|error| PortalError::io(&plan.cargo_config_source, error))?;

    cancel.check()?;
    let manifest_path = staging.join("magic.toml");
    let serialized =
        toml::to_string_pretty(&plan.manifest).map_err(|error| PortalError::InvalidManifest {
            path: manifest_path.clone(),
            message: format!("failed to serialize magic.toml: {error}"),
        })?;
    fs::write(&manifest_path, serialized)
        .map_err(|error| PortalError::io(&manifest_path, error))?;

    Ok(())
}

fn copy_engine_version(engine_dir: &Path, staging: &Path, cancel: &CancelFlag) -> PortalResult<()> {
    let binary_name = Os::current().executable_name(crate::core::run::ENGINE_EXECUTABLE);
    let entries = fs::read_dir(engine_dir).map_err(|error| PortalError::io(engine_dir, error))?;
    for entry in entries {
        cancel.check()?;
        let entry = entry.map_err(|error| PortalError::io(engine_dir, error))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.as_ref() == "portal" || name.as_ref() == ".github" || name.as_ref() == binary_name {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|error| PortalError::io(entry.path(), error))?;
        if file_type.is_symlink() {
            return Err(PortalError::InvalidInput(format!(
                "engine directory contains a symbolic link: {}",
                entry.path().display()
            )));
        }

        let target = staging.join(name.as_ref());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target, &no_skip, cancel)?;
        } else if file_type.is_file() {
            copy_file(&entry.path(), &target)?;
        }
    }
    Ok(())
}

fn copy_template(template_dir: &Path, staging: &Path, cancel: &CancelFlag) -> PortalResult<()> {
    let entries =
        fs::read_dir(template_dir).map_err(|error| PortalError::io(template_dir, error))?;
    for entry in entries {
        cancel.check()?;
        let entry = entry.map_err(|error| PortalError::io(template_dir, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| PortalError::io(entry.path(), error))?;
        if file_type.is_symlink() {
            return Err(PortalError::InvalidInput(format!(
                "template contains a symbolic link: {}",
                entry.path().display()
            )));
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        match name.as_ref() {
            "template.toml" => continue,
            "packages.toml" => {
                let destination = staging.join("packages").join("packages.toml");
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
                }
                copy_file(&entry.path(), &destination)?;
            }
            "assets" if file_type.is_dir() => {
                let destination = staging.join("assets");
                copy_dir_all(&entry.path(), &destination, &icon_only, cancel)?;
                if destination.is_dir() {
                    let empty = fs::read_dir(&destination)
                        .map_err(|error| PortalError::io(&destination, error))?
                        .next()
                        .is_none();
                    if empty {
                        fs::remove_dir_all(&destination)
                            .map_err(|error| PortalError::io(&destination, error))?;
                    }
                }
            }
            _ if file_type.is_dir() => {
                copy_dir_all(&entry.path(), &staging.join(&*name), &no_skip, cancel)?;
            }
            _ if file_type.is_file() => {
                copy_file(&entry.path(), &staging.join(&*name))?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn icon_only(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("icon.png")
}

fn no_skip(_path: &Path) -> bool {
    false
}

fn copy_dir_all(
    source: &Path,
    destination: &Path,
    skip_file: &dyn Fn(&Path) -> bool,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    fs::create_dir_all(destination).map_err(|error| PortalError::io(destination, error))?;
    let entries = fs::read_dir(source).map_err(|error| PortalError::io(source, error))?;
    for entry in entries {
        cancel.check()?;
        let entry = entry.map_err(|error| PortalError::io(source, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| PortalError::io(entry.path(), error))?;
        if file_type.is_symlink() {
            return Err(PortalError::InvalidInput(format!(
                "template contains a symbolic link: {}",
                entry.path().display()
            )));
        }

        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target, skip_file, cancel)?;
        } else if file_type.is_file() {
            if skip_file(&entry.path()) {
                continue;
            }
            copy_file(&entry.path(), &target)?;
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> PortalResult<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
    }
    fs::copy(source, destination).map_err(|error| PortalError::io(destination, error))?;
    Ok(())
}

pub fn validate_project_name(name: &str) -> PortalResult<()> {
    if name.trim().is_empty() {
        return Err(PortalError::InvalidInput(
            "project name must not be empty".into(),
        ));
    }
    if name == "." || name == ".." {
        return Err(PortalError::InvalidInput(format!(
            "invalid project name: {name}"
        )));
    }
    if name.contains(['/', '\\', '\0', ':', '<', '>', '"', '|', '?', '*']) {
        return Err(PortalError::InvalidInput(format!(
            "project name contains characters that are not allowed: {name}"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(PortalError::InvalidInput(format!(
            "project name contains control characters: {name}"
        )));
    }
    Ok(())
}

fn validate_destination(destination: &Path) -> PortalResult<()> {
    if destination.exists() {
        if !destination.is_dir() {
            return Err(PortalError::Install(format!(
                "destination {} exists and is not a directory",
                destination.display()
            )));
        }
        let mut entries =
            fs::read_dir(destination).map_err(|error| PortalError::io(destination, error))?;
        if entries.next().is_some() {
            return Err(PortalError::Install(format!(
                "destination {} already contains files",
                destination.display()
            )));
        }
        return Ok(());
    }

    if let Some(parent) = destination.parent()
        && parent.exists()
        && !parent.is_dir()
    {
        return Err(PortalError::Install(format!(
            "destination parent {} is not a directory",
            parent.display()
        )));
    }
    Ok(())
}

fn staging_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(".magic-portal-{}-{}", std::process::id(), nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_project_names() {
        assert!(validate_project_name("my-game").is_ok());
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("../escape").is_err());
        assert!(validate_project_name("a/b").is_err());
        assert!(validate_project_name("bad:name").is_err());
    }

    fn fixture_paths(label: &str) -> (crate::core::test_support::TempDir, MagicPaths) {
        let root = crate::core::test_support::TempDir::new(label);
        let paths = MagicPaths::new(root.path().join(".magic-bevy"));
        let version_dir = paths.versions.join("0.1.0");
        let template_dir = version_dir.join("portal").join("templates").join("empty");
        fs::create_dir_all(template_dir.join("layouts")).expect("create template dirs");
        let cargo_config =
            version_dir.join(crate::platform::Os::current().cargo_config_relative_path());
        fs::create_dir_all(cargo_config.parent().expect("cargo config parent"))
            .expect("create config dir");
        fs::write(
            template_dir.join("template.toml"),
            "name = \"Empty\"\ndescription = \"A clean starting point.\"\ncategory = \"none\"\n",
        )
        .expect("write template manifest");
        fs::write(
            template_dir.join("packages.toml"),
            "[dependencies]\nmarker = \"template\"\n",
        )
        .expect("write packages");
        fs::write(template_dir.join("layouts").join("def.layout"), "layout").expect("write layout");
        fs::write(cargo_config, "[build]\nrustc-wrapper = \"sccache\"\n")
            .expect("write cargo config");

        fs::write(
            version_dir.join("magic.toml"),
            "version = \"0.1.0\"\nname = \"nova\"\ndescription = \"Early development build\"\nchannel = \"dev\"\nportal = \"0.1.0\"\n",
        )
        .expect("write magic manifest");
        fs::write(
            version_dir.join(crate::platform::Os::current().executable_name("magic-bevy")),
            b"#!/bin/sh\nexit 0\n",
        )
        .expect("write engine binary");
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "versions.toml",
            "LICENSE",
            "README.md",
            "src/main.rs",
            "magic_core/lib.rs",
            "magic_editor/lib.rs",
            "magic_runtime/lib.rs",
            "magic_pkg/lib.rs",
            "magic_log/lib.rs",
            "magic_api_editor/lib.rs",
            "magic_api_runtime/lib.rs",
            "agent-docs/README.md",
            "assets/icon.svg",
            "packages/packages.toml",
            "layouts/global.layout",
            ".github/workflows/ci.yml",
        ] {
            let file = version_dir.join(path);
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).expect("create engine file parent");
            }
            fs::write(file, format!("engine:{path}")).expect("write engine file");
        }

        let mut registry = InstalledRegistry::default();
        registry.upsert(super::super::paths::InstalledVersion {
            version: "0.1.0".into(),
            name: "nova".into(),
            description: Some("Early development build".into()),
            channel: Some("dev".into()),
            path: "versions/0.1.0".into(),
            installed_at: super::super::paths::now_rfc3339(),
        });
        registry.save(&paths.registry).expect("save registry");
        (root, paths)
    }

    #[test]
    fn builds_and_creates_project() {
        let (root, paths) = fixture_paths("project");
        let destination = root.path().join("projects").join("my-game");
        let plan = build_create_plan("0.1.0", "empty", destination.clone(), &paths)
            .expect("build create plan");

        let cancel = CancelFlag::new();
        let mut reporter = |_event: ProgressEvent| {};
        create_project(&plan, &mut reporter, &cancel).expect("create project");

        assert!(destination.join("magic.toml").is_file());
        assert!(destination.join("Cargo.toml").is_file());
        assert!(destination.join("Cargo.lock").is_file());
        assert!(destination.join("versions.toml").is_file());
        assert!(destination.join("LICENSE").is_file());
        assert!(destination.join("README.md").is_file());
        assert!(destination.join("src").join("main.rs").is_file());
        assert!(destination.join("magic_core").join("lib.rs").is_file());
        assert!(destination.join("agent-docs").join("README.md").is_file());
        assert!(destination.join("assets").join("icon.svg").is_file());
        assert!(!destination.join(".github").exists());
        assert!(destination.join("packages").join("packages.toml").is_file());
        assert!(destination.join("layouts").join("global.layout").is_file());
        assert!(destination.join("layouts").join("def.layout").is_file());
        assert!(destination.join(".cargo").join("config.toml").is_file());
        assert!(!destination.join("template.toml").exists());
        assert!(
            !destination
                .join(crate::platform::Os::current().executable_name("magic-bevy"))
                .exists()
        );
        assert!(!destination.join("portal").exists());

        let packages = fs::read_to_string(destination.join("packages").join("packages.toml"))
            .expect("read packages");
        assert!(packages.contains("marker = \"template\""));
    }

    #[test]
    fn rejects_non_empty_destination() {
        let (root, paths) = fixture_paths("project-nonempty");
        let destination = root.path().join("existing");
        fs::create_dir_all(&destination).expect("create destination");
        fs::write(destination.join("keep.txt"), b"keep").expect("write file");

        assert!(build_create_plan("0.1.0", "empty", destination, &paths).is_err());
    }
}
