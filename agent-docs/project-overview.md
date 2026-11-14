# Project Overview

## Purpose

Magic Portal is a Unity Hub-like launcher and project manager for Magic Bevy. It should let the user:

1. Discover compatible Magic Bevy releases.
2. Download and install an engine version.
3. Install OS-specific prerequisites.
4. Create a new project from an engine template.

The portal is not the engine. It installs, manages, and launches engine versions; the engine itself lives in `magic-bevy`.

## Current State

The core and CLI are implemented; the GUI is not.

Implemented:

- `src/core/`: path/registry handling, release parsing/filtering/sorting, GitHub discovery/download, install/uninstall, template discovery, project creation, project run, doctor, progress and cancellation.
- `src/cli.rs`: `releases`, `install`, `uninstall`, `installed`, `templates`, `create`, `run`, `doctor`, plus `--json`, `--yes`, `--include-older`, and `--purge-cache`.
- `src/platform/`: Linux, macOS, and Windows path/script/architecture rules.
- `src/theme.rs`: the Magic Bevy egui theme, kept for the upcoming GUI.

Not implemented yet:

- `src/app.rs` and the eframe window.
- Semver-compatible portal matching. The current rule is exact `release.portal == portal.version`.

Verified:

- End-to-end install, replacement install, uninstall, template discovery, project creation, and doctor against the current published Linux test release.
- `run` launches the engine with the project directory as cwd; verified with a fake engine executable that prints its cwd.
- The release publishes OS archives but no separate source asset; the portal falls back to the GitHub tag archive for the source.

The remaining sections describe target behavior. Core/CLI behavior below is implemented unless noted.

## Target Behavior

### 0. CLI and GUI Parity

The CLI is a first-class interface and must be implemented from the first working version.

Requirements:

- Every GUI action has a CLI equivalent.
- Every CLI command works headless.
- `--json` gives stable machine-readable output for agents.
- Destructive CLI actions require `--yes` when non-interactive.
- CLI and GUI share the same `core` logic.

See `cli.md` for the command contract.

### 1. Version Discovery

Read engine releases from the Magic Bevy GitHub repository.

Source of truth:

```text
https://raw.githubusercontent.com/<owner>/magic-bevy/main/versions.toml
```

Fallback:

```text
https://raw.githubusercontent.com/<owner>/magic-bevy/master/versions.toml
```

Expected schema:

```toml
[[releases]]
version = "0.1.0"
name = "nova"
description = "Early development build"
channel = "dev"
portal = "0.1.0"
```

Rules:

- `portal` is the compatibility version for Magic Portal.
- Show releases compatible with the running portal version.
- For now, exact match on `portal` is acceptable. Later use semver-compatible matching.
- Show a short list of recent compatible releases.
- If there are more compatible releases, put older ones in a separate expandable list.
- Every item must remain installable.

### 2. Engine Installation

After the user selects a version and confirms:

1. Download the Magic Bevy source archive (release source asset, otherwise the GitHub tag archive).
2. Detect OS and architecture.
3. Download the OS-specific built executable archive (`tar.gz` on Linux/macOS, `zip` on Windows).
4. Verify checksum when a `.sha256` or `.sha256sum` asset is available.
5. Extract both into `~/.magic-bevy/cache/staging/<version>/`.
6. Rename staging into:

```text
~/.magic-bevy/versions/<version>/
```

7. Run the prerequisite spell for the current OS:

```text
~/.magic-bevy/versions/<version>/portal/spells/linux.sh
~/.magic-bevy/versions/<version>/portal/spells/macos.sh
~/.magic-bevy/versions/<version>/portal/spells/windows.ps1
```

8. Register the installed version in `~/.magic-bevy/installed.toml`.

Do not overwrite an existing version without confirmation.

### 3. Uninstall Engine Version

The user can remove an installed engine version.

Rules:

- Show the version, name, channel, and path before removal.
- Ask for explicit confirmation.
- Remove only:

```text
~/.magic-bevy/versions/<version>/
```

- Remove the matching entry from `~/.magic-bevy/installed.toml`.
- Never remove projects, packages, or layouts.
- Never remove another version.
- If a project references the removed version, warn the user but do not delete the project.
- Optionally offer to clear cached downloads for that version.

