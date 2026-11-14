use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::platform::Os;

use super::error::{PortalError, PortalResult};
use super::github::{self, ReleaseAsset};
use super::paths::{
    InstalledRegistry, InstalledVersion, MagicPaths, now_rfc3339, validate_version,
};
use super::progress::{CancelFlag, ProgressEvent, Reporter};
use super::version::{self, EngineReleaseEntry};

const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub release: EngineReleaseEntry,
    pub release_url: String,
    pub source: ReleaseAsset,
    pub binary: ReleaseAsset,
    pub destination: PathBuf,
    pub spell_path: PathBuf,
    pub spell_command: String,
    pub existing: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    pub replace_existing: bool,
    pub run_spells: bool,
}

#[derive(Debug, Clone)]
pub struct UninstallPlan {
    pub version: String,
    pub name: String,
    pub channel: String,
    pub path: PathBuf,
    pub registered: bool,
    pub cache_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct UninstallOptions {
    pub purge_cache: bool,
    pub force: bool,
}

pub fn build_install_plan(
    release: &EngineReleaseEntry,
    paths: &MagicPaths,
    os: Os,
) -> PortalResult<InstallPlan> {
    validate_version(&release.version)?;

    let releases = github::fetch_releases()?;
    let github_release = github::find_release(&releases, &release.version).ok_or_else(|| {
        PortalError::NotFound(format!(
            "GitHub release for Magic Bevy {} was not found in {}/{}",
            release.version,
            github::ENGINE_OWNER,
            github::ENGINE_REPO
        ))
    })?;
    let (source, binary) = github::resolve_assets(github_release, &release.version, os)?;

    let destination = paths.version_dir(&release.version)?;
    let spell_path = destination.join(os.spell_relative_path());
    let args = os.spell_args(&spell_path);
    let spell_command = format_command(os.spell_program(), &args);

    let registry = InstalledRegistry::load(&paths.registry)?;
    let existing = registry.get(&release.version).is_some() || destination.exists();

    Ok(InstallPlan {
        release: release.clone(),
        release_url: github::release_page_url(github_release),
        source,
        binary,
        destination,
        spell_path,
        spell_command,
        existing,
    })
}

pub fn install(
    plan: &InstallPlan,
    options: &InstallOptions,
    paths: &MagicPaths,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    if plan.existing && !options.replace_existing {
        return Err(PortalError::Install(format!(
            "version {} is already installed; confirm replacement before installing",
            plan.release.version
        )));
    }

    let version = &plan.release.version;
    let os = Os::current();
    paths.ensure_layout()?;

    let staging = paths.staging.join(version);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| PortalError::io(&staging, error))?;
    }
    fs::create_dir_all(&staging).map_err(|error| PortalError::io(&staging, error))?;

    let result = install_inner(plan, options, paths, os, &staging, reporter, cancel);
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn install_inner(
    plan: &InstallPlan,
    options: &InstallOptions,
    paths: &MagicPaths,
    os: Os,
    staging: &Path,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    let version = &plan.release.version;
    cancel.check()?;

    reporter.report(ProgressEvent::step(format!(
        "downloading {}",
        plan.source.name
    )));
    let source_path = paths.downloads.join(&plan.source.name);
    github::download(
        &plan.source.url,
        &plan.source.name,
        &source_path,
        MAX_DOWNLOAD_BYTES,
        reporter,
        cancel,
    )?;
    github::verify_checksum(
        &source_path,
        plan.source.checksum_url.as_deref(),
        plan.source.digest.as_deref(),
        &plan.source.name,
        reporter,
    )?;

    cancel.check()?;
    reporter.report(ProgressEvent::step(format!(
        "extracting {}",
        plan.source.name
    )));
    extract_archive(&source_path, staging, cancel)?;
    flatten_single_root(staging)?;

    cancel.check()?;
    reporter.report(ProgressEvent::step(format!(
        "downloading {}",
        plan.binary.name
    )));
    let binary_path = paths.downloads.join(&plan.binary.name);
    github::download(
        &plan.binary.url,
        &plan.binary.name,
        &binary_path,
        MAX_DOWNLOAD_BYTES,
        reporter,
        cancel,
    )?;
    github::verify_checksum(
        &binary_path,
        plan.binary.checksum_url.as_deref(),
        plan.binary.digest.as_deref(),
        &plan.binary.name,
        reporter,
    )?;

    cancel.check()?;
    reporter.report(ProgressEvent::step(format!(
        "extracting {}",
        plan.binary.name
    )));
    extract_archive(&binary_path, staging, cancel)?;

    if !version::manifest_path(staging).is_file() {
        return Err(PortalError::Install(format!(
            "source archive did not contain magic.toml at {}",
            staging.display()
        )));
    }

    cancel.check()?;
    let spell_path = staging.join(os.spell_relative_path());
    if !spell_path.is_file() {
        return Err(PortalError::Install(format!(
            "prerequisite spell not found at {}",
            spell_path.display()
        )));
    }

    if options.run_spells {
        run_spell(&spell_path, staging, os, reporter, cancel)?;
    } else {
        reporter.report(ProgressEvent::log(
            "spell execution skipped by caller".to_string(),
        ));
    }

    cancel.check()?;
    let backup_path = staging
        .parent()
        .map(|parent| parent.join(format!("backup-{version}-{}", std::process::id())));
    let moved_backup = commit_staging(
        staging,
        &plan.destination,
        backup_path.as_deref(),
        &plan.release.version,
        reporter,
    )?;

    let manifest = version::read_engine_manifest(&plan.destination).ok();
    let name = manifest
        .as_ref()
        .map(|entry| entry.name.clone())
        .or_else(|| plan.release.name.clone())
        .unwrap_or_else(|| plan.release.version.clone());
    let description = manifest
        .as_ref()
        .and_then(|entry| entry.description.clone())
        .or_else(|| plan.release.description.clone());
    let channel = manifest
        .as_ref()
        .map(|entry| entry.channel.clone())
        .unwrap_or_else(|| plan.release.channel.clone());

    let mut registry = InstalledRegistry::load(&paths.registry)?;
    registry.upsert(InstalledVersion {
        version: plan.release.version.clone(),
        name,
        description,
        channel: Some(channel),
        path: format!("versions/{}", plan.release.version),
        installed_at: now_rfc3339(),
    });
    if let Err(error) = registry.save(&paths.registry) {
        if let Some(backup) = &moved_backup {
            let _ = fs::remove_dir_all(&plan.destination);
            if let Err(restore_error) = fs::rename(backup, &plan.destination) {
                return Err(PortalError::Install(format!(
                    "failed to register {} ({error}) and failed to restore the previous version from {}: {restore_error}",
                    plan.destination.display(),
                    backup.display()
                )));
            }
        } else {
            let _ = fs::remove_dir_all(&plan.destination);
        }
        return Err(PortalError::Install(format!(
            "installed files at {} but failed to register the version: {error}",
            plan.destination.display()
        )));
    }

    if let Some(backup) = moved_backup
        && backup.exists()
        && let Err(error) = fs::remove_dir_all(&backup)
    {
        reporter.report(ProgressEvent::log(format!(
            "installed version but could not remove backup {}: {error}",
            backup.display()
        )));
    }

    reporter.report(ProgressEvent::step(format!(
        "installed Magic Bevy {} at {}",
        plan.release.version,
        plan.destination.display()
    )));
    Ok(())
}

