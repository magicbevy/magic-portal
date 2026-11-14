# Workflows

Every workflow below is available through the GUI and the CLI. The CLI contract is in `cli.md`; the agent should prefer the CLI with `--json`.

## 1. Version Discovery

### Input

- Current portal version: `0.1.0` (from `Cargo.toml`).
- Engine repository owner/name: configurable; default to the Magic Bevy repository.

### Steps

1. Fetch `versions.toml` from `main`, then `master`.
2. Parse all `[[releases]]`.
3. Keep releases whose `portal` value is compatible with the running portal.
4. Sort releases newest first.
5. Show the first five compatible releases.
6. If more than five exist, show a separate "Older releases" list.
7. Every release row shows:
   - version
   - name
   - description
   - channel
   - portal compatibility
8. Every release row has an install action.

CLI equivalent:

```bash
magic-portal releases --json
magic-portal releases --json --include-older
```

### Compatibility

Current rule:

```text
release.portal == portal.version
```

Later:

```text
same major version and compatible minor/patch
```

Do not label a `dev` release as stable.

### Errors

- Network error: show a retry action.
- Parse error: show the file URL and the TOML error.
- No compatible releases: explain which portal versions are available.

## 2. Install Engine Version

### Confirmation

Before downloading, show:

- Version and display name.
- Description and channel.
- Source repository URL.
- Source archive URL.
- OS executable archive URL.
- Destination path.
- Estimated download count.
- Whether an existing version will be replaced.

The user must confirm before any download starts.

### Steps

1. Resolve the release in `versions.toml`.
2. Fetch the GitHub release list and match the tag or name.
3. Resolve the source archive (release source asset, otherwise the tag archive) and the OS archive.
4. Download both into `~/.magic-bevy/cache/downloads/` (two files).
5. Verify checksums using the release `digest` field, or a `.sha256`/`.sha256sum` asset when present.
6. Extract the source archive and the OS archive into `~/.magic-bevy/cache/staging/<version>/`.
7. If the source archive has one top-level directory, flatten it into staging.
8. Accept zip (Windows/source) and tar.gz (Linux/macOS).
9. Locate the spell inside the staged version:

```text
~/.magic-bevy/cache/staging/<version>/portal/spells/linux.sh
~/.magic-bevy/cache/staging/<version>/portal/spells/macos.sh
~/.magic-bevy/cache/staging/<version>/portal/spells/windows.ps1
```

10. Show the spell command and ask for confirmation.
11. Run the spell with the staged version as the working directory and stream its output.
12. Rename staging into `~/.magic-bevy/versions/<version>/` only after the spell succeeds.
13. Register the version in `~/.magic-bevy/installed.toml`.

CLI equivalent:

```bash
magic-portal install 0.1.0 --yes --json
```

### Atomic Install

- Download to `~/.magic-bevy/cache/downloads/`.
- Extract to `~/.magic-bevy/cache/staging/<version>/`.
- Rename to `~/.magic-bevy/versions/<version>/` only after success.
- If the version already exists, move it to a staging backup first.
- If the spell or registration fails, leave the previous version intact and restore it when possible.
- Never delete a version directory before a successful replacement.

### Cancellation

- Ctrl-C sets the shared cancel flag.
- Cancel stops downloads, extraction, and the spell, and removes staging files.
- Cancel does not remove an existing installed version.

## 3. Uninstall Engine Version

### Confirmation

Before deleting, show:

- Version and display name.
- Channel.
- Destination path.
- Registry entry.
- Whether projects use this version.

The user must confirm.

### Steps

1. Look up the version in `~/.magic-bevy/installed.toml`.
2. Verify the directory exists.
3. Delete only `~/.magic-bevy/versions/<version>/`.
4. Remove the matching `[[versions]]` entry from `installed.toml`.
5. Optionally clear cached downloads and staging for that version.
6. Report success.

CLI equivalent:

```bash
magic-portal uninstall 0.1.0 --yes --json
magic-portal uninstall 0.1.0 --yes --json --purge-cache
```

### Rules

