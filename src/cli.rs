use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use crate::core::doctor;
use crate::core::error::{PortalError, PortalResult};
use crate::core::github;
use crate::core::installer::{self, InstallOptions, UninstallOptions};
use crate::core::paths::{InstalledRegistry, MagicPaths, expand_path};
use crate::core::progress::{CancelFlag, ProgressEvent, Reporter};
use crate::core::project;
use crate::core::run;
use crate::core::templates;
use crate::core::version::{self, EngineReleaseEntry};
use crate::platform::Os;

#[derive(Debug, Parser)]
#[command(
    name = "magic-portal",
    version,
    about = "Launcher and project manager for the Magic Bevy game engine",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List Magic Bevy releases compatible with this portal.
    Releases {
        /// Include older compatible releases.
        #[arg(long)]
        include_older: bool,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
    /// Download and install one engine version.
    Install {
        /// Engine version to install.
        version: String,
        /// Confirm without prompting.
        #[arg(long)]
        yes: bool,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
    /// Remove one installed engine version.
    Uninstall {
        /// Engine version to remove.
        version: String,
        /// Confirm deletion without prompting.
        #[arg(long)]
        yes: bool,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
        /// Also remove cached downloads for this version.
        #[arg(long)]
        purge_cache: bool,
    },
    /// List installed engine versions.
    Installed {
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
    /// List templates for an installed engine version.
    Templates {
        /// Installed engine version.
        #[arg(long)]
        version: String,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
    /// Create a project from an installed engine version and template.
    Create {
        /// Installed engine version.
        #[arg(long)]
        engine: String,
        /// Template directory name.
        #[arg(long)]
        template: String,
        /// Project name.
        #[arg(long)]
        name: String,
        /// Project destination path.
        #[arg(long)]
        path: String,
        /// Confirm creation without prompting.
        #[arg(long)]
        yes: bool,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
    /// Run a project with its engine.
    Run {
        /// Project directory.
        #[arg(long, default_value = ".")]
        path: String,
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
        /// Arguments passed to the engine after `--`.
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Check the portal environment.
    Doctor {
        /// Print a machine-readable JSON envelope.
        #[arg(long)]
        json: bool,
    },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Releases { .. } => "releases",
            Self::Install { .. } => "install",
            Self::Uninstall { .. } => "uninstall",
            Self::Installed { .. } => "installed",
            Self::Templates { .. } => "templates",
            Self::Create { .. } => "create",
            Self::Run { .. } => "run",
            Self::Doctor { .. } => "doctor",
        }
    }

    pub fn json(&self) -> bool {
        match self {
            Self::Releases { json, .. }
            | Self::Install { json, .. }
            | Self::Uninstall { json, .. }
            | Self::Installed { json }
            | Self::Templates { json, .. }
            | Self::Create { json, .. }
            | Self::Run { json, .. }
            | Self::Doctor { json } => *json,
        }
    }
}

struct CommandOutput {
    data: Value,
    human: String,
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    let command_name = cli.command.name();
    let json = cli.command.json();

    match execute(&cli.command) {
        Ok(output) => {
            if json {
                let envelope = json!({
                    "ok": true,
                    "command": command_name,
                    "data": output.data,
                });
                print_json(&envelope);
            } else {
                print!("{}", output.human);
                let _ = std::io::stdout().flush();
            }
            0
        }
        Err(error) => {
            if json {
                let envelope = json!({
                    "ok": false,
                    "command": command_name,
                    "error": {
                        "kind": error.kind(),
                        "message": error.to_string(),
                    },
                });
                print_json(&envelope);
            } else {
                eprintln!("error: {error}");
            }
            error.exit_code()
        }
    }
}

fn print_json(value: &Value) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("error: failed to serialize JSON output: {error}");
            println!(
                "{{\"ok\":false,\"error\":{{\"kind\":\"io\",\"message\":\"failed to serialize JSON output\"}}}}"
            );
        }
    }
}

