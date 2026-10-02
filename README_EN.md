<div align="center">

<img src="./assets/logo.svg" alt="UQ Logo" width="130" height="130" />

# uq (Unquarantine)

**⚡️ Lightning-fast macOS quarantine remover & zero-memory background watcher**

Say goodbye to "App is damaged and cannot be opened" and "Apple cannot verify this app" errors on macOS.

[![macOS](https://img.shields.io/badge/Platform-macOS%2010.15+-blue?logo=apple&style=flat-square)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202024-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![Homebrew](https://img.shields.io/badge/Install-Homebrew%20Tap-FBB040?logo=homebrew&style=flat-square)](https://brew.sh)
[![License](https://img.shields.io/badge/License-MIT-emerald?style=flat-square)](./LICENSE)
[![Linux.do](https://img.shields.io/badge/Community-Linux.do-2D3748?logo=discourse&style=flat-square)](https://linux.do)

[English](./README_EN.md) · [简体中文](./README.md)

</div>

---

## 🚀 One-Line Installation & Auto-Service (Recommended)

Install via Homebrew Tap and register the zero-memory background service in one go:

```bash
brew tap SHORiN-KiWATA/uq https://github.com/SHORiN-KiWATA/uq && brew install uq && uq service install
```

> **Done!** `uq` will monitor `~/Downloads` and `/Applications` using native macOS `WatchPaths`.
> It consumes **0 MB memory and 0% CPU** while idle, waking up only when new files arrive to instantly strip quarantine attributes.

---

## ✨ Features

- ⚡️ **Lightweight & Fast**: Pure Rust system calls, optimized with LTO and strip down to **~500 KB**.
- 🍃 **Zero Idle Footprint**: Event-driven launchd integration without persistent background memory usage.
- 🔍 **Quarantine Inspection**: Easily check quarantine tags and origin metadata with `uq check`.
- 📜 **Built-in Logging**: Trace auto-clean events easily with `uq log` (supports `-f` follow and `-c` clear).
- 🌐 **Native Bilingual i18n**: Out-of-the-box Chinese and English support with CLI language persistence.

---

## 🛠️ Command Cheat Sheet

```bash
# Strip quarantine attributes manually
uq /Applications/SomeApp.app

# Check quarantine status
uq check /Applications/SomeApp.app

# Service management
uq service status
uq service install
uq service uninstall

# Logs
uq log
uq log -f

# Language switching
uq lang en
uq lang zh
```

---

## 🤝 Friends & Community

- [LINUX DO](https://linux.do) - *A vibrant, friendly and geeky tech community.*

---

## 📄 License

MIT License © Shorin & Miyu