fn commit_staging(
    staging: &Path,
    destination: &Path,
    backup: Option<&Path>,
    version: &str,
    reporter: &mut dyn Reporter,
) -> PortalResult<Option<PathBuf>> {
    if destination.exists() && !destination.is_dir() {
        return Err(PortalError::Install(format!(
            "destination {} exists and is not a directory",
            destination.display()
        )));
    }

    let mut moved_backup = None;
    if destination.exists() {
        let Some(backup) = backup else {
            return Err(PortalError::Install(format!(
                "cannot replace {} without a backup path",
                destination.display()
            )));
        };
        if backup.exists() {
            fs::remove_dir_all(backup).map_err(|error| PortalError::io(backup, error))?;
        }
        fs::rename(destination, backup).map_err(|error| PortalError::io(destination, error))?;
        moved_backup = Some(backup.to_path_buf());
        reporter.report(ProgressEvent::step(format!(
            "replacing existing version {version}"
        )));
    }

    if let Err(error) = fs::rename(staging, destination) {
        if let Some(backup) = &moved_backup {
            let _ = fs::rename(backup, destination);
        }
        return Err(PortalError::io(destination, error));
    }

    Ok(moved_backup)
}

fn flatten_single_root(root: &Path) -> PortalResult<()> {
    let mut entries = fs::read_dir(root).map_err(|error| PortalError::io(root, error))?;
    let Some(first) = entries.next() else {
        return Ok(());
    };
    if entries.next().is_some() {
        return Ok(());
    }

    let first = first.map_err(|error| PortalError::io(root, error))?;
    let file_type = first
        .file_type()
        .map_err(|error| PortalError::io(first.path(), error))?;
    if !file_type.is_dir() {
        return Ok(());
    }

    let inner = first.path();
    if !inner.join("portal").is_dir() && !inner.join("magic.toml").is_file() {
        return Ok(());
    }

    let children = fs::read_dir(&inner).map_err(|error| PortalError::io(&inner, error))?;
    for child in children {
        let child = child.map_err(|error| PortalError::io(&inner, error))?;
        let target = root.join(child.file_name());
        fs::rename(child.path(), &target).map_err(|error| PortalError::io(&target, error))?;
    }
    fs::remove_dir(&inner).map_err(|error| PortalError::io(&inner, error))?;
    Ok(())
}

fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    let name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive_path, destination, cancel)
    } else if name.ends_with(".zip") {
        extract_zip(archive_path, destination, cancel)
    } else {
        Err(PortalError::Unsupported(format!(
            "unsupported archive format: {}",
            archive_path.display()
        )))
    }
}

fn extract_tar_gz(
    archive_path: &Path,
    destination: &Path,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    let file = File::open(archive_path).map_err(|error| PortalError::io(archive_path, error))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    fs::create_dir_all(destination).map_err(|error| PortalError::io(destination, error))?;

    let mut total = 0_u64;
    let entries = archive.entries().map_err(|error| {
        PortalError::Install(format!(
            "failed to read archive {}: {error}",
            archive_path.display()
        ))
    })?;

    for entry in entries {
        cancel.check()?;
        let mut entry = entry.map_err(|error| {
            PortalError::Install(format!(
                "failed to read an entry in {}: {error}",
                archive_path.display()
            ))
        })?;

        let header = entry.header();
        total = total.saturating_add(header.size().map_err(|error| {
            PortalError::Install(format!(
                "failed to read the size of an entry in {}: {error}",
                archive_path.display()
            ))
        })?);
        if total > MAX_EXTRACT_BYTES {
            return Err(PortalError::Install(format!(
                "archive {} is larger than the {MAX_EXTRACT_BYTES} byte limit",
                archive_path.display()
            )));
        }

        let entry_type = header.entry_type();
        #[cfg(unix)]
        let mode = header
            .mode()
            .map_err(|error| PortalError::io(archive_path, error))?;

        let path = entry.path().map_err(|error| {
            PortalError::Install(format!(
                "failed to read an entry path in {}: {error}",
                archive_path.display()
            ))
        })?;
        let relative = sanitize_tar_path(&path)?;
        if relative.as_os_str().is_empty() {
            if entry_type.is_dir() {
                continue;
            }
            return Err(PortalError::Install(format!(
                "empty archive entry path: {}",
                path.display()
            )));
        }

        let output = destination.join(relative);
        if !output.starts_with(destination) {
            return Err(PortalError::Install(format!(
                "archive entry escapes destination: {}",
                path.display()
            )));
        }

        if entry_type.is_dir() {
            fs::create_dir_all(&output).map_err(|error| PortalError::io(&output, error))?;
            continue;
        }
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(PortalError::Install(format!(
                "archive {} contains a symbolic or hard link: {}",
                archive_path.display(),
                path.display()
            )));
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
        }
        entry
            .unpack(&output)
            .map_err(|error| PortalError::io(&output, error))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&output, fs::Permissions::from_mode(mode & 0o777));
        }
    }

    Ok(())
}

