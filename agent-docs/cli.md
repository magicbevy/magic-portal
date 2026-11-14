# CLI Contract

Magic Portal is a GUI application, but the CLI is a first-class interface. The agent must be able to drive the portal from the terminal without a window.

## Principles

- The CLI is implemented from the first working version.
- GUI and CLI share the same `core` logic.
- CLI must work without a TTY.
- `--json` output must be stable and machine-readable.
- Human output is the default when `--json` is not set.
- Progress and logs go to stderr.
- Destructive actions require `--yes` when running non-interactively.
- `--json` never prompts. If a destructive action lacks `--yes`, it fails with a usage error.

## Commands

```text
magic-portal releases [--json] [--include-older]
magic-portal install <version> [--yes] [--json]
magic-portal uninstall <version> [--yes] [--json] [--purge-cache]
magic-portal installed [--json]
magic-portal templates --version <version> [--json]
magic-portal create --engine <version> --template <name> --name <project> --path <path> [--yes] [--json]
magic-portal run [--path <path>] [--json] [-- <engine args>]
magic-portal doctor [--json]
magic-portal --version
magic-portal --help
```

## Command Details

### `releases`

List compatible engine releases.

Options:

- `--json` - machine-readable output.
- `--include-older` - include older compatible releases.

Default output:

```text
0.1.0  nova  Early development build  dev  portal 0.1.0
```

### `install <version>`

Download and install one engine version.

Options:

- `--yes` - confirm without prompting.
- `--json` - machine-readable output.

Behavior:

1. Resolve the version from `versions.toml`.
2. Find the matching GitHub release.
3. Show the release, source/OS asset URLs, destination, spell command, and whether an existing version will be replaced.
4. Ask for confirmation unless `--yes` is set.
5. Download the source archive (release source asset, otherwise the GitHub tag archive) and the OS archive (zip on Windows, tar.gz on Linux/macOS).
6. Verify checksums using the release `digest` field, or a `.sha256`/`.sha256sum` asset when present.
7. Extract both archives into `~/.magic-bevy/cache/staging/<version>/` and flatten a single source root directory.
8. Run the OS spell from the staging directory.
9. Rename staging into `~/.magic-bevy/versions/<version>/` only after success.
10. Register the version in `~/.magic-bevy/installed.toml`.

The source archive may be the GitHub tag archive. It contains one top-level directory (`magic-bevy-<version>/`); the portal flattens it so `portal/`, `magic.toml`, and the engine executable are at the version root.

If `--yes` is missing and stdin is not a TTY, fail with a usage error. Do not hang on a prompt.

If the version is already installed, the confirmation asks whether to replace it. Replacement uses a staging backup and restores the previous version if registration fails.

### `uninstall <version>`

Remove one installed engine version.

Options:

- `--yes` - confirm deletion.
- `--json` - machine-readable output.
- `--purge-cache` - also remove cached downloads for the version.

Rules:

- Remove only `~/.magic-bevy/versions/<version>/`.
- Remove the matching registry entry.
- Never remove projects, packages, layouts, or other versions.
- If the version is not registered but the directory exists, warn and ask before deleting. `--yes` confirms the removal.
- If the registry entry is missing and the directory is also missing, report that there is nothing to remove.
- If deletion fails, keep the registry entry and show the error.
- `--purge-cache` removes matching cached downloads and staging directories, never projects.

### `installed`

List installed engine versions from `~/.magic-bevy/installed.toml`.

### `templates --version <version>`

List templates for one installed engine version.

Read:

```text
~/.magic-bevy/versions/<version>/portal/templates/<template>/template.toml
```

Include:

- template directory name
- `name`
- `description`
- `category` (defaults to `none`)
- absolute icon path when `assets/icon.png` exists
- absolute template path

Invalid `template.toml` files are reported on stderr and skipped.

### `create`

Create a project from an installed engine version and template.

Required options:

- `--engine <version>`
- `--template <name>`
- `--name <project>`
- `--path <path>`

Options:

- `--yes` - confirm creation.
- `--json` - machine-readable output.

Behavior:

1. Validate the engine version is in `installed.toml`.
2. Validate the template exists and has readable metadata.
3. Validate the project name and destination.
4. Read the engine `magic.toml` from `versions/<version>/magic.toml` (a `magic-bevy/magic.toml` layout is also accepted).
5. Copy the entire engine version directory into staging, excluding `magic-bevy`, `magic-bevy.exe`, `portal/`, and `.github/`.
6. Overlay the template into staging, mapping `template/packages.toml` to `packages/packages.toml`.
7. Copy the OS cargo config to `<project>/.cargo/config.toml`.
8. Write `<project>/magic.toml` with the engine version, name, description, channel, and portal version.
9. Rename the staging directory to the destination.
10. Report the project path.

