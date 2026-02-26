## 0.0.10 - 2026-02-26

- Added support for installing `sui-node` binary.

## 0.0.9 - 2026-02-26

- Added support for installing binaries from the signers repository.
- Refactored binary metadata handling to use TOML-backed definitions and added all expected TOML fields.
- Strengthened error handling around missing paths and JSON decoding.
- Improved CI stability by retrying tests by default with `nextest`.
- Fixed missing help output when no command is provided.
- Ongoing Windows test and workflow stability fixes.

## 0.0.8 - 2026-01-20

- Made `switch` use `default set` implementation for more consistent behavior and better error messages.
- Improved OS detection by explicitly handling Ubuntu naming.
- CI now runs on pull requests (instead of pushes to `main`).

## 0.0.7 - 2025-12-09

- Added `move-analyzer` installation support.
- Fixed Windows binary path handling to avoid invalid/non-existent paths.
- Additional Windows test fixes.

## 0.0.6 - 2025-12-05

- Migrated to Rust edition 2024.
- Clippy and lint cleanup.

## 0.0.5 - 2025-12-05

- Improved self-update behavior on Windows (archive extraction handling).
- Added generic standalone installer support (including `mvr` path improvements).
- Refactored JSON file handling utilities.
- Added regex cache performance improvements.
- Improved Windows CI/runners and installer-script behavior.

## 0.0.4 - 2025-07-25

- Added `doctor` command for environment diagnostics.
- Added `cleanup` command for cache cleanup.
- Added `switch` support for switching versions, including nightly variants.
- Added Walrus Sites support (`site-builder`).
- Added custom install directory support in install script.
- Improved table-style output and error messaging.

## 0.0.3 - 2025-07-07

- Command-handling refactors.
- Installation workflow improvements.
- `which` output cleanup and general bug fixes.

## 0.0.2 - 2025-06-02

- Added support for `=`, `==`, and `@` version specifiers.
- Added `suiup self` command (`self update`, uninstall flow).
- Enabled tracing (`RUST_LOG=info`/`debug` support).
- Refactored command structure and updated install script.

## 0.0.1 - 2025-05-20

- First release.