fn sanitize_tar_path(path: &Path) -> PortalResult<PathBuf> {
    use std::path::Component;

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PortalError::Install(format!(
                    "unsafe archive entry path: {}",
                    path.display()
                )));
            }
        }
    }

    Ok(safe)
}

fn extract_zip(archive_path: &Path, destination: &Path, cancel: &CancelFlag) -> PortalResult<()> {
    let file = File::open(archive_path).map_err(|error| PortalError::io(archive_path, error))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        PortalError::Install(format!(
            "failed to open archive {}: {error}",
            archive_path.display()
        ))
    })?;

    fs::create_dir_all(destination).map_err(|error| PortalError::io(destination, error))?;

    let mut total = 0_u64;
    for index in 0..archive.len() {
        cancel.check()?;
        let mut entry = archive.by_index(index).map_err(|error| {
            PortalError::Install(format!(
                "failed to read archive entry {index} in {}: {error}",
                archive_path.display()
            ))
        })?;

        total = total.saturating_add(entry.size());
        if total > MAX_EXTRACT_BYTES {
            return Err(PortalError::Install(format!(
                "archive {} is larger than the {MAX_EXTRACT_BYTES} byte limit",
                archive_path.display()
            )));
        }

        let relative = entry.enclosed_name().ok_or_else(|| {
            PortalError::Install(format!(
                "archive {} contains an unsafe entry path: {}",
                archive_path.display(),
                entry.name()
            ))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let output = destination.join(relative);
        if !output.starts_with(destination) {
            return Err(PortalError::Install(format!(
                "archive entry escapes destination: {}",
                entry.name()
            )));
        }

        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| PortalError::io(&output, error))?;
            continue;
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
        }

        let mut output_file =
            File::create(&output).map_err(|error| PortalError::io(&output, error))?;
        std::io::copy(&mut entry, &mut output_file)
            .map_err(|error| PortalError::io(&output, error))?;
        output_file
            .flush()
            .map_err(|error| PortalError::io(&output, error))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&output, fs::Permissions::from_mode(mode & 0o777));
            }
        }
    }

    Ok(())
}

pub fn build_uninstall_plan(version: &str, paths: &MagicPaths) -> PortalResult<UninstallPlan> {
    validate_version(version)?;
    let registry = InstalledRegistry::load(&paths.registry)?;
    let entry = registry.get(version);
    let path = match entry {
        Some(entry) => paths.resolve_registry_path(entry)?,
        None => paths.version_dir(version)?,
    };

    let cache_paths = cache_paths_for_version(paths, version);

    if path.is_symlink() {
        return Err(PortalError::InvalidInput(format!(
            "refusing to remove symbolic link at {}",
            path.display()
        )));
    }
    if path.exists() && !path.is_dir() {
        return Err(PortalError::InvalidInput(format!(
            "refusing to remove non-directory path {}",
            path.display()
        )));
    }

    Ok(UninstallPlan {
        version: version.to_string(),
        name: entry
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| "-".to_string()),
        channel: entry
            .and_then(|entry| entry.channel.clone())
            .unwrap_or_else(|| "-".to_string()),
        path,
        registered: entry.is_some(),
        cache_paths,
    })
}

