mod i18n;

use colored::*;
use i18n::{detect_default_lang, save_lang, Lang, Messages};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::Duration;
use walkdir::WalkDir;

const QUARANTINE_ATTR: &str = "com.apple.quarantine";
const SERVICE_LABEL: &str = "com.user.unquarantine";

fn build_cli(msg: &Messages) -> clap::Command {
    use clap::{arg, value_parser, ArgAction};

    clap::Command::new("unquarantine")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Shorin & Miyu")
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
                            arg!(-d --daemon)
                                .help(msg.cmd_service_daemon())
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

fn process_path(target: &Path, quiet: bool, verbose: bool, lang: Lang) -> (usize, usize) {
    let mut total_scanned = 0;
    let mut total_removed = 0;

    if !target.exists() {
        if !quiet {
            let err_msg = match lang {
                Lang::Zh => format!("{} 路径不存在: {}", "[-]".red(), target.display()),
                Lang::En => format!("{} Path does not exist: {}", "[-]".red(), target.display()),
            };
            eprintln!("{}", err_msg);
        }
        return (0, 0);
    }

    for entry in WalkDir::new(target).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        total_scanned += 1;
        match remove_quarantine(p) {
            Ok(true) => {
                total_removed += 1;
                if verbose && !quiet {
                    let tag = match lang {
                        Lang::Zh => "[已清除]",
                        Lang::En => "[STRIPPED]",
                    };
                    println!("  {} {}", tag.green().bold(), p.display());
                }
            }
            Ok(false) => {
                if verbose && !quiet {
                    let tag = match lang {
                        Lang::Zh => "[未检出]",
                        Lang::En => "[CLEAN]",
                    };
                    println!("  {} {}", tag.dimmed(), p.display());
                }
            }
            Err(_) => {}
        }
    }

    if !quiet {
        if total_removed > 0 {
            let msg = match lang {
                Lang::Zh => format!(
                    "{} 已清除 {} 的隔离属性（共处理 {} 项）",
                    "[+]".green().bold(),
                    target.display(),
                    total_removed
                ),
                Lang::En => format!(
                    "{} Removed quarantine from {} ({} item{})",
                    "[+]".green().bold(),
                    target.display(),
                    total_removed,
                    if total_removed > 1 { "s" } else { "" }
                ),
            };
            println!("{}", msg);
        } else {
            let msg = match lang {
                Lang::Zh => format!(
                    "{} {} 未发现隔离属性（已扫描 {} 项）",
                    "[=]".blue(),
                    target.display(),
                    total_scanned
                ),
                Lang::En => format!(
                    "{} No quarantine found on {} (scanned {} items)",
                    "[=]".blue(),
                    target.display(),
                    total_scanned
                ),
            };
            println!("{}", msg);
        }
    }

    (total_scanned, total_removed)
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

fn default_watch_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(download_dir) = dirs::download_dir() {
        dirs.push(download_dir);
    }
    let app_dir = PathBuf::from("/Applications");
    if app_dir.exists() {
        dirs.push(app_dir);
    }
    dirs
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

    loop {
        match rx.recv() {
            Ok(event) => match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        if !path.exists() {
                            continue;
                        }
                        if let Some(ext) = path.extension() {
                            if ext == "download" || ext == "crdownload" || ext == "part" {
                                continue;
                            }
                        }
                        let (_, removed) = process_path(&path, true, false, lang);
                        if removed > 0 {
                            let msg = match lang {
                                Lang::Zh => format!(
                                    "{} [自动清除] 已为 {} 移除 {} 个隔离属性",
                                    "[+]".green().bold(),
                                    path.display(),
                                    removed
                                ),
                                Lang::En => format!(
                                    "{} [Auto-Clean] Cleared {} quarantine attribute(s) from {}",
                                    "[+]".green().bold(),
                                    removed,
                                    path.display()
                                ),
                            };
                            println!("{}", msg);
                        }
                    }
                }
                _ => {}
            },
            Err(e) => {
                eprintln!("Watch error: {}", e);
                break;
            }
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
            let daemon = sub_m.get_flag("daemon");
            let exe_path = get_current_exe();
            let parent_dir = plist_path.parent().unwrap();
            fs::create_dir_all(parent_dir).expect("Failed to create LaunchAgents directory");

            let watch_dirs = default_watch_dirs();
            let plist_content = if daemon {
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
    <string>/tmp/unquarantine.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/unquarantine.err</string>
</dict>
</plist>
"#,
                    SERVICE_LABEL,
                    exe_path.display()
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
    <string>/tmp/unquarantine.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/unquarantine.err</string>
</dict>
</plist>
"#,
                    SERVICE_LABEL,
                    exe_path.display(),
                    tags,
                    tags
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
                    if daemon {
                        let m = match lang {
                            Lang::Zh => "运行模式: 持续常驻守护进程 (watch 模式)",
                            Lang::En => "Mode: Persistent Daemon (watch mode)",
                        };
                        println!("{}", m);
                    } else {
                        let m1 = match lang {
                            Lang::Zh => "运行模式: 零内存事件唤醒 (WatchPaths)",
                            Lang::En => "Mode: Zero-Memory Event Trigger (WatchPaths)",
                        };
                        let m2 = match lang {
                            Lang::Zh => "空闲开销: 0 MB 内存, 0% CPU",
                            Lang::En => "Idle Footprint: 0 MB memory, 0% CPU",
                        };
                        println!("{}", m1);
                        println!("{}", m2);
                    }
                    println!("Service Plist: {}", plist_path.display());
                    println!("Logs: /tmp/unquarantine.log");
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
                println!("Logs: /tmp/unquarantine.log");
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
    let log_path = PathBuf::from("/tmp/unquarantine.log");
    let err_path = PathBuf::from("/tmp/unquarantine.err");

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
            Lang::Zh => "暂无日志文件 (/tmp/unquarantine.log 不存在)。",
            Lang::En => "No log file found (/tmp/unquarantine.log does not exist).",
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
        Lang::Zh => format!("=== 最近 {} 条服务日志 (/tmp/unquarantine.log) ===", all_lines.len() - start),
        Lang::En => format!("=== Last {} log entries (/tmp/unquarantine.log) ===", all_lines.len() - start),
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
        Some(("check", sub_m)) => {
            let paths: Vec<PathBuf> = sub_m.get_many::<PathBuf>("paths").unwrap().cloned().collect();
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
                let err_msg = match active_lang {
                    Lang::Zh => "未指定路径。请运行 `unquarantine --help` 查看使用说明。",
                    Lang::En => "No paths specified. Run `unquarantine --help` for usage.",
                };
                eprintln!("{} {}", "[-]".yellow(), err_msg);
                std::process::exit(1);
            }

            let mut total_s = 0;
            let mut total_r = 0;
            for p in &paths {
                let (s, r) = process_path(p, quiet, verbose, active_lang);
                total_s += s;
                total_r += r;
            }
            if !quiet && paths.len() > 1 {
                let summary = match active_lang {
                    Lang::Zh => format!(
                        "{} 处理完毕。共扫描 {} 项，清除 {} 项的隔离属性。",
                        "[*]".cyan().bold(),
                        total_s,
                        total_r
                    ),
                    Lang::En => format!(
                        "{} Done. Processed {} items total, stripped quarantine from {}.",
                        "[*]".cyan().bold(),
                        total_s,
                        total_r
                    ),
                };
                println!("{}", summary);
            }
        }
    }
}
