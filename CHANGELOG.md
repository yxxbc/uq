# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-10-02

### Added
- Added release automation guardrails for version sync, install flow, and release note generation.
- Added repository agent guidance in `AGENTS.md` to keep release-related changes coordinated.

### Changed
- Switched the Homebrew formula to install prebuilt release archives instead of requiring user-side `cargo install` compilation.
- Automated release version synchronization so `Cargo.toml` and `Formula/uq.rb` stay aligned.
- Hardened the GitHub Actions release workflow to validate tag and artifact naming before packaging.

### Fixed
- Fixed `brew uninstall uq` not clearing the local launchd service by adding an uninstall hook.
- Fixed the dashboard log path mismatch and standardized the runtime log path constants.
- Fixed release artifact naming logic to reject invalid non-tag values during manual workflow runs.

## [0.1.0] - 2026-10-02

### Added
- Initial macOS quarantine stripper and launchd watcher implementation.
- CLI support for direct path stripping, `check`, `watch`, `service`, `log`, and `lang` subcommands.
- Homebrew formula and release workflow scaffolding for package distribution.
- Automatic GitHub release and changelog-driven release note generation.

### Changed
- Standardized release artifact naming and tag validation for GitHub releases.
- Centralized log path configuration to keep service runtime and UI output in sync.
- Improved release automation to use prebuilt binary archives rather than source compilation.

### Fixed
- Fixed incorrect log path display in the dashboard output.
- Fixed release workflow fallback logic so non-tag workflow runs do not produce invalid release names.
- Aligned release notes and changelog automation with the project packaging flow.