pub fn uninstall(
    plan: &UninstallPlan,
    options: &UninstallOptions,
    paths: &MagicPaths,
    reporter: &mut dyn Reporter,
) -> PortalResult<()> {
    let version = &plan.version;

    if plan.path.exists() {
        if !plan.registered && !options.force {
            return Err(PortalError::NotFound(format!(
                "version {version} is not registered in {}; confirm removal before deleting {}",
                paths.registry.display(),
                plan.path.display()
            )));
        }

        reporter.report(ProgressEvent::step(format!(
            "removing {}",
            plan.path.display()
        )));
        fs::remove_dir_all(&plan.path).map_err(|error| PortalError::io(&plan.path, error))?;
    } else if plan.registered {
        reporter.report(ProgressEvent::log(format!(
            "version {version} directory was already missing; removing the registry entry"
        )));
    }

    if plan.registered {
        let mut registry = InstalledRegistry::load(&paths.registry)?;
        if registry.remove(version) {
            registry.save(&paths.registry)?;
        }
    }

    if options.purge_cache {
        purge_cache(plan, paths, reporter)?;
    }

    reporter.report(ProgressEvent::step(format!(
        "uninstalled Magic Bevy version {version}"
    )));
    Ok(())
}

fn cache_paths_for_version(paths: &MagicPaths, version: &str) -> Vec<PathBuf> {
    let mut paths_found = Vec::new();

    let staging = paths.staging.join(version);
    if staging.exists() {
        paths_found.push(staging);
    }

    if let Ok(entries) = fs::read_dir(&paths.staging) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("backup-{version}-")) {
                paths_found.push(entry.path());
            }
        }
    }

    if let Ok(entries) = fs::read_dir(&paths.downloads) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if cache_matches_version(&name, version) {
                paths_found.push(entry.path());
            }
        }
    }

    paths_found
}

fn cache_matches_version(name: &str, version: &str) -> bool {
    name == version
        || name.contains(&format!("-{version}-"))
        || name.ends_with(&format!("-{version}.zip"))
        || name.ends_with(&format!("-{version}.zip.sha256"))
        || name.ends_with(&format!("-{version}.zip.sha256sum"))
}

fn purge_cache(
    plan: &UninstallPlan,
    paths: &MagicPaths,
    reporter: &mut dyn Reporter,
) -> PortalResult<()> {
    for path in &plan.cache_paths {
        if !path.exists() {
            continue;
        }
        reporter.report(ProgressEvent::step(format!(
            "removing cache {}",
            path.display()
        )));
        let result = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(error) = result {
            reporter.report(ProgressEvent::log(format!(
                "could not remove cache {}: {error}",
                path.display()
            )));
        }
    }

    let staging = paths.staging.join(&plan.version);
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    Ok(())
}

pub fn format_command(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote(program));
    parts.extend(args.iter().map(|argument| quote(argument)));
    parts.join(" ")
}

