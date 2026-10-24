mod i18n;

use colored::*;
use i18n::{detect_default_lang, save_lang, Lang, Messages};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const QUARANTINE_ATTR: &str = "com.apple.quarantine";
const SERVICE_LABEL: &str = "com.user.uq";
const LOG_PATH: &str = "/tmp/uq.log";
const ERR_LOG_PATH: &str = "/tmp/uq.err";
// Batch bursts of file events (an app copy emits thousands) before sweeping.
const DEBOUNCE: Duration = Duration::from_millis(350);

fn log_paths() -> (PathBuf, PathBuf) {
    (PathBuf::from(LOG_PATH), PathBuf::from(ERR_LOG_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_consistent_service_log_paths() {
        assert_eq!(LOG_PATH, "/tmp/uq.log");
        assert_eq!(ERR_LOG_PATH, "/tmp/uq.err");
    }

    #[test]
    fn collapses_nested_event_paths() {
        let paths = vec![
            PathBuf::from("/Applications/A.app/Contents/MacOS/a"),
            PathBuf::from("/Applications/A.app"),
            PathBuf::from("/Applications/B.app"),
        ];
        assert_eq!(
            collapse_paths(paths),
            vec![
                PathBuf::from("/Applications/A.app"),
                PathBuf::from("/Applications/B.app")
            ]
        );
    }

    #[test]
    fn ignores_partial_downloads_and_hidden_files() {
        assert!(!is_relevant_path(Path::new("/tmp/a.zip.crdownload")));
        assert!(!is_relevant_path(Path::new("/tmp/a.dmg.part")));
        assert!(!is_relevant_path(Path::new("/tmp/.DS_Store")));
        assert!(is_relevant_path(Path::new("/tmp/App.dmg")));
    }

    #[test]
    fn detects_archives() {
        assert!(is_archive(Path::new("/tmp/App.dmg")));
        assert!(is_archive(Path::new("/tmp/Tool.PKG")));
        assert!(!is_archive(Path::new("/tmp/App.app")));
    }
}

fn build_cli(msg: &Messages) -> clap::Command {
    use clap::{arg, value_parser, ArgAction};

    clap::Command::new("uq")
        .alias("unquarantine")
        .version(env!("CARGO_PKG_VERSION"))
        .author("yxxbc")
        .about(msg.about())
        .arg(
            arg!([PATH] ... "paths")
                .help(msg.arg_paths())
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            arg!(-q --quiet)
                .help(msg.arg_quiet())
                .action(ArgAction::SetTrue),
        )
        .arg(
            arg!(-v --verbose)
                .help(msg.arg_verbose())
                .action(ArgAction::SetTrue),
        )
        .arg(
            arg!(-l --lang <LANG>)
                .help(msg.arg_lang())
                .action(ArgAction::Set),
        )
        .subcommand(
            clap::Command::new("do")
                .about(msg.cmd_do())
                .arg(
                    arg!([PATH] ... "paths")
                        .help(msg.arg_do_paths())
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            clap::Command::new("check")
                .about(msg.cmd_check())
                .arg(
                    arg!(<PATH> ... "paths")
                        .help(msg.arg_paths())
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            clap::Command::new("watch")
                .about(msg.cmd_watch())
                .arg(
                    arg!(-p --paths <DIR> ... "dirs")
                        .help(msg.cmd_watch_paths_help())
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            clap::Command::new("service")
                .about(msg.cmd_service())
                .subcommand(
                    clap::Command::new("install")
                        .about(msg.cmd_service_install())
                        .arg(
                            arg!(-z --zero)
                                .help(msg.cmd_service_zero())
                                .action(ArgAction::SetTrue),
                        ),
                )
                .subcommand(clap::Command::new("uninstall").about(msg.cmd_service_uninstall()))
                .subcommand(clap::Command::new("status").about(msg.cmd_service_status())),
        )
        .subcommand(
            clap::Command::new("log")
                .about(msg.cmd_log())
                .alias("logs")
                .arg(
                    arg!(-f --follow)
                        .help(msg.arg_log_follow())
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    arg!(-n --lines <NUM>)
                        .help(msg.arg_log_lines())
                        .value_parser(value_parser!(usize))
                        .default_value("20"),
                )
                .arg(
                    arg!(-c --clear)
                        .help(msg.arg_log_clear())
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            clap::Command::new("lang")
                .about(msg.cmd_lang())
                .arg(arg!([LANG]).help(msg.cmd_lang_target())),
        )
}

fn remove_quarantine(path: &Path) -> Result<bool, std::io::Error> {
    if let Some(_) = xattr::get(path, QUARANTINE_ATTR)? {
        xattr::remove(path, QUARANTINE_ATTR)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn process_path(target: &Path, quiet: bool, verbose: bool, fast: bool, lang: Lang) -> (usize, usize, usize) {
    if !target.exists() {
        if !quiet {
            let err_msg = match lang {
                Lang::Zh => format!("{} 路径不存在: {}", "✗".red().bold(), target.display()),
                Lang::En => format!("{} Path does not exist: {}", "✗".red().bold(), target.display()),
            };
            eprintln!("{}", err_msg);
        }
        return (0, 0, 0);
    }

    let started = Instant::now();
    let (scanned, removed, failed) = if fast {
        sweep_fast(target, verbose && !quiet)
    } else {
        sweep(target, verbose && !quiet)
    };
    if !quiet {
        println!(
            "{}",
            result_line(lang, target, scanned, removed, failed, started.elapsed())
        );
    }
    (scanned, removed, failed)
}

/// Fast sweep for container dirs (~/Downloads, /Applications, mounted images).
/// macOS applies quarantine uniformly to a downloaded item, so an item whose root is
/// clean is skipped instead of walking its whole tree (~99 top-level items instead of 780k).
fn sweep_fast(target: &Path, verbose: bool) -> (usize, usize, usize) {
    if !target.is_dir() {
        return sweep(target, verbose);
    }

    let mut children: Vec<PathBuf> = match fs::read_dir(target) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return sweep(target, verbose),
    };
    children.sort();

    let mut scanned = 0;
    let mut removed = 0;
    let mut failed = 0;
    for child in children {
        let quarantined = matches!(xattr::get(&child, QUARANTINE_ATTR), Ok(Some(_)));
        if quarantined {
            match remove_quarantine(&child) {
                Ok(_) => {
                    let (s, r, f) = sweep(&child, verbose);
                    scanned += s;
                    removed += r + 1;
                    failed += f;
                }
                // App Management/TCC blocks the whole bundle, walking it would only fail again.
                Err(_) => {
                    scanned += 1;
                    failed += 1;
                    if verbose {
                        println!("    {} {} (permission denied)", "✗".red(), child.display());
                    }
                }
            }
        } else if !child.is_dir() {
            let (s, r, f) = sweep(&child, verbose);
            scanned += s;
            removed += r;
            failed += f;
        } else {
            scanned += 1;
        }
    }
    (scanned, removed, failed)
}

/// Strips quarantine from `target` and everything below it.
/// Returns (scanned, removed, failed).
fn sweep(target: &Path, verbose: bool) -> (usize, usize, usize) {
    let mut scanned = 0;
    let mut removed = 0;
    let mut failed = 0;

    for entry in WalkDir::new(target).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        scanned += 1;
        match remove_quarantine(p) {
            Ok(true) => {
                removed += 1;
                if verbose {
                    println!("    {} {}", "✓".green(), p.display());
                }
            }
            Ok(false) => {}
            Err(e) => {
                failed += 1;
                if verbose {
                    println!("    {} {} ({})", "✗".red(), p.display(), e);
                }
            }
        }
    }

    (scanned, removed, failed)
}

fn result_line(
    lang: Lang,
    target: &Path,
    scanned: usize,
    removed: usize,
    failed: usize,
    elapsed: Duration,
) -> String {
    let time = format_elapsed(elapsed);
    let denied = if failed > 0 {
        match lang {
            Lang::Zh => format!(" · 权限不足 {} 项", failed),
            Lang::En => format!(" · {} denied", failed),
        }
    } else {
        String::new()
    };

    if removed > 0 || failed > 0 {
        let detail = if is_archive(target) && scanned == 1 {
            match lang {
                Lang::Zh => format!("归档已解锁 · 安装/挂载后不再传播隔离属性 · {}", time),
                Lang::En => format!("archive unlocked · no quarantine spread on install/mount · {}", time),
            }
        } else {
            match lang {
                Lang::Zh => format!("清除 {} 项 · 扫描 {} 项 · {}{}", removed, scanned, time, denied),
                Lang::En => format!("stripped {} · scanned {} · {}{}", removed, scanned, time, denied),
            }
        };
        if failed > 0 {
            format!("  {} {:<44} {}", "!".yellow().bold(), target.display().to_string(), detail.yellow())
        } else {
            format!("  {} {:<44} {}", "✓".green().bold(), target.display().to_string(), detail.green())
        }
    } else {
        let detail = match lang {
            Lang::Zh => format!("无隔离属性 · 扫描 {} 项 · {}", scanned, time),
            Lang::En => format!("clean · scanned {} · {}", scanned, time),
        };
        format!("  {} {:<44} {}", "·".dimmed(), target.display().to_string(), detail.dimmed())
    }
}

fn permission_hint(lang: Lang, failed: usize) -> String {
    let open_cmd = "open \"x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AppBundles\"";
    match lang {
        Lang::Zh => format!(
            "⚠️  {} 项被 App Management 拦截（sudo 也无效：权限跟随发起进程，也就是你的终端）。\n   \
             1) 系统设置 › 隐私与安全性 › App 管理 → 加入并勾选你的终端（建议同时给「完全磁盘访问权限」），重启终端后再试\n   \
             2) 一键跳转：{}\n   \
             3) Homebrew Cask 安装的应用可从源头避免：export HOMEBREW_CASK_OPTS=\"--no-quarantine\"",
            failed,
            open_cmd.yellow(),
        ),
        Lang::En => format!(
            "⚠️  {} item(s) blocked by App Management (sudo does not help: permission follows the invoking process, your terminal).\n   \
             1) System Settings › Privacy & Security › App Management → add and enable your terminal (grant Full Disk Access too), restart the terminal and retry\n   \
             2) Jump straight there: {}\n   \
             3) For Homebrew Cask, avoid it at the source: export HOMEBREW_CASK_OPTS=\"--no-quarantine\"",
            failed,
            open_cmd.yellow(),
        ),
    }
}

fn format_elapsed(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

fn is_archive(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("dmg") | Some("pkg") | Some("zip") | Some("tar") | Some("gz") | Some("xip")
    )
}

fn handle_do(paths: &[PathBuf], lang: Lang) {
    let use_defaults = paths.is_empty();
    let mut targets = if use_defaults { hot_dirs() } else { paths.to_vec() };
    if use_defaults {
        targets.retain(|p| p.exists());
    }

    println!();
    let title = match lang {
        Lang::Zh => "🧹 uq do · 手动清理隔离属性",
        Lang::En => "🧹 uq do · Manual quarantine cleanup",
    };
    println!("{}", title.cyan().bold());
    if use_defaults {
        let hint = match lang {
            Lang::Zh => "未指定路径，自动清理常用落地目录与已挂载镜像:",
            Lang::En => "No path given, sweeping common locations and mounted images:",
        };
        println!("{}", hint.dimmed());
    }
    println!();

    if targets.is_empty() {
        let msg = match lang {
            Lang::Zh => "没有可清理的目标目录。",
            Lang::En => "Nothing to sweep.",
        };
        println!("{} {}", "=".blue(), msg);
        return;
    }

    let started = Instant::now();
    let mut scanned = 0;
    let mut removed = 0;
    let mut failed = 0;
    for target in &targets {
        let (s, r, f) = process_path(target, false, false, use_defaults, lang);
        scanned += s;
        removed += r;
        failed += f;
    }

    println!("{}", "─".repeat(72).dimmed());
    let summary = match lang {
        Lang::Zh => format!(
            "✅ 清理完成 · 目标 {} 个 · 扫描 {} 项 · 清除 {} 项 · 用时 {}",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
        Lang::En => format!(
            "✅ Done · {} target(s) · scanned {} · stripped {} · {}",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
    };
    if removed > 0 {
        println!("{}", summary.green().bold());
    } else {
        println!("{}", summary.blue().bold());
    }
    if failed > 0 {
        println!("{}", permission_hint(lang, failed));
    }
    println!();
}


fn handle_check(paths: &[PathBuf], lang: Lang) {
    for path in paths {
        if !path.exists() {
            let not_found = match lang {
                Lang::Zh => "文件不存在",
                Lang::En => "not found",
            };
            println!("{} {} ({})", "[-]".red(), path.display(), not_found);
            continue;
        }

        let mut found_count = 0;
        for entry in WalkDir::new(path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if let Ok(Some(val)) = xattr::get(p, QUARANTINE_ATTR) {
                found_count += 1;
                let val_str = String::from_utf8_lossy(&val);
                println!(
                    "{} {}: {}",
                    "[QUARANTINED]".yellow().bold(),
                    p.display(),
                    val_str.trim()
                );
            }
        }

        if found_count == 0 {
            let clean_label = match lang {
                Lang::Zh => "正常无隔离属性",
                Lang::En => "Clean",
            };
            println!("{} {}: {}", "[OK]".green().bold(), clean_label, path.display());
        }
    }
}

/// Real user home; `sudo uq do` must still sweep the invoking user's folders.
fn home_dir() -> Option<PathBuf> {
    if let Ok(user) = std::env::var("SUDO_USER") {
        let p = PathBuf::from("/Users").join(user);
        if p.is_dir() {
            return Some(p);
        }
    }
    dirs::home_dir()
}

fn default_watch_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        for d in [home.join("Downloads"), home.join("Desktop")] {
            if d.is_dir() {
                dirs.push(d);
            }
        }
    }
    for p in ["/Applications", "/Volumes", "/usr/local"] {
        let p = PathBuf::from(p);
        if p.exists() {
            dirs.push(p);
        }
    }
    dirs
}

/// Mount points of currently attached disk images (.dmg/.iso).
fn mounted_image_volumes() -> Vec<PathBuf> {
    let mut vols: Vec<PathBuf> = Vec::new();
    if let Ok(out) = Command::new("hdiutil").arg("info").output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(idx) = line.find("/Volumes/") {
                let mp = PathBuf::from(line[idx..].trim_end());
                if mp.is_dir() && !vols.contains(&mp) {
                    vols.push(mp);
                }
            }
        }
    }
    vols
}

/// Locations `uq do` sweeps when no path is given.
fn hot_dirs() -> Vec<PathBuf> {
    let mut dirs = default_watch_dirs();
    dirs.retain(|d| d != Path::new("/Volumes"));
    dirs.extend(mounted_image_volumes());
    dirs
}

fn is_relevant_path(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.is_empty() || name.starts_with('.') {
        return false;
    }
    !["download", "crdownload", "part", "partial", "tmp"]
        .iter()
        .any(|ext| name.ends_with(&format!(".{}", ext)))
}

/// Drops paths already covered by one of their ancestors (an app copy emits a path per file).
fn collapse_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths.dedup();
    let mut collapsed: Vec<PathBuf> = Vec::new();
    for p in paths {
        if collapsed.iter().any(|kept| p.starts_with(kept)) {
            continue;
        }
        collapsed.push(p);
    }
    collapsed
}

fn handle_watch(custom_paths: &[PathBuf], lang: Lang) {
    let watch_paths = if custom_paths.is_empty() {
        default_watch_dirs()
    } else {
        custom_paths.to_vec()
    };

    let title = match lang {
        Lang::Zh => "=== Unquarantine 实时监听已启动 ===".cyan().bold(),
        Lang::En => "=== Unquarantine Watcher Started ===".cyan().bold(),
    };
    println!("{}", title);
    let prompt = match lang {
        Lang::Zh => "正在监听以下目录:",
        Lang::En => "Monitoring directories:",
    };
    println!("{}", prompt);
    for p in &watch_paths {
        println!("  -> {}", p.display().to_string().yellow());
    }

    // Catch anything that landed while the watcher was not running.
    let mut initial_removed = 0;
    let mut initial_failed = 0;
    for p in &watch_paths {
        if p.exists() {
            let (_, removed, failed) = sweep_fast(p, false);
            initial_removed += removed;
            initial_failed += failed;
        }
    }
    if initial_removed > 0 {
        let msg = match lang {
            Lang::Zh => format!("{} [启动扫描] 补清 {} 项历史隔离属性", "+".green().bold(), initial_removed),
            Lang::En => format!("{} [Startup Sweep] Cleaned {} leftover item(s)", "+".green().bold(), initial_removed),
        };
        println!("{}", msg);
    }
    if initial_failed > 0 {
        println!("{}", permission_hint(lang, initial_failed));
    }

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    )
    .expect("Failed to initialize file watcher");

    for p in &watch_paths {
        if p.exists() {
            let _ = watcher.watch(p, RecursiveMode::Recursive);
        }
    }

    let mut pending: Vec<PathBuf> = Vec::new();
    loop {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(event) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    pending.extend(event.paths.into_iter().filter(|p| is_relevant_path(p)));
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if pending.is_empty() {
                    continue;
                }
                for path in collapse_paths(std::mem::take(&mut pending)) {
                    if !path.exists() {
                        continue;
                    }
                    let (_, removed, failed) = sweep(&path, false);
                    if removed > 0 {
                        let msg = match lang {
                            Lang::Zh => format!(
                                "{} [自动清除] 已为 {} 移除 {} 项隔离属性",
                                "+".green().bold(),
                                path.display(),
                                removed
                            ),
                            Lang::En => format!(
                                "{} [Auto-Clean] Cleared {} quarantine attribute(s) from {}",
                                "+".green().bold(),
                                removed,
                                path.display()
                            ),
                        };
                        println!("{}", msg);
                    }
                    if failed > 0 {
                        println!("{}", permission_hint(lang, failed));
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn get_plist_path() -> PathBuf {
    let home = dirs::home_dir().expect("Cannot determine user home directory");
    home.join("Library/LaunchAgents").join(format!("{}.plist", SERVICE_LABEL))
}

fn get_current_exe() -> PathBuf {
    std::env::current_exe().expect("Failed to determine current executable path")
}

fn handle_service(action_matches: &clap::ArgMatches, lang: Lang) {
    let plist_path = get_plist_path();

    match action_matches.subcommand() {
        Some(("install", sub_m)) => {
            let zero_memory = sub_m.get_flag("zero");
            let exe_path = get_current_exe();
            let parent_dir = plist_path.parent().unwrap();
            fs::create_dir_all(parent_dir).expect("Failed to create LaunchAgents directory");

            let watch_dirs = default_watch_dirs();
            let plist_content = if !zero_memory {
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>watch</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>
"#,
                    SERVICE_LABEL,
                    exe_path.display(),
                    LOG_PATH,
                    ERR_LOG_PATH
                )
            } else {
                let mut dir_strings = Vec::new();
                for d in &watch_dirs {
                    dir_strings.push(format!("        <string>{}</string>", d.display()));
                }
                let tags = dir_strings.join("\n");

                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>-q</string>
{}
    </array>
    <key>WatchPaths</key>
    <array>
{}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>
"#,
                    SERVICE_LABEL,
                    exe_path.display(),
                    tags,
                    tags,
                    LOG_PATH,
                    ERR_LOG_PATH
                )
            };

            fs::write(&plist_path, plist_content).expect("Failed to write plist file");
            let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
            let status = Command::new("launchctl").arg("load").arg(&plist_path).status();

            match status {
                Ok(s) if s.success() => {
                    let success_msg = match lang {
                        Lang::Zh => format!("{} 后台服务安装并注册成功！", "[+]".green().bold()),
                        Lang::En => format!("{} Launchd agent installed successfully!", "[+]".green().bold()),
                    };
                    println!("{}", success_msg);
                    if zero_memory {
                        let m1 = match lang {
                            Lang::Zh => "运行模式: 零内存事件唤醒 (WatchPaths，较慢)",
                            Lang::En => "Mode: Zero-Memory Event Trigger (WatchPaths, slower)",
                        };
                        let m2 = match lang {
                            Lang::Zh => "空闲开销: 0 MB 内存, 0% CPU",
                            Lang::En => "Idle Footprint: 0 MB memory, 0% CPU",
                        };
                        println!("{}", m1);
                        println!("{}", m2);
                    } else {
                        let m1 = match lang {
                            Lang::Zh => "运行模式: FSEvents 实时守护 (毫秒级响应，约 350ms 内完成清理)",
                            Lang::En => "Mode: Realtime FSEvents Daemon (cleanup within ~350ms)",
                        };
                        let m2 = match lang {
                            Lang::Zh => "空闲开销: 约 7 MB 内存, 接近 0% CPU",
                            Lang::En => "Idle Footprint: ~7 MB memory, ~0% CPU",
                        };
                        println!("{}", m1);
                        println!("{}", m2);
                    }
                    println!("Service Plist: {}", plist_path.display());
                    println!("Logs: {}", LOG_PATH);
                }
                _ => {
                    let err_msg = match lang {
                        Lang::Zh => format!("{} 使用 launchctl 加载服务失败。", "[-]".red()),
                        Lang::En => format!("{} Failed to load launchd service with launchctl.", "[-]".red()),
                    };
                    eprintln!("{}", err_msg);
                }
            }
        }
        Some(("uninstall", _)) => {
            if plist_path.exists() {
                let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
                let _ = fs::remove_file(&plist_path);
                let msg = match lang {
                    Lang::Zh => format!("{} 后台服务已停止并卸载完成。", "[+]".green().bold()),
                    Lang::En => format!("{} Background agent stopped and uninstalled.", "[+]".green().bold()),
                };
                println!("{}", msg);
            } else {
                let msg = match lang {
                    Lang::Zh => format!("{} 后台服务尚未安装。", "[=]".blue()),
                    Lang::En => format!("{} Background agent is not installed.", "[=]".blue()),
                };
                println!("{}", msg);
            }
        }
        Some(("status", _)) => {
            let output = Command::new("launchctl")
                .arg("list")
                .output()
                .expect("Failed to execute launchctl");
            let list = String::from_utf8_lossy(&output.stdout);
            if list.contains(SERVICE_LABEL) {
                let is_daemon = fs::read_to_string(&plist_path)
                    .map(|c| c.contains("<key>KeepAlive</key>"))
                    .unwrap_or(false);
                if is_daemon {
                    let msg = match lang {
                        Lang::Zh => format!("{} 服务处于激活状态 (常驻守护模式)", "[RUNNING]".green().bold()),
                        Lang::En => format!("{} Service is ACTIVE (running as daemon)", "[RUNNING]".green().bold()),
                    };
                    println!("{}", msg);
                } else {
                    let msg1 = match lang {
                        Lang::Zh => format!("{} 服务处于就绪状态 (零内存 WatchPaths 事件监听)", "[READY]".green().bold()),
                        Lang::En => format!("{} Service is ACTIVE (Zero-memory WatchPaths event trigger)", "[READY]".green().bold()),
                    };
                    let msg2 = match lang {
                        Lang::Zh => "资源占用: 平时 0 MB 内存，仅在文件变动时唤醒处理",
                        Lang::En => "Footprint: 0 MB memory idle, wakes on file changes",
                    };
                    println!("{}", msg1);
                    println!("{}", msg2);
                }
                println!("Plist: {}", plist_path.display());
                println!("Logs: {}", LOG_PATH);
            } else if plist_path.exists() {
                let msg = match lang {
                    Lang::Zh => format!("{} 服务配置文件存在但未加载", "[STOPPED]".yellow().bold()),
                    Lang::En => format!("{} Service plist exists but is not loaded", "[STOPPED]".yellow().bold()),
                };
                println!("{}", msg);
            } else {
                let msg = match lang {
                    Lang::Zh => format!("{} 服务未安装", "[NOT INSTALLED]".blue()),
                    Lang::En => format!("{} Service is not installed", "[NOT INSTALLED]".blue()),
                };
                println!("{}", msg);
            }
        }
        _ => {}
    }
}

fn handle_log(sub_m: &clap::ArgMatches, lang: Lang) {
    let (log_path, err_path) = log_paths();

    if sub_m.get_flag("clear") {
        let _ = fs::write(&log_path, "");
        let _ = fs::write(&err_path, "");
        let msg = match lang {
            Lang::Zh => format!("{} 后台服务日志已清空。", "[+]".green().bold()),
            Lang::En => format!("{} Background service log files cleared.", "[+]".green().bold()),
        };
        println!("{}", msg);
        return;
    }

    if sub_m.get_flag("follow") {
        if !log_path.exists() {
            let _ = fs::write(&log_path, "");
        }
        let hint = match lang {
            Lang::Zh => "正在实时追踪后台日志（按 Ctrl+C 退出）:",
            Lang::En => "Following background log in real-time (Press Ctrl+C to exit):",
        };
        println!("{}", hint.cyan().bold());
        let _ = Command::new("tail").arg("-f").arg(&log_path).status();
        return;
    }

    let lines_to_show = *sub_m.get_one::<usize>("lines").unwrap_or(&20);

    if !log_path.exists() {
        let msg = match lang {
            Lang::Zh => format!("暂无日志文件 ({} 不存在)。", LOG_PATH),
            Lang::En => format!("No log file found ({} does not exist).", LOG_PATH),
        };
        println!("{} {}", "[=]".blue(), msg);
        return;
    }

    let content = fs::read_to_string(&log_path).unwrap_or_default();
    let all_lines: Vec<&str> = content.lines().collect();

    if all_lines.is_empty() {
        let msg = match lang {
            Lang::Zh => "后台日志内容为空。",
            Lang::En => "Log is empty.",
        };
        println!("{} {}", "[=]".blue(), msg);
        return;
    }

    let start = if all_lines.len() > lines_to_show {
        all_lines.len() - lines_to_show
    } else {
        0
    };

    let header = match lang {
        Lang::Zh => format!("=== 最近 {} 条服务日志 ({}) ===", all_lines.len() - start, LOG_PATH),
        Lang::En => format!("=== Last {} log entries ({}) ===", all_lines.len() - start, LOG_PATH),
    };
    println!("{}", header.cyan().bold());
    for line in &all_lines[start..] {
        println!("{}", line);
    }
}

fn handle_lang_command(target_lang: Option<&String>, current_lang: Lang) {
    if let Some(target) = target_lang {
        if let Some(new_lang) = Lang::from_str(target) {
            if let Err(e) = save_lang(new_lang) {
                eprintln!("{} Failed to save language: {}", "[-]".red(), e);
            } else {
                let msg = match new_lang {
                    Lang::Zh => format!("{} 默认语言已切换为中文 (zh)", "[+]".green().bold()),
                    Lang::En => format!("{} Default language set to English (en)", "[+]".green().bold()),
                };
                println!("{}", msg);
            }
        } else {
            eprintln!("{} Unknown language: '{}'. Supported: zh, en", "[-]".red(), target);
        }
    } else {
        let msg = match current_lang {
            Lang::Zh => format!("当前默认语言: 中文 (zh)"),
            Lang::En => format!("Current default language: English (en)"),
        };
        println!("{}", msg);
    }
}

fn show_dashboard(lang: Lang) {
    let plist_path = get_plist_path();
    let service_installed = plist_path.exists();

    let output = Command::new("launchctl")
        .arg("list")
        .output()
        .ok();
    let launchctl_output = output
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut is_active = false;
    let mut pid_opt: Option<u32> = None;

    for line in launchctl_output.lines() {
        if line.contains(SERVICE_LABEL) {
            is_active = true;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pid_str) = parts.first() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    pid_opt = Some(pid);
                }
            }
            break;
        }
    }

    let is_daemon = if service_installed {
        fs::read_to_string(&plist_path)
            .map(|c| c.contains("<key>KeepAlive</key>"))
            .unwrap_or(false)
    } else {
        false
    };

    let title = format!("⚡️ uq (Unquarantine) v{}", env!("CARGO_PKG_VERSION"));
    let subtitle = match lang {
        Lang::Zh => "极速 macOS 隔离属性自动解除与后台静默守护工具",
        Lang::En => "Lightning-fast macOS quarantine remover & background watcher",
    };

    println!("\n{}", title.cyan().bold());
    println!("{}\n", subtitle.dimmed());

    let (status_text, mem_text, cpu_text, power_text, mode_text) = if !service_installed {
        match lang {
            Lang::Zh => (
                "⚪️ 未安装 (运行 `uq service install` 一键开启)".yellow().to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "未配置".dimmed().to_string(),
            ),
            Lang::En => (
                "⚪️ NOT INSTALLED (Run `uq service install` to enable)".yellow().to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "Unconfigured".dimmed().to_string(),
            ),
        }
    } else if is_active {
        if is_daemon {
            let (mem, cpu) = if let Some(pid) = pid_opt {
                if let Ok(ps_out) = Command::new("ps")
                    .arg("-o")
                    .arg("rss,%cpu")
                    .arg("-p")
                    .arg(pid.to_string())
                    .output()
                {
                    let ps_str = String::from_utf8_lossy(&ps_out.stdout);
                    let mut lines = ps_str.lines().skip(1);
                    if let Some(data) = lines.next() {
                        let parts: Vec<&str> = data.split_whitespace().collect();
                        let rss_kb: f64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let cpu_val: &str = parts.get(1).unwrap_or(&"0.0");
                        (format!("{:.2} MB", rss_kb / 1024.0), format!("{}%", cpu_val))
                    } else {
                        ("~7.00 MB".to_string(), "0.0%".to_string())
                    }
                } else {
                    ("~7.00 MB".to_string(), "0.0%".to_string())
                }
            } else {
                ("~7.00 MB".to_string(), "0.0%".to_string())
            };
            match lang {
                Lang::Zh => (
                    "🟢 运行中 (常驻守护模式)".green().bold().to_string(),
                    mem,
                    cpu,
                    "< 0.05 W (极低功耗)".green().to_string(),
                    "FSEvents 实时守护 (毫秒级触发)".to_string(),
                ),
                Lang::En => (
                    "🟢 RUNNING (Persistent Daemon)".green().bold().to_string(),
                    mem,
                    cpu,
                    "< 0.05 W (Ultra-low)".green().to_string(),
                    "Realtime FSEvents Daemon".to_string(),
                ),
            }
        } else {
            match lang {
                Lang::Zh => (
                    "🟢 激活就绪 (事件触发即时处理)".green().bold().to_string(),
                    "0.00 MB (空闲零常驻)".green().bold().to_string(),
                    "0.0%".green().bold().to_string(),
                    "0.00 W (完全休眠零功耗)".green().bold().to_string(),
                    "零内存事件唤醒 (WatchPaths)".cyan().to_string(),
                ),
                Lang::En => (
                    "🟢 ACTIVE (Ready for file events)".green().bold().to_string(),
                    "0.00 MB (Zero-Memory Idle)".green().bold().to_string(),
                    "0.0%".green().bold().to_string(),
                    "0.00 W (Zero Idle Power)".green().bold().to_string(),
                    "Zero-Memory Event Trigger (WatchPaths)".cyan().to_string(),
                ),
            }
        }
    } else {
        match lang {
            Lang::Zh => (
                "🟡 已配置但未加载".yellow().to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "已停用".dimmed().to_string(),
            ),
            Lang::En => (
                "🟡 Configured but Stopped".yellow().to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "Stopped".dimmed().to_string(),
            ),
        }
    };

    let watch_dirs = default_watch_dirs()
        .iter()
        .map(|d| d.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let size_text = std::env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| format!("{:.0} KB (LTO + Strip)", m.len() as f64 / 1024.0))
        .unwrap_or_else(|| "-".to_string());

    let (label_service, label_mode, label_status, label_dirs, label_logs, label_perf, label_mem, label_cpu, label_power, label_size, label_guide, label_more) = match lang {
        Lang::Zh => (
            "系统服务状态 (Launchd Service)",
            "运行模式",
            "服务状态",
            "监听路径",
            "日志路径",
            "系统性能与资源开销 (Performance)",
            "常驻内存",
            "CPU 占用",
            "估算功耗",
            "程序体积",
            "💡 常用命令指南:",
            "提示: 运行 `uq --help` 或 `uq -h` 可查看全部子命令与参数说明。\n",
        ),
        Lang::En => (
            "Launchd Service Status",
            "Mode",
            "Status",
            "Watch Dirs",
            "Log Path",
            "System Performance & Footprint",
            "Memory Idle",
            "CPU Usage",
            "Est. Power",
            "Binary Size",
            "💡 Quick Command Guide:",
            "Hint: Run `uq --help` or `uq -h` to see all available commands and options.\n",
        ),
    };

    println!("┌─ {} {}", label_service.bold(), "─".repeat(45));
    println!("│  {:<12} {}", format!("{}:", label_mode), mode_text);
    println!("│  {:<12} {}", format!("{}:", label_status), status_text);
    println!("│  {:<12} {}", format!("{}:", label_dirs), watch_dirs.dimmed());
    println!("│  {:<12} {}", format!("{}:", label_logs), LOG_PATH.dimmed());
    println!("├─ {} {}", label_perf.bold(), "─".repeat(40));
    println!("│  {:<12} {}", format!("{}:", label_mem), mem_text);
    println!("│  {:<12} {}", format!("{}:", label_cpu), cpu_text);
    println!("│  {:<12} {}", format!("{}:", label_power), power_text);
    println!("│  {:<12} {}", format!("{}:", label_size), size_text.dimmed());
    println!("└─────────────────────────────────────────────────────────────\n");

    println!("{}", label_guide.bold());
    match lang {
        Lang::Zh => {
            println!("  • 手动清理全部:  {}", "uq do".yellow());
            println!("  • 解除应用隔离:  {}", "uq /Applications/SomeApp.app".yellow());
            println!("  • 检查隔离属性:  {}", "uq check /Applications/SomeApp.app".yellow());
            println!("  • 后台日志追踪:  {}", "uq log -f".yellow());
            println!("  • 服务管理命令:  {}", "uq service [install|uninstall|status]".yellow());
        }
        Lang::En => {
            println!("  • Manual sweep:      {}", "uq do".yellow());
            println!("  • Strip quarantine:  {}", "uq /Applications/SomeApp.app".yellow());
            println!("  • Check quarantine:  {}", "uq check /Applications/SomeApp.app".yellow());
            println!("  • Live log stream:   {}", "uq log -f".yellow());
            println!("  • Service manager:   {}", "uq service [install|uninstall|status]".yellow());
        }
    }
    println!("\n{}", label_more.dimmed());
}

fn extract_cli_lang() -> Option<Lang> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--lang" || args[i] == "-l" {
            if let Some(val) = args.get(i + 1) {
                return Lang::from_str(val);
            }
        } else if let Some(val) = args[i].strip_prefix("--lang=") {
            return Lang::from_str(val);
        } else if let Some(val) = args[i].strip_prefix("-l=") {
            return Lang::from_str(val);
        }
    }
    None
}