The engine copy is dynamic: every file and directory in the version is copied except the exclusions, so new engine files are included automatically.

The destination must not be inside `~/.magic-bevy/versions`, `packages`, `layouts`, or `cache`. An existing destination must be empty; existing files are never overwritten.

### `run`

Run a project with its engine.

Options:

- `--path <path>` - project directory (default `.`).
- `--json` - machine-readable output.
- `-- <engine args>` - arguments passed to the engine.

Behavior:

1. Read the engine version from `<project>/magic.toml`.
2. Look up that version in `installed.toml`.
3. Start `versions/<version>/magic-bevy` (or `magic-bevy.exe` on Windows).
4. Set the working directory to the project directory before starting the engine.
5. Wait for the engine to exit.

The engine must run with the project directory as its current working directory. It reads `packages/`, `layouts/`, and its build cache from there.

In `--json` mode, engine stdout and stderr are forwarded to stderr so stdout contains only the JSON envelope.

Exit code is `0` when the engine exits successfully, and `1` when the engine exits with a non-zero code or cannot start. The engine exit code is included in the error message.

### `doctor`

Check the portal environment:

- `~/.magic-bevy` exists.
- `installed.toml` exists and is valid.
- Every registered version directory exists.
- The OS spell exists for the current platform.
- The OS cargo config exists for the current platform.
- At least one template is readable for each installed version.

The JSON `ok` field is true only when all checks pass.

## JSON Envelope

Use one stable envelope for all commands:

```json
{
  "ok": true,
  "command": "install",
  "data": {
    "version": "0.1.0",
    "path": "/home/user/.magic-bevy/versions/0.1.0",
    "source_url": "https://api.github.com/repos/magicbevy/magic-bevy/zipball/v0.1.0",
    "binary_url": "https://github.com/magicbevy/magic-bevy/releases/download/v0.1.0/magic-bevy-0.1.0-linux-x86_64.tar.gz",
    "release_url": "https://github.com/magicbevy/magic-bevy/releases/tag/v0.1.0",
    "repository": "https://github.com/magicbevy/magic-bevy",
    "download_count": 2,
    "spell": "bash \"/home/user/.magic-bevy/versions/0.1.0/portal/spells/linux.sh\""
  }
}
```

Run:

```json
{
  "ok": true,
  "command": "run",
  "data": {
    "project": "/home/user/projects/my-game",
    "engine": "0.1.0",
    "engine_name": "nova",
    "executable": "/home/user/.magic-bevy/versions/0.1.0/magic-bevy",
    "exit_code": 0
  }
}
```

Error:

```json
{
  "ok": false,
  "command": "install",
  "error": {
    "kind": "network",
    "message": "failed to fetch versions.toml: connection timed out"
  }
}
```

Stable error kinds: `network`, `io`, `manifest`, `invalid_input`, `usage`, `checksum`, `not_found`, `unsupported`, `install`, `runtime`, `cancelled`.

Rules:

- stdout contains only the JSON envelope when `--json` is set.
- stderr contains progress and diagnostics.
- Do not mix human text into stdout in `--json` mode.
- Do not print secrets or tokens.
- Keep field names stable. Add fields, do not rename them.

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime error |
| 2 | Usage error |
| 3 | Cancelled by user |

## Agent Usage

The agent should use the CLI for all operations:

```bash
magic-portal releases --json
magic-portal install 0.1.0 --yes --json
magic-portal installed --json
magic-portal templates --version 0.1.0 --json
magic-portal create --engine 0.1.0 --template 3d --name my-game --path ~/projects/my-game --yes --json
magic-portal run --path ~/projects/my-game --json
magic-portal uninstall 0.1.0 --yes --json
```

Do not scrape human output. Do not run the GUI for automation.

## Dependencies

```toml
clap = { version = "4.5", features = ["derive"] }
ctrlc = "3"
dirs = "6"
flate2 = "1"
semver = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tar = "0.4"
thiserror = "2"
toml = "0.8"
ureq = "2"
zip = { version = "2", default-features = false, features = ["deflate"] }
```

## Structure

Add:

```text
src/cli.rs
src/core/doctor.rs
src/core/progress.rs
src/core/run.rs
```

`src/main.rs` parses arguments and dispatches:

```rust
fn main() {
    std::process::exit(cli::run());
}
```

Do not duplicate install, uninstall, template, or project logic in `cli.rs`. Call the same `core` functions as the GUI.

## Rules

- Every GUI action must have a CLI equivalent.
- Every CLI command must work headless.
- Never prompt when `--json` is set. If a destructive action lacks `--yes`, fail with a usage error.
- `run` starts the engine with the project directory as cwd; do not copy the engine into the project.
- Keep output stable for the agent.
- Update this file when a command or JSON field changes.