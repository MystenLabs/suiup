# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Rust CLI implementation. `src/main.rs` bootstraps the app, while `src/lib.rs` exposes shared modules. Command parsing and routing live in `src/commands/` (e.g., `install`, `update`, `default`), and execution logic is split across `src/handlers/` and `src/component/`. Shared utilities and types are in `src/paths.rs`, `src/fs_utils.rs`, `src/standalone.rs`, and `src/types.rs`.

Integration-style tests are in `tests/` (`integration_test.rs`, `commands_test.rs`, `test_utils.rs`). CI and release automation are in `.github/workflows/`.

## Build, Test, and Development Commands
- `cargo build` : Compile the project in debug mode.
- `cargo build --release` : Build optimized binaries.
- `cargo run -- <args>` : Run the CLI locally (example: `cargo run -- list`).
- `cargo fmt --all -- --check` : Enforce formatting (matches CI).
- `cargo nextest run --no-fail-fast --retries 4` : Run the test suite the same way CI does.
- `cargo test` : Quick local fallback if `nextest` is not installed.

## Coding Style & Naming Conventions
Use Rust 2024 edition defaults and `rustfmt` formatting (4-space indentation, trailing commas where formatter applies). Keep modules focused by command domain (`commands/<command>.rs`, `handlers/<domain>.rs`).

Naming patterns:
- `snake_case` for functions/modules/files.
- `PascalCase` for structs/enums.
- Clear command-oriented names (example: `parse_component_with_version`).

Run `cargo clippy --all-targets --all-features -D warnings` before opening a PR.

## Testing Guidelines
Add or update integration tests under `tests/` for user-visible CLI behavior changes. Prefer descriptive async test names like `test_update_workflow` and validate outputs with `assert_cmd` + `predicates`.

No strict coverage threshold is enforced; prioritize critical flows: install, update, switch/default, cleanup, and platform-specific path handling.

## Commit & Pull Request Guidelines
Recent history favors concise, imperative commits and often Conventional Commit prefixes (`feat:`, `fix:`, `refactor:`), plus release bumps (`Bump to x.y.z`). Use that style consistently.

PRs should include:
- What changed and why.
- Linked issue/PR when applicable.
- Test evidence (command output or CI pass).
- Platform notes for OS-specific behavior (Linux/macOS/Windows).

## Security & Configuration Tips
Use `GITHUB_TOKEN` for authenticated GitHub API access to avoid rate limits during installs/tests. Do not hardcode tokens; pass via environment variables.
