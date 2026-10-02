# AGENTS.md

## Project overview

This repository ships a small Rust CLI named `uq` for macOS. It removes the `com.apple.quarantine` xattr from downloaded files and provides a lightweight background watcher/launchd integration.

## Critical rules for any change

1. Keep the release packaging workflow, Homebrew formula, changelog, and release notes in sync with the current version.
2. Any user-facing feature or installation change must be reflected in both `README.md` and `CHANGELOG.md`.
3. If the behavior of the binary changes, update the release notes and any installation instructions that mention launchd, service state, or release assets.
4. Never ship a release where the Homebrew formula points to a source tarball instead of a prebuilt binary archive when the project already publishes release assets.
5. Always validate the repository before finalizing a release candidate.

## Required validation before release

- `cargo test --quiet`
- `ruby -c Formula/uq.rb`
- YAML parse validation for `.github/workflows/release.yml`
- Ensure `CHANGELOG.md` contains a matching version section and `RELEASE_NOTES.md` is generated from it

## Release / distribution checklist

- Keep `Cargo.toml` version aligned with the release tag.
- Run `python3 scripts/sync_release_version.py` whenever the Cargo version changes so the Homebrew formula and release URL stay aligned automatically.
- Keep `Formula/uq.rb` version and hash aligned with the GitHub release asset.
- Keep the `release.yml` workflow asset naming aligned with the formula and tag naming.
- Add a changelog section for each release under `## [x.y.z]`.
- Ensure the GitHub release body is generated from `CHANGELOG.md` automatically.

## Notes for future agents

- Prefer a single source of truth for versioning and release metadata.
- If a change affects installation, service automation, security behavior, or release packagers, update all related docs and release assets together.
- Do not silently change asset naming or archive layout without updating the formula and release process in the same patch.