- Do not delete projects.
- Do not delete packages or layouts.
- Do not delete other versions.
- If the version is registered but its directory is missing, remove the registry entry.
- If the version is not registered but its directory exists, warn and ask before deleting.
- If deletion fails, keep the registry entry and show the error.
- `--purge-cache` only removes matching cache files and staging directories.

## 4. Project Creation

### Steps

1. List installed versions from `installed.toml`.
2. Let the user select an engine version.
3. Scan:

```text
~/.magic-bevy/versions/<version>/portal/templates/
```

4. Read each `template.toml`:

```toml
name = "Empty"
description = "A clean starting point with a magical blank canvas."
category = "none"
```

5. Show template name, description, category, and icon.
6. Let the user select a template.
7. Ask for project name and destination.
8. Validate:
   - project name is not empty
   - project name has no path separators or invalid characters
   - destination is not inside `versions/`, `packages/`, `layouts/`, or `cache/`
   - destination does not already contain files
9. Show a summary and ask for confirmation.
10. Copy the entire installed engine version directory into staging, excluding:

```text
magic-bevy        # engine executable
magic-bevy.exe    # Windows engine executable
portal/           # portal data
.github/          # CI metadata
```

The copy is dynamic, not a hardcoded file list, so new engine files are included automatically.

11. Overlay template files:

```text
template/*  ->  <project>/
template/packages.toml  ->  <project>/packages/packages.toml
```

12. Copy OS config:

```text
~/.magic-bevy/versions/<version>/portal/config/<os>/cargo.toml
  ->  <project>/.cargo/config.toml
```

13. The engine manifest is already copied from `~/.magic-bevy/versions/<version>/magic.toml` (a `magic-bevy/magic.toml` layout is also accepted).
14. Write `<project>/magic.toml` with the engine metadata.
15. Ensure `packages/packages.toml` and `layouts/` exist.
16. Rename the staging directory to the destination.
17. Report success with the project path.

CLI equivalent:

```bash
magic-portal create --engine 0.1.0 --template 3d --name my-game --path ~/projects/my-game --yes --json
```

### Template Metadata

The current engine templates are minimal. Some metadata may be inconsistent. Treat `template.toml` as the source of truth and do not infer names from directory names.

## 5. Run Project

### Steps

1. Read the engine version from `<project>/magic.toml`.
2. Look up that version in `~/.magic-bevy/installed.toml`.
3. Resolve `~/.magic-bevy/versions/<version>/magic-bevy` (or `magic-bevy.exe` on Windows).
4. Start the executable with the project directory as the current working directory.
5. Pass any extra arguments after `--` to the engine.
6. Wait for the engine to exit and report its exit code.

CLI equivalent:

```bash
magic-portal run --path ~/projects/my-game --json
magic-portal run --path ~/projects/my-game -- --editor-flag value
```

### Rules

- The engine must run with the project directory as cwd. The engine reads `packages/`, `layouts/`, and build cache from cwd.
- Do not copy the engine executable into the project.
- Do not add the engine to `PATH`.
- In `--json` mode, forward engine stdout/stderr to stderr so stdout stays JSON-only.
- A non-zero engine exit is a runtime error (exit code 1), with the engine code in the message.

## 6. Error Handling

Every error must include:

- What failed.
- Where it failed.
- The URL or path involved.
- A suggested next action.

Do not show raw panic messages. Do not continue after a failed checksum or failed spell unless the user explicitly chooses to continue.

JSON errors use a stable kind: `network`, `io`, `manifest`, `invalid_input`, `usage`, `checksum`, `not_found`, `unsupported`, `install`, or `cancelled`.

If uninstall fails, keep the registry entry and show the path that could not be removed.

## 7. Logging

Log:

- Version fetch URL.
- Selected release and resolved asset URLs.
- Download start/finish.
- Checksum result.
- Extraction destination and archive format.
- Spell command, streamed output, and exit status.
- Staging commit and replacement.
- Cancellation.
- Project creation source and destination.
- Run start, project path, engine path, and exit status.
- Uninstall request, version, path, and result.

Do not log secrets or tokens.