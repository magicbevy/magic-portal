# Magic Portal - Agent Guide

Magic Portal is the launcher and project manager for the Magic Bevy game engine. It is a separate repository from `magic-bevy`; do not edit the engine unless the user explicitly asks.

## Project Facts

- Path: `~/RustProjects/magic-portal`
- Current state: Rust binary with the shared egui theme, `core` install/project logic, and a first-class CLI. The GUI is not implemented yet.
- Package: `magic-portal` v0.1.0
- Edition: 2024
- Current entry point: `src/main.rs` dispatches to `cli::run()`.
- License: MIT

## Read First

Before changing code, read:

- `project-overview.md`
- `architecture.md`
- `workflows.md`
- `coding-conventions.md`
- `cli.md`
- `theme.md`
- `key-files.md`
- `AGENT_RULES.md`

## Agent Rules

- If the request is ambiguous, ask before implementing.
- Do not implement features that were not requested.
- Keep code clean and warning-free.
- The CLI is a first-class interface. Every GUI operation must have a CLI equivalent.
- Every CLI command must work headless and support `--json`.
- Run `cargo check` after every code change.
- Run `cargo fmt --check` and `cargo clippy -- -D warnings` before declaring work done.
- Do not add comments to code unless the user asks.
- Do not use `unwrap()` or `expect()` in user-facing paths.
- Do not silently download, overwrite, or delete user data.
- Do not remove an installed engine version without confirmation.
- Do not remove projects, packages, or layouts when uninstalling an engine.
- Do not run commands that require confirmation without telling the user what will happen.
- Commit with conventional commits.
- Use one `-m` only. Put the subject and full body in the same message.
- Never commit `target/`, `.magic-bevy/`, secrets, or generated caches.

## Implemented

- `core`: paths, registry, version parsing/filtering/sorting, GitHub discovery/download, install/uninstall, templates, project creation, project run, doctor, progress/cancellation.
- `cli`: `releases`, `install`, `uninstall`, `installed`, `templates`, `create`, `run`, `doctor`, `--version`, `--help`.
- Tests for parsing, compatibility, sorting, registry roundtrip, path validation, template discovery, project creation, archive validation, and checksum parsing.
- Verified end-to-end against the current published Linux test release: install, replacement install, uninstall, template discovery, project creation, and doctor.
- Verified `run` starts the engine with the project directory as cwd.

## Not Implemented

- The GUI (`app.rs`) and eframe window.
- Semver-compatible portal matching (current rule is exact `portal == portal.version`).

## Documentation Updates

Update this folder when:

- The crate structure changes.
- A new module or dependency is added.
- The install/version workflow changes.
- A command, path, or public type changes.
- A rule in this guide changes.

## Related Engine Data

Magic Portal consumes data produced by `magic-bevy`:

- `versions.toml` - engine release list
- `magic.toml` - current engine version metadata
- `portal/templates/<template>/template.toml` - project templates
- `portal/config/<os>/cargo.toml` - OS-specific cargo configuration
- `portal/spells/<os>.*` - prerequisite installation scripts

Do not duplicate engine data in this repository. Read it from the installed engine version or from the engine GitHub repository.