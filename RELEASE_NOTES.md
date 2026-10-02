## uq v0.1.1

This release hardens the packaging and installation flow for uq while keeping the local service lifecycle safe and predictable.

### Highlights
- Prebuilt Homebrew install path instead of forcing users to compile from source
- Automatic version synchronization between Cargo and the Homebrew formula
- Improved release workflow validation for tags, artifact names, and generated release notes
- `brew uninstall uq` now also removes the installed launchd service

### Notes
- The release workflow automatically derives the GitHub release body from `CHANGELOG.md`.
- The formula and release assets are aligned to the same versioned archive layout.
