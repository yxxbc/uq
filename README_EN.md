<div align="center">

<img src="./assets/logo.svg" alt="UQ Logo" width="120" height="120" />

# uq (Unquarantine)

macOS Quarantine Remover & Background Watcher

[![macOS](https://img.shields.io/badge/Platform-macOS%2010.15+-blue?logo=apple&style=flat-square)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202024-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![Homebrew](https://img.shields.io/badge/Install-Homebrew%20Tap-FBB040?logo=homebrew&style=flat-square)](https://brew.sh)
[![License](https://img.shields.io/badge/License-MIT-emerald?style=flat-square)](./LICENSE)
[![Linux.do](https://img.shields.io/badge/Community-Linux.do-2D3748?logo=discourse&style=flat-square)](https://linux.do)

[English](./README_EN.md) · [简体中文](./README.md)

</div>

---

## Why uq?

Automatically strips the `com.apple.quarantine` attribute from downloaded Mac apps to resolve "App is damaged" and "Cannot verify developer" warnings without manual terminal commands.

---

## One-Line Install & Setup

Install via Homebrew Tap and register the background service:

```bash
brew tap yxxbc/uq https://github.com/yxxbc/uq && brew install uq && uq service install
```

Once installed, new apps placed into `~/Downloads` or `/Applications` are automatically handled without manual intervention.

---

## Comparison

| Approach | uq (This Project) | Disable Gatekeeper (`spctl --master-disable`) | Manual `xattr -cr` |
| :--- | :--- | :--- | :--- |
| **Automation** | **Fully automated**, strips attributes immediately upon file arrival | **Global bypass**, disables checks entirely | **Manual**, requires terminal command on every error |
| **System Security** | **High**, only strips quarantine from targets, keeps system protection intact | **Very Low**, turns off global security safeguards | **High**, affects target path only |
| **Ease of Use** | **Zero effort**, runs silently in background after setup | **Moderate**, requires root password and policy override | **Low**, requires terminal proficiency and copy-pasting paths |
| **Stability** | **Persistent**, based on macOS launchd, survives system upgrades | **Poor**, often re-enabled by macOS major system updates | **Persistent**, but must be manually repeated every time |
| **Symlink Safety** | **Safe**, avoids symlink traversal | **None** | **Risk**, recursive `xattr` can follow symlinks unexpectedly |

---

## Performance & Resource Footprint

Written in pure Rust and driven by macOS launchd system events:

- **Idle Overhead**: **0 Processes / 0 MB Memory / 0% CPU**. Powered by native macOS `WatchPaths`, requiring no persistent daemon in the background.
- **Active Overhead**: Briefly awakened upon file writes. Processing finishes in **10–30 ms** with peak transient memory around **7 MB**, immediately terminating upon completion to free all resources.
- **Binary Size**: Optimized with LTO and Strip down to **~513 KB**.

---

## Commands

Running `uq` without arguments displays service status and performance metrics:

```bash
# Display dashboard and service status
uq

# Manually strip quarantine from a file or application
uq /Applications/SomeApp.app

# Check quarantine attributes and origin metadata
uq check /Applications/SomeApp.app

# Live stream service logs
uq log -f

# View last 20 log entries
uq log

# Clear service logs
uq log -c

# Service management
uq service status
uq service uninstall
uq service install

# Language switching (zh/en, automatically detects system locale by default)
uq lang en
uq lang zh

# View full command help
uq --help
```

---

## Manual Build

```bash
git clone https://github.com/yxxbc/uq.git
cd uq
cargo build --release
sudo cp target/release/uq /usr/local/bin/
uq service install
```

---

## Friends

- [LINUX DO](https://linux.do) - *A sincere, friendly, geeky tech and digital lifestyle community.*

---

## License

MIT License © yxxbc
