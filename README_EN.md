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

### Method 1: One-Line Fast Install (Recommended)

Automatically detects architecture (Apple Silicon / Intel), downloads prebuilt binary, and registers the service:

```bash
curl -fsSL https://raw.githubusercontent.com/yxxbc/uq/master/install.sh | bash
```

### Method 2: Via Homebrew Tap

```bash
brew tap yxxbc/uq https://github.com/yxxbc/uq
brew trust yxxbc/uq 2>/dev/null || true
brew install uq
uq service install
```

Both Homebrew and the install script ship **Apple Silicon (arm64)** and **Intel (x86_64)** binaries; prebuilt archives are also on [GitHub Releases](https://github.com/yxxbc/uq/releases).

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

Written in pure Rust, running as a launchd agent driven by realtime FSEvents:

- **Idle Overhead**: **~7 MB memory / ~0% CPU**. No polling, no disk I/O while idle.
- **Trigger Latency**: Detects new files in **milliseconds** and finishes cleanup in ~300 ms (bursts are merged, so copying an app is swept once).
- **Zero-Memory Mode**: `uq service install --zero` switches to launchd `WatchPaths`: **0 processes / 0 MB idle**, at the cost of slower, coarser triggers (whole-directory rescans).
- **Binary Size**: Optimized with LTO and Strip down to **~640 KB**.

---

## Coverage (.app / .dmg / .pkg)

- **`.app`**: Apps dropped into `/Applications` are unquarantined at the moment they land.
- **`.dmg`**: The image file itself is unlocked; `uq do` also cleans contents of currently mounted images, and apps dragged into `/Applications` are covered by the watcher.
- **`.pkg`**: The package is unlocked, and files installed into `/Applications`, `/usr/local`, etc. are watched as well.
- **Permissions**: macOS 13+ App Management blocks plain processes from modifying app bundles in `/Applications`. Such failures are logged; run `sudo uq do` once (or allow your terminal under System Settings › Privacy & Security › App Management).

---

## Commands

Running `uq` without arguments displays service status and performance metrics:

```bash
# Display dashboard and service status
uq

# One-shot manual sweep (no args: ~/Downloads, Desktop, /Applications, /usr/local, mounted images)
uq do

# Clean a specific target (.dmg / .pkg / .app / any file)
uq do ~/Downloads/SomeApp.dmg
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
uq service install     # default: realtime FSEvents daemon
uq service install -z  # zero-memory WatchPaths mode (slower, 0 MB idle)

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
