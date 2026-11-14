# Key Files

## Current Repository

| File | Purpose |
|---|---|
| `Cargo.toml` | Package manifest and dependencies. |
| `Cargo.lock` | Lockfile. |
| `src/main.rs` | Entry point. Dispatches to `cli::run()`. |
| `src/cli.rs` | Clap commands, JSON envelope, confirmations, headless execution. |
| `src/theme.rs` | Engine theme copied from Magic Bevy. |
| `src/core/mod.rs` | Core module exports and test helpers. |
| `src/core/error.rs` | Shared error type and JSON error kinds. |
| `src/core/paths.rs` | Magic Bevy paths, registry, version validation, time formatting. |
| `src/core/version.rs` | Release/manifest parsing, compatibility, sorting. |
| `src/core/github.rs` | versions.toml, releases, assets, downloads, checksums. |
| `src/core/installer.rs` | Install/uninstall, zip/tar.gz extraction, spells, atomic commit. |
| `src/core/templates.rs` | Template discovery and metadata. |
| `src/core/project.rs` | Project validation, full engine copy (excluding executable, `portal/`, `.github/`), template overlay, project `magic.toml`. |
| `src/core/run.rs` | Resolve and launch a project with its engine. |
| `src/core/doctor.rs` | Environment checks. |
| `src/core/progress.rs` | Progress events, reporter trait, cancellation flag. |
| `src/platform/mod.rs` | OS detection, archive extensions, spell/cargo paths. |
| `src/platform/linux.rs` | Linux constants. |
| `src/platform/macos.rs` | macOS constants. |
| `src/platform/windows.rs` | Windows constants. |
| `agent-docs/cli.md` | CLI command contract. |
| `README.md` | One-line project description. |
| `LICENSE` | MIT license. |
| `.gitignore` | Rust and IDE ignores. |
| `agent-docs/` | Agent documentation. |
| `agent-docs/theme.md` | Exact egui theme copied from Magic Bevy. |

## Source Files

| File | Purpose |
|---|---|
| `src/main.rs` | Start the application. |
| `src/app.rs` | UI state and screens. Not implemented yet. |
| `src/cli.rs` | Clap commands and JSON output. |
| `src/core/mod.rs` | Core module exports and test helpers. |
| `src/core/error.rs` | Shared error type. |
| `src/core/paths.rs` | Resolve `~/.magic-bevy` paths and registry. |
| `src/core/version.rs` | Parse and filter engine releases. |
| `src/core/github.rs` | Fetch manifests and release assets. |
| `src/core/installer.rs` | Download, verify, extract, install, uninstall. |
| `src/core/templates.rs` | Discover templates and metadata. |
| `src/core/project.rs` | Create projects by copying the engine version and overlaying a template. |
| `src/core/run.rs` | Run projects with the engine. |
| `src/core/doctor.rs` | Check the portal environment. |
| `src/core/progress.rs` | Progress and cancellation. |
| `src/platform/mod.rs` | OS detection. |
| `src/platform/linux.rs` | Linux paths, extensions, and spell args. |
| `src/platform/macos.rs` | macOS paths, extensions, and spell args. |
| `src/platform/windows.rs` | Windows paths, extensions, and spell args. |

## External Engine Data

### `magic-bevy/versions.toml`

```toml
[[releases]]
version = "0.1.0"
name = "nova"
description = "Early development build"
channel = "dev"
portal = "0.1.0"
```

### `magic-bevy/magic.toml`

```toml
version = "0.1.0"
name = "nova"
description = "Early development build"
channel = "dev"
portal = "0.1.0"
```

Installed copies are read from `~/.magic-bevy/versions/<version>/magic.toml`. A `~/.magic-bevy/versions/<version>/magic-bevy/magic.toml` layout is also accepted.

### `magic-bevy/portal/templates/<template>/template.toml`

```toml
name = "Empty"
description = "A clean starting point with a magical blank canvas."
category = "none"
```

Template assets:

```text
assets/icon.png
layouts/def.layout
packages.toml
```

### `magic-bevy/portal/config/<os>/cargo.toml`

OS-specific cargo configuration:

```text
linux/cargo.toml
macos/cargo.toml
windows/cargo.toml
```

### `magic-bevy/portal/spells/`

```text
linux.sh
macos.sh
windows.ps1
```

## Local User Paths

```text
~/.magic-bevy/versions/<version>/
~/.magic-bevy/packages/
~/.magic-bevy/layouts/
~/.magic-bevy/cache/downloads/
~/.magic-bevy/cache/staging/
~/.magic-bevy/installed.toml
```

## Important Types

| Type | Location | Purpose |
|---|---|---|
| `EngineReleaseEntry` | `src/core/version.rs` | One engine release. |
| `EngineReleasesFile` | `src/core/version.rs` | `versions.toml` root. |
| `EngineManifest` | `src/core/version.rs` | `magic.toml` metadata. |
| `PortalError` | `src/core/error.rs` | Shared error type. |
| `InstalledVersion` | `src/core/paths.rs` | Installed version record. |
| `InstalledRegistry` | `src/core/paths.rs` | `installed.toml` root. |
| `InstallPlan` | `src/core/installer.rs` | Resolved install request. |
| `UninstallPlan` | `src/core/installer.rs` | Resolved uninstall request. |
| `TemplateInfo` | `src/core/templates.rs` | Template metadata. |
| `CreatePlan` | `src/core/project.rs` | Resolved project creation request. |
| `RunPlan` | `src/core/run.rs` | Resolved project run request. |
| `DoctorReport` | `src/core/doctor.rs` | Environment check result. |
| `ProgressEvent` | `src/core/progress.rs` | UI/CLI progress event. |
| `CancelFlag` | `src/core/progress.rs` | Shared cancellation flag. |
| `Reporter` | `src/core/progress.rs` | Progress sink trait. |
| `EngineTheme` | `src/theme.rs` | UI colors and egui style. |
| `Cli` / `Command` | `src/cli.rs` | Clap command definitions. |

## Commands

```bash
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Commit Style

Use one `-m` only:

```bash
git commit -m "feat: add engine version discovery"
```

Put the subject and full body in the same message. Do not use multiple `-m` flags.