fn main() {
    // 1. Determine active language
    let active_lang = extract_cli_lang().unwrap_or_else(detect_default_lang);
    let msg = Messages::new(active_lang);

    // 2. Build and parse dynamic CLI
    let app = build_cli(&msg);
    let matches = app.get_matches();

    let quiet = matches.get_flag("quiet");
    let verbose = matches.get_flag("verbose");

    // 3. Dispatch subcommands
    match matches.subcommand() {
        Some(("do", sub_m)) => {
            let paths: Vec<PathBuf> = sub_m
                .get_many::<PathBuf>("PATH")
                .map(|p| p.cloned().collect())
                .unwrap_or_default();
            handle_do(&paths, active_lang);
        }
        Some(("check", sub_m)) => {
            let paths: Vec<PathBuf> = sub_m.get_many::<PathBuf>("PATH").unwrap().cloned().collect();
            handle_check(&paths, active_lang);
        }
        Some(("watch", sub_m)) => {
            let paths: Vec<PathBuf> = sub_m
                .get_many::<PathBuf>("paths")
                .map(|p| p.cloned().collect())
                .unwrap_or_default();
            handle_watch(&paths, active_lang);
        }
        Some(("service", sub_m)) => {
            handle_service(sub_m, active_lang);
        }
        Some(("log", sub_m)) => {
            handle_log(sub_m, active_lang);
        }
        Some(("lang", sub_m)) => {
            let target = sub_m.get_one::<String>("LANG");
            handle_lang_command(target, active_lang);
        }
        _ => {
            let paths: Vec<PathBuf> = matches
                .get_many::<PathBuf>("PATH")
                .map(|p| p.cloned().collect())
                .unwrap_or_default();

            if paths.is_empty() {
                show_dashboard(active_lang);
                return;
            }

            if quiet {
                for p in &paths {
                    if p.exists() {
                        sweep_fast(p, verbose);
                    }
                }
                return;
            }

            handle_do(&paths, active_lang);
        }
    }
}
