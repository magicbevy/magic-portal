# Coding Conventions

## Rust Style

- Edition: 2024.
- Format with `cargo fmt`.
- No new warnings.
- No `unsafe`.
- No code comments unless the user explicitly asks.
- Prefer small modules with clear responsibilities.
- Prefer `&str` for read-only string parameters.
- Prefer `Path` / `PathBuf` for filesystem paths.
- Do not use `unwrap()` or `expect()` in user-facing code.
- Do not use `panic!()` for expected failures.
- Return `Result<T, E>` and let the UI display the error.

## Required Checks

Run these before declaring work complete:

```bash
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If a check fails, fix it. Do not silence warnings with broad `#[allow(...)]` unless the user agrees.

## Error Handling

Use one shared error type:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("network error: {0}")]
    Network(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("checksum mismatch for {path}")]
    ChecksumMismatch { path: PathBuf },
    #[error("cancelled")]
    Cancelled,
}
```

Rules:

- Add context to errors.
- Do not leak secrets.
- Do not print raw backtraces to the UI.
- Keep error messages actionable.

## Async and Concurrency

- The UI must stay responsive.
- Downloads, extraction, and spells run in worker threads.
- Send progress through `mpsc`.
- Provide a cancel flag.
- Do not block the UI thread on network or filesystem operations.
- Do not spawn detached threads that outlive the operation.

## Paths

- Engine root: `~/.magic-bevy/`
- Versions: `~/.magic-bevy/versions/<version>/`
- Engine manifest: `~/.magic-bevy/versions/<version>/magic.toml` (a `magic-bevy/magic.toml` layout is also accepted)
- Packages: `~/.magic-bevy/packages/`
- Layouts: `~/.magic-bevy/layouts/`
- Downloads: `~/.magic-bevy/cache/downloads/`
- Staging: `~/.magic-bevy/cache/staging/`
- Registry: `~/.magic-bevy/installed.toml`

Use a single `paths.rs` module for path construction. Do not hardcode paths in UI code.

## TOML and Serialization

- Use `serde` derive for TOML structs.
- Use `Option<T>` for optional fields.
- Add `#[serde(default)]` for fields that may be missing in older files.
- Add `skip_serializing_if = "Option::is_none"` when missing fields should stay missing.
- Do not silently discard unknown fields if future compatibility matters.

## Downloads and Archives

- Use HTTPS.
- Follow redirects only to trusted GitHub hosts.
- Enforce a size limit when possible.
- Validate remote asset names before joining them to a local path.
- Prefer the release asset `digest` field, then a `.sha256`/`.sha256sum` asset.
- Support zip (Windows/source) and tar.gz (Linux/macOS).
- If no source asset is published, use the GitHub tag archive and flatten its single top-level directory.
- Reject archive entries that escape the destination, including tar paths with `..` or absolute paths.
- Extract to staging first.
- Rename into place only after success.

## Scripts

- Show the exact command before running a spell.
- Ask for confirmation.
- Stream output to the UI.
- Treat a non-zero exit code as a failed install.
- Do not run scripts from a partially downloaded file.

## CLI

- Use `clap` 4.5 with derive.
- Every GUI action has a CLI equivalent.
- CLI commands must work headless.
- `--json` prints one JSON envelope to stdout.
- Progress and logs go to stderr.
- Do not prompt in non-interactive mode; require `--yes` for destructive actions.
- Do not duplicate core logic in `cli.rs`.
- Keep JSON field names stable.

## UI Theme

Use the exact Magic Bevy theme:

- `eframe = "0.36"`
- `egui = "0.36"`
- `egui-phosphor = "0.14"`

Put the theme in `src/theme.rs`. Do not invent new colors. Do not use the default egui dark theme. See `theme.md` for the full code and color table.

## UI

- Every action has a visible result.
- Every destructive action has a confirmation.
- Progress is visible and cancellable.
- Errors are readable.
- Do not use blocking dialogs for long operations.

## Tests

Add unit tests for:

- Version parsing.
- Compatibility filtering.
- Version sorting.
- Registry roundtrip.
- Path construction and traversal rejection.
- Zip and tar.gz entry validation and extraction.
- Checksum parsing.
- Template discovery.
- Project name validation and project creation.
- Asset name resolution.

Do not test against the live network. Use fixtures for `versions.toml` and small archive fixtures.

## Dependencies

Keep the dependency list small. Prefer:

- `serde`
- `toml`
- `ureq`
- `zip`
- `tar`
- `flate2`
- `sha2` for checksums
- `semver` for version sorting
- `ctrlc` for CLI cancellation
- `thiserror`
- `dirs`
- `clap` 4.5 for CLI
- `serde_json` 1 for `--json` output
- `eframe` 0.36 / `egui` 0.36 if building the GUI
- `egui-phosphor` 0.14 for icons

Do not add a large framework, async runtime, or download manager without telling the user first.

## Security

- Never commit tokens.
- Never log secrets.
- Do not execute downloaded code without confirmation.
- Do not overwrite user projects without confirmation.
- Do not delete installed versions automatically.
- Do not trust archive paths.
- Do not trust remote TOML blindly; validate every field used.

## Documentation

Update `agent-docs/` when:

- A module is added or removed.
- A path changes.
- A command changes.
- A public type changes.
- A workflow changes.
- A rule changes.