fn quote(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

enum SpellLine {
    Out(String),
    Err(String),
}

fn run_spell(
    spell: &Path,
    working_dir: &Path,
    os: Os,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<()> {
    let program = os.spell_program();
    let args = os.spell_args(spell);
    reporter.report(ProgressEvent::step(format!(
        "running prerequisite spell: {}",
        format_command(program, &args)
    )));

    let mut child = Command::new(program)
        .args(&args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            PortalError::Install(format!(
                "failed to start prerequisite spell {program}: {error}"
            ))
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PortalError::Install("prerequisite spell stdout was not captured".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| PortalError::Install("prerequisite spell stderr was not captured".into()))?;

    let (sender, receiver) = mpsc::channel::<SpellLine>();
    let stdout_sender = sender.clone();
    let stdout_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if stdout_sender.send(SpellLine::Out(line)).is_err() {
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
                    if sender.send(SpellLine::Err(line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let status = loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(PortalError::Cancelled);
        }

        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(SpellLine::Out(line)) => reporter.report(ProgressEvent::log(line)),
            Ok(SpellLine::Err(line)) => reporter.report(ProgressEvent::log(line)),
            Err(RecvTimeoutError::Timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| PortalError::io(working_dir, error))?
                {
                    break status;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                let status = child
                    .wait()
                    .map_err(|error| PortalError::io(working_dir, error))?;
                break status;
            }
        }
    };

    for line in receiver.iter() {
        match line {
            SpellLine::Out(line) | SpellLine::Err(line) => {
                reporter.report(ProgressEvent::log(line))
            }
        }
    }
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    if status.success() {
        Ok(())
    } else {
        Err(PortalError::Install(format!(
            "prerequisite spell exited with status {status}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    fn write_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).expect("create tar.gz");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (entry_path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, *entry_path, Cursor::new(*contents))
                .expect("append tar entry");
        }
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");
    }

    #[test]
    fn rejects_archive_entries_that_escape_destination() {
        let root = crate::core::test_support::TempDir::new("extract");
        let archive_path = root.path().join("evil.zip");
        let destination = root.path().join("destination");

        let file = File::create(&archive_path).expect("create archive");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../../evil.txt", SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(b"evil").expect("write archive entry");
        writer.finish().expect("finish archive");

        let cancel = CancelFlag::new();
        assert!(extract_zip(&archive_path, &destination, &cancel).is_err());
    }

    #[test]
    fn extracts_normal_entries() {
        let root = crate::core::test_support::TempDir::new("extract-ok");
        let archive_path = root.path().join("ok.zip");
        let destination = root.path().join("destination");

        let file = File::create(&archive_path).expect("create archive");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .add_directory("portal/spells/", SimpleFileOptions::default())
            .expect("create dir entry");
        writer
            .start_file("portal/spells/linux.sh", SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(b"#!/bin/sh\n").expect("write entry");
        writer.finish().expect("finish archive");

        let cancel = CancelFlag::new();
        extract_zip(&archive_path, &destination, &cancel).expect("extract");
        assert!(destination.join("portal/spells/linux.sh").is_file());
    }

    #[test]
    fn extracts_tar_gz_entries() {
        let root = crate::core::test_support::TempDir::new("extract-tar");
        let archive_path = root.path().join("ok.tar.gz");
        let destination = root.path().join("destination");
        write_tar_gz(
            &archive_path,
            &[
                ("portal/spells/", b""),
                ("portal/spells/linux.sh", b"#!/bin/sh\n"),
            ],
        );

        let cancel = CancelFlag::new();
        extract_tar_gz(&archive_path, &destination, &cancel).expect("extract tar");
        assert!(destination.join("portal/spells/linux.sh").is_file());
    }

    #[test]
    fn rejects_tar_paths_that_escape_destination() {
        assert!(sanitize_tar_path(Path::new("../evil.txt")).is_err());
        assert!(sanitize_tar_path(Path::new("/etc/passwd")).is_err());
        assert!(sanitize_tar_path(Path::new("portal/spells/linux.sh")).is_ok());
    }

    #[test]
    fn flattens_single_source_root() {
        let root = crate::core::test_support::TempDir::new("flatten");
        let staging = root.path().join("staging");
        let source = staging.join("magic-bevy-0.1.0");
        fs::create_dir_all(source.join("portal").join("spells")).expect("create source");
        fs::write(source.join("magic.toml"), "version = \"0.1.0\"\n").expect("write manifest");
        fs::write(
            source.join("portal").join("spells").join("linux.sh"),
            b"#!/bin/sh\n",
        )
        .expect("write spell");

        flatten_single_root(&staging).expect("flatten");
        assert!(staging.join("magic.toml").is_file());
        assert!(
            staging
                .join("portal")
                .join("spells")
                .join("linux.sh")
                .is_file()
        );
        assert!(!source.exists());
    }
}