fn execute(command: &Command) -> PortalResult<CommandOutput> {
    match command {
        Command::Releases { include_older, .. } => execute_releases(*include_older),
        Command::Install {
            version, yes, json, ..
        } => execute_install(version, *yes, *json),
        Command::Uninstall {
            version,
            yes,
            json,
            purge_cache,
        } => execute_uninstall(version, *yes, *json, *purge_cache),
        Command::Installed { .. } => execute_installed(),
        Command::Templates { version, .. } => execute_templates(version),
        Command::Create {
            engine,
            template,
            name,
            path,
            yes,
            json,
        } => execute_create(engine, template, name, path, *yes, *json),
        Command::Run { path, json, args } => execute_run(path, *json, args),
        Command::Doctor { .. } => execute_doctor(),
    }
}

fn execute_releases(include_older: bool) -> PortalResult<CommandOutput> {
    let portal_version = env!("CARGO_PKG_VERSION");
    let file = github::fetch_versions_toml()?;
    let mut compatible: Vec<EngineReleaseEntry> = file
        .releases
        .iter()
        .filter(|release| release.is_compatible(portal_version))
        .cloned()
        .collect();

    if compatible.is_empty() {
        let mut available: Vec<String> = file
            .releases
            .iter()
            .map(|release| release.portal.clone())
            .collect();
        available.sort();
        available.dedup();
        let available = if available.is_empty() {
            "none".to_string()
        } else {
            available.join(", ")
        };
        return Err(PortalError::NotFound(format!(
            "no releases compatible with portal {portal_version}; available portal versions: {available}"
        )));
    }

    version::sort_newest_first(&mut compatible);
    let older_count = compatible.len().saturating_sub(5);

    let mut shown = Vec::new();
    if include_older {
        for (index, release) in compatible.iter().enumerate() {
            shown.push((release.clone(), index >= 5));
        }
    } else {
        for release in compatible.iter().take(5) {
            shown.push((release.clone(), false));
        }
    }

    let mut human = format!("Compatible Magic Bevy releases (portal {portal_version})\n");
    if include_older {
        for (index, release) in compatible.iter().enumerate() {
            if index == 5 {
                human.push_str("\nOlder releases\n");
            }
            human.push_str(&release_line(release));
            human.push('\n');
        }
    } else {
        for release in compatible.iter().take(5) {
            human.push_str(&release_line(release));
            human.push('\n');
        }
        if older_count > 0 {
            human.push_str(&format!(
                "\n... {older_count} older compatible release(s). Use --include-older to show them.\n"
            ));
        }
    }

    let releases: Vec<Value> = shown
        .iter()
        .map(|(release, older)| release_json(release, *older))
        .collect();
    let data = json!({
        "releases": releases,
        "older_count": older_count,
        "include_older": include_older,
    });

    Ok(CommandOutput { data, human })
}

fn execute_installed() -> PortalResult<CommandOutput> {
    let paths = MagicPaths::open()?;
    let registry = InstalledRegistry::load(&paths.registry)?;

    let mut human = String::new();
    if registry.versions.is_empty() {
        human.push_str("No installed engine versions.\n");
    } else {
        for entry in &registry.versions {
            human.push_str(&format!(
                "{}  {}  {}\n",
                entry.version,
                entry.name,
                paths
                    .resolve_registry_path(entry)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| entry.path.clone())
            ));
        }
    }

    let versions: Vec<Value> = registry
        .versions
        .iter()
        .map(|entry| {
            json!({
                "version": entry.version,
                "name": entry.name,
                "description": entry.description,
                "channel": entry.channel,
                "path": entry.path,
                "installed_at": entry.installed_at,
            })
        })
        .collect();

    Ok(CommandOutput {
        data: json!({ "versions": versions }),
        human,
    })
}

