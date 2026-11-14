# magic-portal

The magical launcher and project manager for the Magic Bevy game engine.

## Status

The core logic and CLI are implemented. The GUI is not implemented yet.

Verified end-to-end against the current published Linux test release: install, replacement install, uninstall, template discovery, project creation, and doctor.

The release publishes OS archives but no separate source asset; the portal uses the GitHub tag archive as the source and flattens its top-level directory into the version root.

## Build

```bash
cargo build --release
```

## CLI

```bash
magic-portal releases --json
magic-portal install 0.1.0 --yes --json
magic-portal installed --json
magic-portal templates --version 0.1.0 --json
magic-portal create --engine 0.1.0 --template empty --name my-game --path ~/projects/my-game --yes --json
magic-portal run --path ~/projects/my-game --json
magic-portal uninstall 0.1.0 --yes --json
magic-portal doctor --json
```

All commands support `--json`. Progress and logs go to stderr. Destructive actions require `--yes` when stdin is not a TTY.

## Checks

```bash
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```