### 4. Project Creation

The project creation screen should:

1. List installed engine versions.
2. Let the user select one engine version.
3. Load templates from:

```text
~/.magic-bevy/versions/<version>/portal/templates/<template>/
```

4. Read `template.toml` for name, description, and category.
5. Load `assets/icon.png` when present.
6. Let the user select a template.
7. Ask for project name and destination.
8. Create the project by copying the installed engine version (excluding the executable, `portal/`, and `.github/`) and overlaying the template and environment configuration.

Expected template structure:

```text
template.toml
packages.toml
layouts/def.layout
assets/icon.png
```

Expected project result:

```text
<project>/
  ...all files from the installed engine version...
  magic.toml
  packages/packages.toml
  layouts/
  .cargo/config.toml
  ...template files...
```

The project is a full copy of the installed engine version directory, excluding:

- the engine executable (`magic-bevy` or `magic-bevy.exe`)
- `portal/`
- `.github/`

The copy is dynamic: every file or directory in the engine version is copied except those exclusions, so future engine files are included automatically. The selected template is then overlaid on top of the engine copy (for example `packages/packages.toml` and `layouts/def.layout`).

`.cargo/config.toml` comes from:

```text
~/.magic-bevy/versions/<version>/portal/config/<os>/cargo.toml
```

`magic.toml` should record the selected engine version and portal version.

### 5. Run Project

The user can run a created project.

- Read the engine version from the project's `magic.toml`.
- Look up that version in `installed.toml`.
- Start `~/.magic-bevy/versions/<version>/magic-bevy` (or `magic-bevy.exe` on Windows).
- Set the current working directory to the project directory.
- Do not copy the executable into the project and do not add it to `PATH`.

The engine reads `packages/`, `layouts/`, and its build cache from the current working directory, so running from the project directory is required.

### 6. User Confirmation

Every install, uninstall, and project-creation step must explain:

- What will be downloaded or deleted.
- Where it will be installed or removed.
- Which scripts will run.
- Whether existing data will be replaced or kept.

No step may run silently. Show progress and allow cancellation.

## Storage Layout

```text
~/.magic-bevy/
  versions/
    <version>/
      magic-bevy          # engine executable
      magic.toml
      portal/
        templates/
        config/<os>/cargo.toml
        spells/<os>.*
      src/
      Cargo.toml
      ...
  packages/
  layouts/
  cache/
    downloads/
    staging/
  installed.toml
```

`packages/` and `layouts/` already exist in the user's current installation. Keep them.

The engine manifest is read from `versions/<version>/magic.toml`; a `versions/<version>/magic-bevy/magic.toml` layout is also accepted for archives that keep the engine in a subdirectory.

## Non-Goals

- Do not compile the engine from source during normal installation.
- Do not replace the package manager (`magic_pkg`).
- Do not store project data inside the portal repository.
- Do not depend on the `magic-bevy` workspace as a Cargo path dependency.

## Platform Notes

- Linux: `linux.sh`
- macOS: `macos.sh`
- Windows: `windows.ps1`
- Executable names and archive extensions differ per OS.
- Path handling must be cross-platform.

## Asset Contract

The current release publishes OS archives with these names:

```text
magic-bevy-<version>-linux-x86_64.tar.gz
magic-bevy-<version>-windows-x86_64.zip
magic-bevy-<version>-macos-x86_64.tar.gz
magic-bevy-<version>-macos-aarch64.tar.gz
```

Linux and macOS use `.tar.gz`; Windows uses `.zip`.

The release does not publish a separate source archive. The portal resolves the source in this order:

1. A release asset named `magic-bevy-<version>-source.zip`.
2. The GitHub tag archive (`zipball_url`, falling back to `tarball_url`).

Tag archives contain a single top-level directory such as `magic-bevy-0.1.0/`. The portal flattens that directory into staging so `portal/`, `magic.toml`, and the engine executable end up at the version root.

Checksums are verified in this order:

1. The release asset `digest` field (`sha256:<hex>`).
2. A sibling `<asset>.sha256` or `<asset>.sha256sum` asset.

If the engine later adds a source asset or asset metadata to `versions.toml`, prefer that metadata over name guessing.