# Repository Guidelines

## Project Structure & Module Organization
- `src/` contains the Rust entry point (`main.rs`) and focused modules for config loading, NetworkManager control, D-Bus RPC, logging, and the append-only ledger.
- `tests/` holds integration coverage such as `nm_compliance_test.rs`, which exercises NetworkManager-safe flows.
- `scripts/` provides operational helpers; prefer `install.sh`, `validate_nm_compliance.sh`, and `recover_ovs_bridge.sh` when setting up or debugging agents.
- `docs/`, `DBUS_BLOCKCHAIN.md`, and `RULES.md` document the D-Bus contract, ledger format, and OVS safety policies—keep new features aligned with them.

## Build, Test, and Development Commands
- `cargo fmt` (or `cargo fmt -- --check`) enforces formatting; run before commits.
- `cargo clippy --all-targets --all-features -D warnings` keeps the code free of lints and must pass before opening a PR.
- `cargo build` or `cargo build --release` compiles the agent; the release binary is used by the install scripts under `target/release/`.
- `cargo test` runs unit and integration suites; use `cargo test -- --nocapture` when you need verbose diagnostics.
- `./scripts/install.sh --bridge ovsbr0 --system` is the canonical local install path; pair it with `./scripts/validate_nm_compliance.sh` after changes that touch NetworkManager flows.

## Coding Style & Naming Conventions
- Adopt Rust 2021 idioms with 4-space indentation; keep modules and functions `snake_case`, types `PascalCase`, and constants `SCREAMING_SNAKE_CASE`.
- Use `anyhow::Result` for fallible top-level functions and `thiserror` for domain-specific errors to match existing patterns in `src/`.
- Log through the `log` facade so output propagates to journald; prefer structured context keys when touching `logging.rs`.
- When introducing interface names, obey the sanitizer in `src/naming.rs` and keep them ≤15 characters (e.g., `veth-mycluster-eth0`).

## Testing Guidelines
- Mirror new logic with unit tests in the same module behind `#[cfg(test)]`; exercise cross-module behavior in `tests/` using Tokio runtimes where needed.
- Maintain coverage for D-Bus serialization, ledger hashing, and NetworkManager reconciliation paths; add fixtures under `tests/fixtures/` if complex setups are required.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test` all pass before pushing; document any test skips in the PR body.

## Commit & Pull Request Guidelines
- Follow Conventional Commit prefixes observed in history (`feat:`, `fix:`, `docs:`) and keep messages imperative and ≤72 characters.
- Each PR should describe behavior changes, link related issues, and include the command transcript for `cargo fmt -- --check`, `cargo clippy`, and `cargo test`.
- Attach screenshots or logs when altering D-Bus APIs, scripts, or systemd units, and call out any manual recovery steps in `RECOVERY.md`.

## NetworkManager & OVS Safety
- Never mutate bridges with `ovs-vsctl` or `ip`; instead, rely on the NetworkManager flows outlined in `RULES.md` and helper scripts like `create_ovs_atomic.sh`.
- Record every operational change in the ledger path defined in `config.rs` and confirm compliance with `./scripts/validate_nm_compliance.sh` before deployments.
