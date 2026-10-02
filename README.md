<div align="center">

<img src="./assets/logo.svg" alt="UQ Logo" width="120" height="120" />

# uq (Unquarantine)

macOS 隔离属性自动解除与后台守护工具

[![macOS](https://img.shields.io/badge/Platform-macOS%2010.15+-blue?logo=apple&style=flat-square)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202024-orange?logo=rust&style=flat-square)](https://www.rust-lang.org)
[![Homebrew](https://img.shields.io/badge/Install-Homebrew%20Tap-FBB040?logo=homebrew&style=flat-square)](https://brew.sh)
[![License](https://img.shields.io/badge/License-MIT-emerald?style=flat-square)](./LICENSE)
[![Linux.do](https://img.shields.io/badge/Community-Linux.do-2D3748?logo=discourse&style=flat-square)](https://linux.do)

[English](./README_EN.md) · [简体中文](./README.md)

</div>

---

## 为什么需要 uq

自动解除网络下载应用携带的 `com.apple.quarantine` 隔离属性，解决「应用已损坏」与「无法验证恶意软件」弹窗，免去每次手动打开终端敲命令的麻烦。

---

## 一键安装与启用

### 方式一：一键快速安装（推荐）

自动识别芯片架构（Apple Silicon / Intel），秒级下载预编译包并自动注册后台服务：

```bash
curl -fsSL https://raw.githubusercontent.com/yxxbc/uq/master/install.sh | bash
```

### 方式二：通过 Homebrew Tap 安装

```bash
brew tap yxxbc/uq https://github.com/yxxbc/uq
brew trust yxxbc/uq 2>/dev/null || true
brew install uq
uq service install
```

也可直接在 [GitHub Releases](https://github.com/yxxbc/uq/releases) 下载对应架构免编译二进制使用。

---

## 方案对比

| 对比维度 | uq (本项目) | 关闭 Gatekeeper (`spctl --master-disable`) | 手动执行 `xattr -cr` |
| :--- | :--- | :--- | :--- |
| **自动化程度** | **完全自动**，检测到新应用落地秒级解除 | **完全放开**，全局不校验 | **纯手动**，每次报错都要进终端处理 |
| **系统安全性** | **高**，仅移除文件的隔离属性，保留系统防护 | **极低**，彻底关闭全局安全防线，任意脚本可静默运行 | **高**，仅针对指定路径生效 |
| **使用门槛** | **零门槛**，一次配置后后台无感运行 | **中**，需要关闭系统安全策略并输入管理员密码 | **高**，需理解终端命令，手动复制文件路径 |
| **长期稳定性** | **稳定**，基于系统 launchd 机制，不受大版本更新影响 | **差**，macOS 系统更新常会自动恢复全局防御 | **稳定**，但每次遇到弹窗都要重复一遍 |
| **安全性防护** | **有**，自带软链接防越界，递归只处理真实文件 | **无**，完全不设防 | **低**，错误使用 `-r` 可能影响符号链接目标 |

---

## 性能损耗与开销

本工具使用 Rust 编写，通过 launchd 系统事件驱动：

- **空闲常驻消耗**：**0 进程 / 0 MB 内存 / 0% CPU**。采用 macOS 原生 `WatchPaths` 机制，平时不运行后台守护进程。
- **触发运行时**：仅在新文件写入完成时由系统短暂唤醒，处理耗时通常在 **10~30 毫秒** 之间，瞬时内存占用约 **7 MB**，处理完毕后进程立即退出并释放全部资源。
- **程序体积**：编译启用 LTO 与 Strip 优化，二进制大小约 **513 KB**。

---

## 常用命令

直接运行 `uq` 会显示当前服务状态与资源监控卡片：

```bash
# 查看服务状态与性能仪表盘
uq

# 手动清除指定应用或文件的隔离属性
uq /Applications/SomeApp.app

# 检查目标是否携带隔离属性及来源信息
uq check /Applications/SomeApp.app

# 实时查看后台自动解除日志
uq log -f

# 查看最近 20 条日志
uq log

# 清空日志记录
uq log -c

# 服务管理
uq service status      # 查看服务状态
uq service uninstall   # 卸载后台服务
uq service install     # 重新安装服务

# 切换语言 (支持中/英双语，默认自动跟随系统)
uq lang zh
uq lang en

# 查看完整命令行帮助
uq --help
```

---

## 手动编译安装

如不使用 Homebrew，可从源码构建：

```bash
git clone https://github.com/yxxbc/uq.git
cd uq
cargo build --release
sudo cp target/release/uq /usr/local/bin/
uq service install
```

---

## 友情链接

- [LINUX DO](https://linux.do) - *真诚、友善、团结、专业，探讨技术与数字生活的优质社区。*

---

## 开源许可证

本项目基于 [MIT 许可证](./LICENSE) 开源。
