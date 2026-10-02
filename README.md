# unquarantine

极速且开箱即用的 macOS 隔离属性（`com.apple.quarantine`）清除与后台自动监控工具。

做这个工具是为了解决 macOS 上常见的问题：从网上或者第三方渠道下载的应用、压缩包或二进制文件，经常会被 Gatekeeper 拦截，弹出「无法验证是否包含恶意软件」或「已损坏，无法打开」。通过本工具可以一键清除、检查，甚至常驻后台自动处理。

---

## 特性

- **极速清除**：基于 Rust 原生系统调用实现，多层深目录和 `.app` 包瞬间完成。
- **安全可靠**：严格保留软链接不越界，支持递归遍历。
- **状态检查**：提供 `check` 子命令，随时查看文件或目录是否携带隔离属性及来源信息。
- **自动监听**：内置 `watch` 模式，基于 macOS FSEvents（底层系统事件机制），一旦检测到下载目录或应用程序目录有新文件落盘，立刻自动脱离隔离区。
- **一键后台常驻**：内置 `service install / uninstall / status` 命令，一键配置 macOS 原生 `launchd` 用户服务，开机自启且无感知静默运行。

---

## 安装与编译

### 从源码编译

确保系统已安装 Rust 环境（`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`）：

```bash
cargo build --release
```

编译出的可执行文件位于 `target/release/unquarantine`。你可以将其移动到系统 PATH 路径中：

```bash
sudo cp target/release/unquarantine /usr/local/bin/
```

---

## 使用指南

### 1. 手动清除隔离属性（最常用）

直接传入文件、目录或应用路径：

```bash
# 单个应用
unquarantine /Applications/SomeApp.app

# 多个路径或文件
unquarantine ~/Downloads/tools.zip /Applications/Another.app

# 静默模式（不打印详细扫描条数）
unquarantine -q /Applications/SomeApp.app
```

### 2. 检查隔离属性

查看目标是否存在隔离属性，以及对应的来源 metadata：

```bash
unquarantine check /Applications/SomeApp.app
```

### 3. 实时监听模式

默认会监控 `~/Downloads` 与 `/Applications`：

```bash
# 默认监控下载与应用目录
unquarantine watch

# 自定义监控目录
unquarantine watch -p ~/Downloads -p /opt/homebrew
```

### 4. 注册为开机自启后台服务

通过系统 `launchd` 服务常驻后台：

```bash
# 安装并注册服务（默认使用 macOS WatchPaths 事件触发：平时 0MB 内存占用，仅在有新文件落地时自动唤醒清理）
unquarantine service install

# 如果需要作为长驻守护进程（保持 watch 模式常驻）
unquarantine service install --daemon

# 查看服务状态
unquarantine service status

# 停止并卸载服务
unquarantine service uninstall
```

> 日志默认输出在 `/tmp/unquarantine.log`，方便随时排查。

---

## 许可证

MIT License
