<div align="center">

<img src="./assets/logo.svg" alt="UQ Logo" width="130" height="130" />

# uq (Unquarantine)

**⚡️ 极速、开箱即用的 macOS 隔离属性自动解除与后台静默守护工具**

告别「应用已损坏，无法打开」与「Apple 无法验证恶意软件」烦恼，一键解放你的 Mac 应用。

[![macOS](https://img.shields.io/badge/Platform-macOS%2010.15+-blue?logo=apple&style=flat-square)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202024-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![Homebrew](https://img.shields.io/badge/Install-Homebrew%20Tap-FBB040?logo=homebrew&style=flat-square)](https://brew.sh)
[![License](https://img.shields.io/badge/License-MIT-emerald?style=flat-square)](./LICENSE)
[![Linux.do](https://img.shields.io/badge/Community-Linux.do-2D3748?logo=discourse&style=flat-square)](https://linux.do)

[English](./README_EN.md) · [简体中文](./README.md)

</div>

---

## 💡 为什么需要 uq？

在 macOS 上，从浏览器、微信、Telegram 或第三方网盘下载的应用与工具包，系统都会强制打上 `com.apple.quarantine`（隔离属性）。
当该应用未通过苹果官方公证收费签名时，macOS Gatekeeper 就会弹出：
- *「无法打开，因为 Apple 无法验证其是否包含恶意软件」*
- *「“XXX.app” 已损坏，你应该将它移到废纸篓」*

**`uq` 为此而生**：它是一个采用 Rust 深度调优的极轻量系统工具，既能在终端秒级解除任何 `.app` 的隔离封锁，更能作为后台服务**静默自动运行**——只要下载了新文件，就会被自动解锁，全程 0 内存占用、0 感知。

---

## 🚀 极速一键安装与自启（推荐）

通过 Homebrew 专属 Tap 一键安装并注册开机自启服务：

```bash
brew tap yxxbc/uq https://github.com/yxxbc/uq && brew install uq && uq service install
```

> **搞定！** 执行完毕后，`uq` 就会常驻于系统后台。
> 基于 macOS 原生 `WatchPaths` 机制，平时完全是 **0 MB 内存、0% CPU**。只要 `~/Downloads` 或 `/Applications` 进了新应用，就会在毫秒内自动抹除隔离属性。

---

## ✨ 核心特性

- ⚡️ **单二进制极简极致**：采用 Rust 原生系统调用与 LTO / Strip 极限瘦身，编译体积仅约 **500 KB**。
- 🍃 **零内存空闲常驻**：无需在后台长驻消耗内存的进程，利用系统 `launchd` 事件机制按需唤醒，用完即退。
- 🔍 **状态随时检查**：随时检查目标应用是否被 Gatekeeper 拦截并查看来源元数据。
- 📜 **完整日志可溯源**：支持 `uq log` 实时滚动查看后台解锁历史与追踪。
- 🌐 **双语 i18n 支持**：自带中文与英文双语界面，既能自动匹配系统语言，也可以命令行随意切换保存。

---

## 🛠️ 常用命令速查

`uq` 命名短至两个字符，敲起来顺手流畅：

### 1. 手动清除隔离（直接拖拽应用）

```bash
# 解决单个无法打开的应用
uq /Applications/SomeApp.app

# 同时批量解除多个路径
uq ~/Downloads/*.zip /Applications/*.app

# 详细输出每一个被处理的子文件
uq -v /Applications/SomeApp.app

# 静默处理
uq -q /Applications/SomeApp.app
```

### 2. 检查应用是否被隔离

```bash
uq check /Applications/SomeApp.app
```

### 3. 查看后台服务与日志

```bash
# 查看服务运行状态
uq service status

# 查看最近 20 条自动解锁日志
uq log

# 类似 tail -f 实时追踪日志变动
uq log -f

# 清空历史日志
uq log -c

# 卸载后台服务
uq service uninstall
```

### 4. 国际化与语言设置

```bash
# 查看当前语言设置
uq lang

# 永久切换为英文 / 中文
uq lang en
uq lang zh

# 单次命令临时指定语言
uq -l en --help
```

---

## 📦 手动构建安装

如果你未安装 Homebrew，也可直接通过 Rust 工具链构建：

```bash
git clone https://github.com/yxxbc/uq.git
cd uq
cargo build --release
sudo cp target/release/uq /usr/local/bin/
uq service install
```

---

## 🤝 友情链接

- [LINUX DO 社区](https://linux.do) - *真诚、友善、团结、专业，探讨技术与数字生活的优质社区。*

---

## 📄 开源许可证

本项目基于 [MIT 许可证](./LICENSE) 开源。欢迎 Star 与 PR！
