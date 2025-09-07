# Repository Guidelines

## Project Structure & Module Organization
- Root Rust workspace in `fast/` with crates in `fast/crates/`.
- Core logic: `fast/crates/core/` (library).
- TUI application: `fast/crates/tui/` (binary/UI).
- Build artifacts live in `fast/target/` (ephemeral; do not edit).

## Build, Test, and Development Commands
- Build all crates: `cd fast && cargo build --workspace`.
- Run TUI binary: `cargo run -p tui`.
- Test all crates: `cargo test --workspace`.
- Lint with Clippy: `cargo clippy --workspace --all-targets -- -D warnings`.
- Format code: `cargo fmt --all`.

## Coding Style & Naming Conventions
- Use Rust 4‑space indentation and keep functions small and focused.
- Run `cargo fmt` before committing; CI expects rustfmt style.
- Fix or justify all `cargo clippy` warnings; prefer `-D warnings` locally.
- Naming: crates and modules `snake_case`; types and traits `PascalCase`; functions/vars `snake_case`; constants `UPPER_SNAKE_CASE`.
- Organize modules under `fast/crates/<crate>/src/` using `mod.rs` or inline `mod` per Rust idioms.

## Testing Guidelines
- Place unit tests inline with modules using `#[cfg(test)]`.
- Integration tests go in `fast/crates/<crate>/tests/`, e.g., `tests/tui_smoke_test.rs`.
- Aim for meaningful coverage of core paths; add regression tests for bugs.
- Run: `cargo test --workspace`; target a crate with `-p core` or `-p tui` as needed.

## Commit & Pull Request Guidelines
- Commit messages: imperative mood and scoped when helpful, e.g., `feat(tui): add key bindings` or `fix(core): handle empty input`.
- Keep commits focused and logically grouped; include tests and formatting updates.
- PRs must include: clear description, linked issue (if any), steps to reproduce, and screenshots/recordings for TUI changes.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass before requesting review.

## Security & Configuration Tips
- Never commit secrets; use environment variables or local config.
- Exclude build outputs from changes; avoid modifying `fast/target/` files in PRs.

## Agent-Specific Notes
- This file governs the whole repo; prefer minimal, targeted diffs.
- When editing, follow the structure above and keep commands reproducible.

