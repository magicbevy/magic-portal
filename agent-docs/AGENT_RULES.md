# Agent Rules

## Before Coding

- Read `agent-docs/` before changing code.
- If the request is ambiguous, ask for clarification.
- Do not implement features that were not requested.
- Do not edit `magic-bevy` unless the user explicitly asks.

## Code Rules

- Rust edition 2024.
- CLI is a first-class interface. Every GUI operation has a CLI equivalent.
- CLI commands must work headless and support `--json`.
- No code comments unless the user asks.
- No `unsafe`.
- No `unwrap()` or `expect()` in user-facing paths.
- No `panic!()` for expected failures.
- Return `Result` with actionable errors.
- Keep UI code separate from core logic.
- Do not block the UI thread.
- Do not add large dependencies without telling the user.
- Do not commit secrets, tokens, `target/`, `.magic-bevy/`, or caches.

## Required Checks

Run before declaring work complete:

```bash
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

`cargo check` must pass with no errors and no new warnings.

## User-Facing Behavior

- Every install step needs confirmation and a clear explanation.
- Every uninstall step needs confirmation and must remove only the selected engine version.
- Every destructive action needs confirmation.
- Show download source, destination, scripts, and progress.
- Never silently overwrite or delete user data.
- Never run downloaded scripts without showing the command first.
- Use the exact Magic Bevy theme from `theme.md`; do not invent colors.

## Documentation

Update `agent-docs/` when:

- A module is added or removed.
- A path or command changes.
- A public type changes.
- A workflow changes.
- A rule changes.

## Commit Rules

- Use conventional commits.
- Use one `-m` only.
- Put the subject and full body in the same message.
- Do not use multiple `-m` flags.

Examples:

```bash
git commit -m "feat: add engine version discovery"
git commit -m "fix: validate archive paths before extraction"
```

## Final Checklist

- [ ] `cargo check` passes.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo test` passes.
- [ ] Docs updated if needed.
- [ ] Commit message uses one `-m`.