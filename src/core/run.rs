use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::platform::Os;

use super::error::{PortalError, PortalResult};
use super::paths::{InstalledRegistry, MagicPaths};
use super::version::EngineManifest;

pub const ENGINE_EXECUTABLE: &str = "magic-bevy";

#[derive(Debug, Clone)]
pub struct RunPlan {
    pub project: PathBuf,
    pub engine_version: String,
    pub engine_name: String,
    pub executable: PathBuf,
}

pub fn build_run_plan(project: &Path, paths: &MagicPaths) -> PortalResult<RunPlan> {
    if !project.is_dir() {
        return Err(PortalError::NotFound(format!(
            "project directory does not exist: {}",
            project.display()
        )));
    }

    let manifest_path = project.join("magic.toml");
    if !manifest_path.is_file() {
        return Err(PortalError::NotFound(format!(
            "project is missing {}",
            manifest_path.display()
        )));
    }

    let content = fs::read_to_string(&manifest_path)
        .map_err(|error| PortalError::io(&manifest_path, error))?;
    let manifest: EngineManifest =
        toml::from_str(&content).map_err(|error| PortalError::InvalidManifest {
            path: manifest_path,
            message: error.to_string(),
        })?;

    let registry = InstalledRegistry::load(&paths.registry)?;
    let entry = registry.get(&manifest.version).ok_or_else(|| {
        PortalError::NotFound(format!(
            "engine version {} used by {} is not installed in {}",
            manifest.version,
            project.display(),
            paths.registry.display()
        ))
    })?;

    let version_dir = paths.resolve_registry_path(entry)?;
    let executable = version_dir.join(Os::current().executable_name(ENGINE_EXECUTABLE));
    if !executable.is_file() {
        return Err(PortalError::NotFound(format!(
            "engine executable not found at {}",
            executable.display()
        )));
    }

    Ok(RunPlan {
        project: project.to_path_buf(),
        engine_version: manifest.version,
        engine_name: manifest.name,
        executable,
    })
}

pub fn run(plan: &RunPlan, args: &[String], json: bool) -> PortalResult<i32> {
    let mut command = Command::new(&plan.executable);
    command.args(args).current_dir(&plan.project);
    if json {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }

    let mut child = command.spawn().map_err(|error| {
        PortalError::Runtime(format!(
            "failed to start {}: {error}",
            plan.executable.display()
        ))
    })?;

    let status = if json {
        wait_forwarded(&mut child, &plan.executable)?
    } else {
        child.wait().map_err(|error| {
            PortalError::Runtime(format!(
                "failed to wait for {}: {error}",
                plan.executable.display()
            ))
        })?
    };

    Ok(status.code().unwrap_or(1))
}

fn wait_forwarded(child: &mut Child, executable: &Path) -> PortalResult<ExitStatus> {
    let stdout = child.stdout.take().ok_or_else(|| {
        PortalError::Runtime(format!(
            "engine stdout was not captured for {}",
            executable.display()
        ))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        PortalError::Runtime(format!(
            "engine stderr was not captured for {}",
            executable.display()
        ))
    })?;

    let (sender, receiver) = mpsc::channel::<String>();
    let stdout_sender = sender.clone();
    let stdout_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if stdout_sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let stderr_thread = thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    for line in receiver {
        eprintln!("{line}");
    }
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    child.wait().map_err(|error| {
        PortalError::Runtime(format!(
            "failed to wait for {}: {error}",
            executable.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(label: &str) -> (crate::core::test_support::TempDir, MagicPaths, PathBuf) {
        let root = crate::core::test_support::TempDir::new(label);
        let paths = MagicPaths::new(root.path().join(".magic-bevy"));
        paths.ensure_layout().expect("layout");

        let version_dir = paths.versions.join("0.1.0");
        fs::create_dir_all(&version_dir).expect("create version");
        let executable = version_dir.join(Os::current().executable_name(ENGINE_EXECUTABLE));
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("write executable");

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

        let project = root.path().join("projects").join("demo");
        fs::create_dir_all(&project).expect("create project");
        fs::write(
            project.join("magic.toml"),
            "version = \"0.1.0\"\nname = \"nova\"\ndescription = \"Early development build\"\nchannel = \"dev\"\nportal = \"0.1.0\"\n",
        )
        .expect("write project manifest");

        (root, paths, project)
    }

    #[test]
    fn builds_run_plan_from_project_manifest() {
        let (_root, paths, project) = fixture("run-plan");
        let plan = build_run_plan(&project, &paths).expect("build run plan");
        assert_eq!(plan.engine_version, "0.1.0");
        assert_eq!(plan.engine_name, "nova");
        assert_eq!(
            plan.executable,
            paths
                .versions
                .join("0.1.0")
                .join(Os::current().executable_name(ENGINE_EXECUTABLE))
        );
    }

    #[test]
    fn missing_project_manifest_is_an_error() {
        let root = crate::core::test_support::TempDir::new("run-missing-manifest");
        let paths = MagicPaths::new(root.path().join(".magic-bevy"));
        let project = root.path().join("project");
        fs::create_dir_all(&project).expect("create project");
        assert!(build_run_plan(&project, &paths).is_err());
    }

    #[test]
    fn missing_executable_is_an_error() {
        let (root, paths, project) = fixture("run-missing-exe");
        fs::remove_file(
            paths
                .versions
                .join("0.1.0")
                .join(Os::current().executable_name(ENGINE_EXECUTABLE)),
        )
        .expect("remove executable");
        assert!(build_run_plan(&project, &paths).is_err());
        drop(root);
    }
}