fn execute_templates(version: &str) -> PortalResult<CommandOutput> {
    let paths = MagicPaths::open()?;
    let version_dir = paths.version_dir(version)?;
    if !version_dir.is_dir() {
        return Err(PortalError::NotFound(format!(
            "engine version {version} is not installed at {}",
            version_dir.display()
        )));
    }

    let mut reporter = |_event: ProgressEvent| {};
    let templates = templates::discover_templates(&version_dir, &mut reporter)?;

    let mut human = format!("Templates for Magic Bevy {version}\n");
    if templates.is_empty() {
        human.push_str("No templates found.\n");
    }
    for template in &templates {
        human.push_str(&format!(
            "{}  {}  {}  {}  icon: {}\n",
            template.directory,
            template.name,
            template.description,
            template.category,
            template
                .icon
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
    }

    let data = json!({
        "version": version,
        "templates": templates,
    });

    Ok(CommandOutput { data, human })
}

fn execute_install(version: &str, yes: bool, json: bool) -> PortalResult<CommandOutput> {
    let paths = MagicPaths::open()?;
    let portal_version = env!("CARGO_PKG_VERSION");
    let file = github::fetch_versions_toml()?;
    let release = file
        .releases
        .iter()
        .find(|release| release.version == version)
        .ok_or_else(|| {
            let available: Vec<String> = file
                .releases
                .iter()
                .map(|release| release.version.clone())
                .collect();
            PortalError::NotFound(format!(
                "version {version} was not found in versions.toml; available versions: {}",
                if available.is_empty() {
                    "none".to_string()
                } else {
                    available.join(", ")
                }
            ))
        })?;

    if !release.is_compatible(portal_version) {
        return Err(PortalError::NotFound(format!(
            "version {version} requires portal {}, but this portal is {portal_version}",
            release.portal
        )));
    }

    let plan = installer::build_install_plan(release, &paths, Os::current())?;

    eprintln!("Install Magic Bevy {}", release.version);
    eprintln!("  name:         {}", release.display_name());
    eprintln!("  description:  {}", release.display_description());
    eprintln!("  channel:      {}", release.channel);
    eprintln!("  release:      {}", plan.release_url);
    eprintln!("  source:       {}", plan.source.url);
    eprintln!("  source size:  {}", format_size(plan.source.size));
    eprintln!("  os asset:     {}", plan.binary.url);
    eprintln!("  os size:      {}", format_size(plan.binary.size));
    eprintln!("  download:     2 files");
    eprintln!("  destination:  {}", plan.destination.display());
    eprintln!("  spell:        {}", plan.spell_command);
    eprintln!("  spell path:   {}", plan.spell_path.display());
    if plan.existing {
        eprintln!("  existing:     yes (will be replaced)");
    } else {
        eprintln!("  existing:     no");
    }

    let prompt = if plan.existing {
        format!(
            "Replace existing Magic Bevy {} at {}?",
            release.version,
            plan.destination.display()
        )
    } else {
        format!(
            "Download and install Magic Bevy {} at {}?",
            release.version,
            plan.destination.display()
        )
    };
    confirm(&prompt, yes, json)?;

    let replace_existing = plan.existing;
    let mut reporter = CliReporter::new(json);
    let cancel = CancelFlag::new();
    let handler_cancel = cancel.clone();
    let _ = ctrlc::set_handler(move || handler_cancel.cancel());
    installer::install(
        &plan,
        &InstallOptions {
            replace_existing,
            run_spells: true,
        },
        &paths,
        &mut reporter,
        &cancel,
    )?;

    let human = format!(
        "Installed Magic Bevy {} at {}\n",
        release.version,
        plan.destination.display()
    );
    let data = json!({
        "version": release.version,
        "path": plan.destination,
        "source_url": plan.source.url,
        "binary_url": plan.binary.url,
        "release_url": plan.release_url,
        "repository": github::repository_url(),
        "download_count": 2,
        "spell": plan.spell_command,
    });

    Ok(CommandOutput { data, human })
}

fn execute_uninstall(
    version: &str,
    yes: bool,
    json: bool,
    purge_cache: bool,
) -> PortalResult<CommandOutput> {
    let paths = MagicPaths::open()?;
    let plan = installer::build_uninstall_plan(version, &paths)?;

    eprintln!("Uninstall Magic Bevy {}", plan.version);
    eprintln!("  name:      {}", plan.name);
    eprintln!("  channel:   {}", plan.channel);
    eprintln!("  path:      {}", plan.path.display());
    eprintln!(
        "  registry:  {}",
        if plan.registered { "yes" } else { "no" }
    );
    eprintln!("  cache:     {} path(s)", plan.cache_paths.len());

    let prompt = if plan.registered {
        format!(
            "Remove Magic Bevy {} from {}?",
            plan.version,
            plan.path.display()
        )
    } else {
        format!(
            "Version {} is not registered in {}. Remove {} anyway?",
            plan.version,
            paths.registry.display(),
            plan.path.display()
        )
    };
    confirm(&prompt, yes, json)?;

    let mut reporter = CliReporter::new(json);
    installer::uninstall(
        &plan,
        &UninstallOptions {
            purge_cache,
            force: true,
        },
        &paths,
        &mut reporter,
    )?;

    let human = format!(
        "Uninstalled Magic Bevy version {} from {}\n",
        plan.version,
        plan.path.display()
    );
    let data = json!({
        "version": plan.version,
        "path": plan.path,
        "registered": plan.registered,
        "purge_cache": purge_cache,
    });

    Ok(CommandOutput { data, human })
}

fn execute_create(
    engine: &str,
    template: &str,
    name: &str,
    path: &str,
    yes: bool,
    json: bool,
) -> PortalResult<CommandOutput> {
    project::validate_project_name(name)?;
    let destination = expand_path(path)?;
    let paths = MagicPaths::open()?;
    let plan = project::build_create_plan(engine, template, destination, &paths)?;

    eprintln!("Create Magic Bevy project");
    eprintln!("  project:   {name}");
    eprintln!(
        "  engine:    {} \"{}\"",
        plan.engine_version, plan.engine_name
    );
    eprintln!("  template:  {} \"{}\"", plan.template, plan.template_name);
    eprintln!("  source:    {}", plan.template_dir.display());
    eprintln!("  destination: {}", plan.destination.display());

    confirm("Proceed with project creation?", yes, json)?;

    let mut reporter = CliReporter::new(json);
    let cancel = CancelFlag::new();
    let project_path = project::create_project(&plan, &mut reporter, &cancel)?;

    let human = format!("Created project at {}\n", project_path.display());
    let data = json!({
        "project": project_path,
        "engine": plan.engine_version,
        "engine_name": plan.engine_name,
        "template": plan.template,
        "template_name": plan.template_name,
    });

    Ok(CommandOutput { data, human })
}

fn execute_run(path: &str, json: bool, args: &[String]) -> PortalResult<CommandOutput> {
    let project = expand_path(path)?;
    let paths = MagicPaths::open()?;
    let plan = run::build_run_plan(&project, &paths)?;
    let exit_code = run::run(&plan, args, json)?;
    if exit_code != 0 {
        return Err(PortalError::Runtime(format!(
            "engine exited with code {exit_code}"
        )));
    }

    let human = format!(
        "Ran {} with Magic Bevy {} in {}\n",
        plan.project.display(),
        plan.engine_version,
        plan.project.display()
    );
    let data = json!({
        "project": plan.project,
        "engine": plan.engine_version,
        "engine_name": plan.engine_name,
        "executable": plan.executable,
        "exit_code": exit_code,
    });

    Ok(CommandOutput { data, human })
}

fn execute_doctor() -> PortalResult<CommandOutput> {
    let paths = MagicPaths::open()?;
    let report = doctor::check(&paths);

    let mut human = String::new();
    human.push_str("Magic Portal environment\n");
    human.push_str(&format!(
        "  root:     {} [{}]\n",
        report.root.display(),
        if report.root_exists { "ok" } else { "missing" }
    ));
    human.push_str(&format!(
        "  registry: {} [{}]\n",
        report.registry_path.display(),
        if report.registry_valid {
            "ok"
        } else {
            "invalid"
        }
    ));
    human.push_str(&format!("  versions: {}\n", report.versions.len()));

    for version in &report.versions {
        human.push_str(&format!(
            "  {} {} at {} [dir: {}, spell: {}, cargo: {}, templates: {}]\n",
            version.version,
            version.name,
            version.path.display(),
            yes_no(version.directory_exists),
            yes_no(version.spell_exists),
            yes_no(version.cargo_config_exists),
            version.template_count
        ));
    }

    if report.issues.is_empty() {
        human.push_str("All checks passed.\n");
    } else {
        human.push('\n');
        for issue in &report.issues {
            human.push_str(&format!("  warning: {issue}\n"));
        }
    }

    let data = json!({
        "ok": report.is_ok(),
        "root": report.root,
        "registry": report.registry_path,
        "registry_valid": report.registry_valid,
        "versions": report.versions,
        "issues": report.issues,
    });

    Ok(CommandOutput { data, human })
}

fn release_line(release: &EngineReleaseEntry) -> String {
    format!(
        "{}  {}  {}  {}  portal {}",
        release.version,
        release.display_name(),
        release.display_description(),
        release.channel,
        release.portal
    )
}

fn release_json(release: &EngineReleaseEntry, older: bool) -> Value {
    json!({
        "version": release.version,
        "name": release.name,
        "description": release.description,
        "channel": release.channel,
        "portal": release.portal,
        "older": older,
    })
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_size(size: Option<u64>) -> String {
    match size {
        Some(bytes) => format!("{bytes} bytes"),
        None => "unknown".to_string(),
    }
}

fn confirm(prompt: &str, yes: bool, json: bool) -> PortalResult<()> {
    if yes {
        return Ok(());
    }
    if json {
        return Err(PortalError::Usage(format!(
            "{prompt} Re-run with --yes when using --json."
        )));
    }
    if !std::io::stdin().is_terminal() {
        return Err(PortalError::Usage(format!(
            "{prompt} Standard input is not a terminal; use --yes."
        )));
    }

    eprint!("{prompt} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|error| PortalError::io(PathBuf::from("<stdin>"), error))?;

    if matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(PortalError::Cancelled)
    }
}

struct CliReporter {
    json: bool,
    current_label: String,
    last_percent: Option<i64>,
}

impl CliReporter {
    fn new(json: bool) -> Self {
        Self {
            json,
            current_label: String::new(),
            last_percent: None,
        }
    }
}

impl Reporter for CliReporter {
    fn report(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Step { message } => {
                eprintln!("[portal] {message}");
            }
            ProgressEvent::Download {
                label,
                received,
                total,
            } => {
                if self.json {
                    return;
                }
                if label != self.current_label {
                    self.current_label = label.clone();
                    self.last_percent = None;
                }

                match total.filter(|total| *total > 0) {
                    Some(total) => {
                        let percent = ((received as u128 * 100) / total as u128) as i64;
                        if self.last_percent != Some(percent) || percent == 100 {
                            self.last_percent = Some(percent);
                            eprint!("\r[portal] {label}: {percent}%");
                            let _ = std::io::stderr().flush();
                        }
                    }
                    None => {
                        if received % (1024 * 1024) == 0 {
                            eprintln!("[portal] {label}: {received} bytes");
                        }
                    }
                }
            }
            ProgressEvent::Log { line } => {
                eprintln!("[portal] {line}");
            }
        }
    }
}
