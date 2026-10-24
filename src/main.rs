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
    fn skips_only_bundles_and_volumes_when_clean() {
        assert!(skip_clean_subtree(Path::new("/Applications/Foo.app")));
        assert!(skip_clean_subtree(Path::new("/Volumes/Some Image")));
        assert!(!skip_clean_subtree(Path::new("/usr/local/bin")));
    }

    #[test]
    fn formats_local_timestamp() {
        let ts = timestamp();
        assert_eq!(ts.len(), 19, "unexpected timestamp: {ts}");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[13..14], ":");
    }

    #[test]
    fn sweep_reports_origin_from_the_removed_value() {
        let dir = std::env::temp_dir().join("uq-sweep-origin");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Demo.app/Contents")).unwrap();
        fs::write(dir.join("Demo.app/Contents/bin"), b"x").unwrap();
        fs::write(dir.join("Installer.pkg"), b"x").unwrap();
        for f in ["Demo.app/Contents/bin", "Installer.pkg"] {
            xattr::set(dir.join(f), QUARANTINE_ATTR, b"0083;68a1b2c3;Safari;ABC").unwrap();
        }
        let pkg = sweep(&dir.join("Installer.pkg"), false, true);
        assert_eq!(pkg.removed, 1);
        assert_eq!(pkg.origin.as_deref(), Some("Safari"));
        let bundle = sweep(&dir.join("Demo.app"), false, true);
        assert_eq!(bundle.removed, 1);
        assert_eq!(bundle.origin.as_deref(), Some("Safari"));
        let fast = sweep_fast(&dir, false, true);
        assert_eq!(fast.removed, 2);
        assert_eq!(fast.origin.as_deref(), Some("Safari"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_download_agent_from_quarantine_value() {
        assert_eq!(
            agent_from_value("01c1;6a963d68;Homebrew Cask;911E1CFD-A9BE-48BA-89FE-2302F0866214")
                .as_deref(),
            Some("Homebrew Cask")
        );
        assert_eq!(agent_from_value("0083;68a1b2c3;Safari;ABC").as_deref(), Some("Safari"));
        assert_eq!(agent_from_value("0083;68a1b2c3;;ABC"), None);
        assert_eq!(agent_from_value("broken"), None);
    }

    #[test]
    fn labels_bundles_by_their_app_name() {
        assert_eq!(app_label(Path::new("/Applications/Foo.app")), "Foo.app");
        assert_eq!(
            app_label(Path::new("/Applications/Foo.app/Contents/MacOS/foo")),
            "Foo.app"
        );
        assert_eq!(app_label(Path::new("/Users/x/Downloads/App.dmg")), "App.dmg");
        assert_eq!(app_label(Path::new("/tmp/plain.txt")), "plain.txt");
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
                .action(ArgAction::SetTrue)
                .global(true),
        )
        .arg(
            arg!(-v --verbose)
                .help(msg.arg_verbose())
                .action(ArgAction::SetTrue)
                .global(true),
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
                )
                .arg(
                    arg!(-n --"dry-run")
                        .help(msg.arg_dry_run())
                        .action(ArgAction::SetTrue),
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
        .after_help(msg.after_help())
}

#[derive(Default)]
struct SweepStats {
    scanned: usize,
    removed: usize,
    failed: usize,
    /// Download agent of the first quarantined item ("Safari", "Homebrew Cask", ...)
    origin: Option<String>,
}

impl std::ops::AddAssign for SweepStats {
    fn add_assign(&mut self, other: Self) {
        self.scanned += other.scanned;
        self.removed += other.removed;
        self.failed += other.failed;
        if self.origin.is_none() {
            self.origin = other.origin;
        }
    }
}

/// Strips `com.apple.quarantine` from `path` and returns the raw value it had.
fn strip_if_quarantined(path: &Path, dry_run: bool) -> Result<Option<String>, std::io::Error> {
    match xattr::get(path, QUARANTINE_ATTR)? {
        Some(value) => {
            if !dry_run {
                xattr::remove(path, QUARANTINE_ATTR)?;
            }
            Ok(Some(String::from_utf8_lossy(&value).to_string()))
        }
        None => Ok(None),
    }
}

/// Download agent recorded in a quarantine value: `flags;timestamp;agent;uuid`.
fn agent_from_value(raw: &str) -> Option<String> {
    let agent = raw.split(';').nth(2)?.trim();
    if agent.is_empty() {
        None
    } else {
        Some(agent.to_string())
    }
}

fn process_path(
    target: &Path,
    quiet: bool,
    verbose: bool,
    fast: bool,
    lang: Lang,
    dry_run: bool,
) -> SweepStats {
    if !target.exists() {
        if !quiet {
            let err_msg = match lang {
                Lang::Zh => format!("{} 路径不存在: {}", "✗".red().bold(), target.display()),
                Lang::En => format!("{} Path does not exist: {}", "✗".red().bold(), target.display()),
            };
            eprintln!("{}", err_msg);
        }
        return SweepStats::default();
    }

    let started = Instant::now();
    let stats = if fast {
        sweep_fast(target, verbose && !quiet, dry_run)
    } else {
        sweep(target, verbose && !quiet, dry_run)
    };
    if !quiet {
        println!(
            "{}",
            result_line(
                lang,
                target,
                stats.scanned,
                stats.removed,
                stats.failed,
                dry_run,
                started.elapsed()
            )
        );
    }
    stats
}

/// Fast sweep for container dirs (~/Downloads, /Applications, mounted images).
/// macOS applies quarantine uniformly to a downloaded item, so a bundle whose root is
/// clean is skipped instead of walking its whole tree (~99 top-level items instead of 780k).
fn sweep_fast(target: &Path, verbose: bool, dry_run: bool) -> SweepStats {
    if !target.is_dir() {
        return sweep(target, verbose, dry_run);
    }

    let mut children: Vec<PathBuf> = match fs::read_dir(target) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return sweep(target, verbose, dry_run),
    };
    children.sort();

    // pkg payloads (bin/lib trees under /usr/local, /opt, /Library) can hold
    // quarantined files without a quarantined parent directory.
    let probe_payloads = ["/usr/local", "/opt", "/Library"]
        .iter()
        .any(|p| target.starts_with(p));

    let mut stats = SweepStats::default();
    for child in children {
        match strip_if_quarantined(&child, dry_run) {
            Ok(Some(value)) => {
                stats.scanned += 1;
                stats.removed += 1;
                if stats.origin.is_none() {
                    stats.origin = agent_from_value(&value);
                }
                if verbose {
                    println!("    {} {}", "✓".green(), child.display());
                }
                stats += sweep(&child, verbose, dry_run);
            }
            Ok(None) if !child.is_dir() => stats += sweep(&child, verbose, dry_run),
            Ok(None) if !probe_payloads || skip_clean_subtree(&child) => stats.scanned += 1,
            Ok(None) => stats += sweep(&child, verbose, dry_run),
            Err(_) => {
                // App Management/TCC blocks the whole bundle, walking it would only fail again.
                stats.scanned += 1;
                stats.failed += 1;
                if verbose {
                    println!("    {} {} (permission denied)", "✗".red(), child.display());
                }
            }
        }
    }
    stats
}

/// Only bundles and mounted volumes are assumed to be quarantined uniformly,
/// so a clean one can be skipped instead of walking its whole tree.
fn skip_clean_subtree(path: &Path) -> bool {
    let is_bundle = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("app"))
        .unwrap_or(false);
    let is_volume = path.parent() == Some(Path::new("/Volumes"));
    is_bundle || is_volume
}

/// Strips quarantine from `target` and everything below it.
fn sweep(target: &Path, verbose: bool, dry_run: bool) -> SweepStats {
    let mut stats = SweepStats::default();

    for entry in WalkDir::new(target).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        stats.scanned += 1;
        match strip_if_quarantined(p, dry_run) {
            Ok(Some(value)) => {
                stats.removed += 1;
                if stats.origin.is_none() {
                    stats.origin = agent_from_value(&value);
                }
                if verbose {
                    println!("    {} {}", "✓".green(), p.display());
                }
            }
            Ok(None) => {}
            Err(e) => {
                stats.failed += 1;
                if verbose {
                    println!("    {} {} ({})", "✗".red(), p.display(), e);
                }
            }
        }
    }

    stats
}

fn result_line(
    lang: Lang,
    target: &Path,
    scanned: usize,
    removed: usize,
    failed: usize,
    dry_run: bool,
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
            match (lang, dry_run) {
                (Lang::Zh, true) => format!("可解除 {} 项 · 扫描 {} 项 · {}{}", removed, scanned, time, denied),
                (Lang::Zh, false) => format!("清除 {} 项 · 扫描 {} 项 · {}{}", removed, scanned, time, denied),
                (Lang::En, true) => format!("would strip {} · scanned {} · {}{}", removed, scanned, time, denied),
                (Lang::En, false) => format!("stripped {} · scanned {} · {}{}", removed, scanned, time, denied),
            }
        };
        let path_cell = pad_end(&truncate(&target.display().to_string(), 44), 44);
        if failed > 0 {
            format!("  {} {} {}", "!".yellow().bold(), path_cell, detail.yellow())
        } else {
            format!("  {} {} {}", "✓".green().bold(), path_cell, detail.green())
        }
    } else {
        let detail = match lang {
            Lang::Zh => format!("无隔离属性 · 扫描 {} 项 · {}", scanned, time),
            Lang::En => format!("clean · scanned {} · {}", scanned, time),
        };
        let path_cell = pad_end(&truncate(&target.display().to_string(), 44), 44);
        format!("  {} {} {}", "·".dimmed(), path_cell, detail.dimmed())
    }
}

fn permission_hint(lang: Lang, failed: usize) -> String {
    let open_cmd = "open \"x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AppBundles\"";
    match lang {
        Lang::Zh => format!(
            "{} 项被 App Management 拦截（sudo 也无效：权限跟随发起进程，也就是你的终端）。\n   \
             1) 系统设置 › 隐私与安全性 › App 管理 → 加入并勾选你的终端（建议同时给「完全磁盘访问权限」），重启终端后再试\n   \
             2) 一键跳转：{}\n   \
             3) Homebrew Cask 安装的应用可从源头避免：export HOMEBREW_CASK_OPTS=\"--no-quarantine\"",
            failed,
            open_cmd.yellow(),
        ),
        Lang::En => format!(
            "{} item(s) blocked by App Management (sudo does not help: permission follows the invoking process, your terminal).\n   \
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

// ---------- 输出样式（对齐中文宽度 + nixsave 风格日志） ----------

const BOX_INNER: usize = 62;

fn char_width(c: char) -> usize {
    let cp = c as u32;
    if matches!(
        cp,
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x2600..=0x27BF
            | 0x2B00..=0x2BFF
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x3FFFD
    ) {
        2
    } else {
        1
    }
}

fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn pad_end(s: &str, width: usize) -> String {
    format!("{}{}", s, " ".repeat(width.saturating_sub(display_width(s))))
}

fn truncate(s: &str, width: usize) -> String {
    if display_width(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if display_width(&out) + char_width(c) + 1 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

fn truncate_words(s: &str, width: usize) -> String {
    if display_width(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    for word in s.split_whitespace() {
        let candidate = if out.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", out, word)
        };
        if display_width(&candidate) + 2 > width {
            break;
        }
        out = candidate;
    }
    format!("{} …", out)
}

fn box_top(title: &str, colored: String) -> String {
    let dashes = BOX_INNER.saturating_sub(3 + display_width(title));
    format!("  ╭─ {} {}╮", colored, "─".repeat(dashes))
}

fn box_sep(title: &str, colored: String) -> String {
    let dashes = BOX_INNER.saturating_sub(3 + display_width(title));
    format!("  ├─ {} {}┤", colored, "─".repeat(dashes))
}

fn box_bottom() -> String {
    format!("  ╰{}╯", "─".repeat(BOX_INNER))
}

fn box_row(label: &str, value: &str, colored: String) -> String {
    let pad = BOX_INNER.saturating_sub(7 + display_width(value));
    format!(
        "  │ {} {}{}│",
        pad_end(label, 6).dimmed(),
        colored,
        " ".repeat(pad)
    )
}

fn log_intro(text: String) {
    println!("{}", text.dimmed());
}

fn log_step(idx: usize, total: usize, title: &str, detail: Option<String>) {
    println!(
        "\n{} {}",
        "[执行]".cyan(),
        format!("{}/{} {}", idx, total, title).bold()
    );
    if let Some(d) = detail {
        println!("  {}", d.dimmed());
    }
}

fn log_action(title: &str, detail: Option<String>) {
    println!("\n{} {}", "[执行]".cyan(), title.bold());
    if let Some(d) = detail {
        println!("  {}", d.dimmed());
    }
}

fn log_done(text: String) {
    println!("\n{} {}", "[完成]".green().bold(), text.bold());
}

fn log_warn(text: String) {
    println!("{} {}", "[警告]".yellow(), text);
}

fn log_fail(text: String) {
    println!("{} {}", "[失败]".red(), text);
}

fn log_hint(text: String) {
    println!("  {}", text.dimmed());
}

/// Local wall-clock timestamp for log lines: 2026-10-24 20:31:07
fn timestamp() -> String {
    let mut buf = [0i8; 32];
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return String::new();
        }
        libc::strftime(
            buf.as_mut_ptr(),
            buf.len(),
            c"%Y-%m-%d %H:%M:%S".as_ptr(),
            &tm,
        );
        std::ffi::CStr::from_ptr(buf.as_ptr())
            .to_string_lossy()
            .to_string()
    }
}

/// Outermost bundle (.app/.pkg/.dmg/.xip) containing `path`, if any.
fn bundle_root(path: &Path) -> Option<&Path> {
    let mut found = None;
    for ancestor in path.ancestors() {
        if let Some(ext) = ancestor.extension() {
            let ext = ext.to_string_lossy();
            if [".app", ".pkg", ".dmg", ".xip"]
                .iter()
                .any(|e| ext.eq_ignore_ascii_case(&e[1..]))
            {
                found = Some(ancestor);
            }
        }
    }
    found
}

/// Human name of the software being cleaned (bundle name, file name or path).
fn app_label(path: &Path) -> String {
    bundle_root(path)
        .and_then(|p| p.file_name())
        .or_else(|| path.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// One log record per cleanup: when · what software · how many · download source · where.
fn log_event(lang: Lang, target: &Path, removed: usize, origin: Option<String>) {
    let count = match lang {
        Lang::Zh => format!("解除 {} 项", removed),
        Lang::En => format!("stripped {}", removed),
    };
    let mut parts = vec![count];
    if let Some(agent) = origin {
        parts.push(match lang {
            Lang::Zh => format!("来源 {}", agent),
            Lang::En => format!("from {}", agent),
        });
    }
    parts.push(target.display().to_string());
    println!(
        "{} {} {} · {}",
        timestamp().dimmed(),
        "[完成]".green().bold(),
        app_label(target).bold(),
        parts.join(" · ")
    );
}

/// Same as `log_event` but for a warning (permission failures etc.).
fn log_warn_event(lang: Lang, target: &Path, failed: usize) {
    println!(
        "{} {} {}",
        timestamp().dimmed(),
        "[警告]".yellow(),
        permission_hint(lang, failed)
    );
    let _ = target;
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

fn handle_do(paths: &[PathBuf], lang: Lang, verbose: bool, dry_run: bool) {
    let use_defaults = paths.is_empty();
    let mut targets = if use_defaults { hot_dirs() } else { paths.to_vec() };
    if use_defaults {
        targets.retain(|p| p.exists());
    }

    log_intro(
        match lang {
            Lang::Zh => if dry_run {
                "uq：预演模式，只列出将被解除隔离属性的目录，不会修改任何文件（去掉 -n 才会真正执行）"
            } else {
                "uq：手动解除隔离属性（忘了用法就运行 uq help）"
            },
            Lang::En => if dry_run {
                "uq: dry run, lists what would be unquarantined and changes nothing (drop -n to apply)"
            } else {
                "uq: strip quarantine manually (run uq help if unsure)"
            },
        }
        .to_string(),
    );

    if targets.is_empty() {
        log_warn(match lang {
            Lang::Zh => "没有可清理的目标目录。".to_string(),
            Lang::En => "Nothing to sweep.".to_string(),
        });
        return;
    }

    let target_list = targets
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    log_action(
        match (lang, dry_run) {
            (Lang::Zh, true) => "扫描（预演，不修改）",
            (Lang::Zh, false) => "扫描并解除隔离属性",
            (Lang::En, true) => "Scan (dry run, no changes)",
            (Lang::En, false) => "Scan and strip quarantine",
        },
        Some(truncate(&target_list, 100)),
    );

    let started = Instant::now();
    let mut totals = SweepStats::default();
    for target in &targets {
        totals += process_path(target, false, verbose, use_defaults, lang, dry_run);
    }
    let (scanned, removed, failed) = (totals.scanned, totals.removed, totals.failed);

    let summary = match (lang, dry_run) {
        (Lang::Zh, true) => format!(
            "{} 个目标 · 扫描 {} 项 · 将解除 {} 项 · 用时 {}（未修改）",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
        (Lang::Zh, false) => format!(
            "{} 个目标 · 扫描 {} 项 · 解除 {} 项 · 用时 {}",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
        (Lang::En, true) => format!(
            "{} target(s) · scanned {} · would strip {} · {} (nothing changed)",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
        (Lang::En, false) => format!(
            "{} target(s) · scanned {} · stripped {} · {}",
            targets.len(),
            scanned,
            removed,
            format_elapsed(started.elapsed())
        ),
    };
    log_done(summary);

    if failed > 0 {
        log_warn(permission_hint(lang, failed));
        log_hint(match lang {
            Lang::Zh => "完整说明见 uq help 的「权限」一节".to_string(),
            Lang::En => "See the Permissions section in uq help".to_string(),
        });
    }
}

fn handle_check(paths: &[PathBuf], lang: Lang) {
    log_intro(
        match lang {
            Lang::Zh => "uq：检查隔离属性与来源（忘了用法就运行 uq help）",
            Lang::En => "uq: inspect quarantine attributes (run uq help if unsure)",
        }
        .to_string(),
    );

    for (idx, path) in paths.iter().enumerate() {
        log_step(
            idx + 1,
            paths.len(),
            match lang {
                Lang::Zh => "检查目标",
                Lang::En => "Inspect target",
            },
            Some(path.display().to_string()),
        );

        if !path.exists() {
            log_fail(match lang {
                Lang::Zh => format!("{} 不存在", path.display()),
                Lang::En => format!("{} does not exist", path.display()),
            });
            continue;
        }

        let mut found = 0;
        for entry in WalkDir::new(path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if let Ok(Some(val)) = xattr::get(p, QUARANTINE_ATTR) {
                found += 1;
                println!(
                    "  {} {}  {}",
                    "[隔离]".yellow(),
                    p.display(),
                    String::from_utf8_lossy(&val).trim().dimmed()
                );
            }
        }

        if found == 0 {
            println!(
                "  {} {}",
                "[干净]".green(),
                path.display().to_string().green()
            );
        } else {
            println!(
                "  {}",
                match lang {
                    Lang::Zh => format!("共 {} 项带隔离属性", found),
                    Lang::En => format!("{} item(s) quarantined", found),
                }
                .dimmed()
            );
        }
    }
}

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

    log_intro(
        match lang {
            Lang::Zh => "uq：实时监听文件变化（忘了用法就运行 uq help）",
            Lang::En => "uq: watch files in realtime (run uq help if unsure)",
        }
        .to_string(),
    );

    let monitored = watch_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    log_step(
        1,
        2,
        match lang {
            Lang::Zh => "补扫历史隔离属性",
            Lang::En => "Sweep leftover quarantine",
        },
        Some(truncate(&monitored, 100)),
    );

    let mut initial = SweepStats::default();
    for p in &watch_paths {
        if p.exists() {
            initial += sweep_fast(p, false, false);
        }
    }
    let (initial_removed, initial_failed) = (initial.removed, initial.failed);
    if initial_removed > 0 || initial_failed > 0 {
        println!(
            "{} {}",
            timestamp().dimmed(),
            match lang {
                Lang::Zh => format!("[完成] 启动补扫 · 解除 {} 项 · 失败 {} 项", initial_removed, initial_failed),
                Lang::En => format!("[完成] startup sweep · stripped {} · failed {}", initial_removed, initial_failed),
            }
        );
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

    log_step(
        2,
        2,
        match lang {
            Lang::Zh => "监听目录变化",
            Lang::En => "Watch directory changes",
        },
        None,
    );
    for p in &watch_paths {
        println!("  {}", p.display().to_string().dimmed());
    }

    log_done(match lang {
        Lang::Zh => "监听中，按 Ctrl+C 退出".to_string(),
        Lang::En => "Watching, press Ctrl+C to stop".to_string(),
    });

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
                    let stats = sweep(&path, false, false);
                    if stats.removed > 0 {
                        log_event(lang, &path, stats.removed, stats.origin);
                    }
                    if stats.failed > 0 {
                        log_warn_event(lang, &path, stats.failed);
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

            log_step(
                1,
                2,
                match lang {
                    Lang::Zh => "写入 launchd 配置",
                    Lang::En => "Write launchd plist",
                },
                Some(plist_path.display().to_string()),
            );
            fs::write(&plist_path, plist_content).expect("Failed to write plist file");

            log_step(
                2,
                2,
                match lang {
                    Lang::Zh => "注册并启动服务",
                    Lang::En => "Register and start service",
                },
                None,
            );
            let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
            let status = Command::new("launchctl").arg("load").arg(&plist_path).status();

            match status {
                Ok(s) if s.success() => {
                    let done = if zero_memory {
                        match lang {
                            Lang::Zh => "后台服务已就绪 · WatchPaths 零常驻模式".to_string(),
                            Lang::En => "Background agent ready · WatchPaths zero-memory mode".to_string(),
                        }
                    } else {
                        match lang {
                            Lang::Zh => "后台服务已运行 · FSEvents 实时守护".to_string(),
                            Lang::En => "Background agent running · realtime FSEvents daemon".to_string(),
                        }
                    };
                    log_done(done);
                    println!(
                        "  {}",
                        match lang {
                            Lang::Zh => if zero_memory {
                                "空闲 0 MB 内存 · 目录变化时由 launchd 唤醒（较慢）".to_string()
                            } else {
                                "约 7 MB 内存 · 350ms 内完成清理 · 启动时自动补扫".to_string()
                            },
                            Lang::En => if zero_memory {
                                "0 MB idle · woken by launchd on directory changes (slower)".to_string()
                            } else {
                                "~7 MB memory · cleanup within 350ms · startup sweep included".to_string()
                            },
                        }
                        .dimmed()
                    );
                    println!("  {}", format!("plist  {}", plist_path.display()).dimmed());
                    println!("  {}", format!("日志   {}", LOG_PATH).dimmed());
                }
                _ => {
                    log_fail(match lang {
                        Lang::Zh => "launchctl 加载服务失败".to_string(),
                        Lang::En => "launchctl failed to load the service".to_string(),
                    });
                    log_hint(match lang {
                        Lang::Zh => format!("配置已写入 {}，可运行 uq service uninstall 回滚", plist_path.display()),
                        Lang::En => format!("Plist written to {}; run uq service uninstall to roll back", plist_path.display()),
                    });
                }
            }
        }
        Some(("uninstall", _)) => {
            if plist_path.exists() {
                let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
                let _ = fs::remove_file(&plist_path);
                log_done(match lang {
                    Lang::Zh => "后台服务已停止并卸载".to_string(),
                    Lang::En => "Background agent stopped and uninstalled".to_string(),
                });
                println!("  {}", plist_path.display().to_string().dimmed());
            } else {
                log_warn(match lang {
                    Lang::Zh => "后台服务尚未安装".to_string(),
                    Lang::En => "Background agent is not installed".to_string(),
                });
                log_hint(match lang {
                    Lang::Zh => "运行 uq service install 开启后台自动清理".to_string(),
                    Lang::En => "Run uq service install to enable automatic cleanup".to_string(),
                });
            }
        }
        Some(("status", _)) => {
            let output = Command::new("launchctl")
                .arg("list")
                .output()
                .expect("Failed to execute launchctl");
            let list = String::from_utf8_lossy(&output.stdout);

            log_action(
                match lang {
                    Lang::Zh => "检查后台服务",
                    Lang::En => "Inspect background service",
                },
                Some(plist_path.display().to_string()),
            );

            if list.contains(SERVICE_LABEL) {
                let is_daemon = fs::read_to_string(&plist_path)
                    .map(|c| c.contains("<key>KeepAlive</key>"))
                    .unwrap_or(false);
                let mode = if is_daemon {
                    match lang {
                        Lang::Zh => "FSEvents 实时守护 (350ms 内完成清理)",
                        Lang::En => "Realtime FSEvents daemon (cleanup within 350ms)",
                    }
                } else {
                    match lang {
                        Lang::Zh => "WatchPaths 零常驻 (目录变化时唤醒)",
                        Lang::En => "WatchPaths zero-memory (woken on changes)",
                    }
                };
                println!(
                    "  {} {}",
                    "●".green(),
                    match lang {
                        Lang::Zh => "运行中",
                        Lang::En => "Running",
                    }
                    .green()
                    .bold()
                );
                println!("  {} {}", pad_end(match lang { Lang::Zh => "模式", Lang::En => "Mode" }, 6).dimmed(), mode);
                let dirs = default_watch_dirs()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n         ");
                println!("  {} {}", pad_end(match lang { Lang::Zh => "监听", Lang::En => "Watch" }, 6).dimmed(), dirs);
                println!("  {} {}", pad_end(match lang { Lang::Zh => "日志", Lang::En => "Log" }, 6).dimmed(), LOG_PATH.dimmed());
            } else if plist_path.exists() {
                log_warn(match lang {
                    Lang::Zh => "配置存在但服务未加载".to_string(),
                    Lang::En => "Plist exists but the service is not loaded".to_string(),
                });
                log_hint(match lang {
                    Lang::Zh => "运行 uq service install 重新注册".to_string(),
                    Lang::En => "Run uq service install to register it again".to_string(),
                });
            } else {
                log_warn(match lang {
                    Lang::Zh => "服务未安装".to_string(),
                    Lang::En => "Service is not installed".to_string(),
                });
                log_hint(match lang {
                    Lang::Zh => "运行 uq service install 开启后台自动清理".to_string(),
                    Lang::En => "Run uq service install to enable automatic cleanup".to_string(),
                });
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
        log_intro(
            match lang {
                Lang::Zh => "uq：清空后台日志（忘了用法就运行 uq help）",
                Lang::En => "uq: clear service logs (run uq help if unsure)",
            }
            .to_string(),
        );
        log_done(match lang {
            Lang::Zh => "后台日志已清空".to_string(),
            Lang::En => "Service logs cleared".to_string(),
        });
        println!("  {}", LOG_PATH.dimmed());
        return;
    }

    if sub_m.get_flag("follow") {
        if !log_path.exists() {
            let _ = fs::write(&log_path, "");
        }
        log_intro(
            match lang {
                Lang::Zh => "uq：实时追踪后台日志（忘了用法就运行 uq help）",
                Lang::En => "uq: follow service logs (run uq help if unsure)",
            }
            .to_string(),
        );
        log_action(
            match lang {
                Lang::Zh => "跟踪日志输出",
                Lang::En => "Follow log output",
            },
            Some(format!("{} · Ctrl+C 退出", LOG_PATH)),
        );
        let _ = Command::new("tail").arg("-f").arg(&log_path).status();
        return;
    }

    let lines_to_show = *sub_m.get_one::<usize>("lines").unwrap_or(&20);

    if !log_path.exists() {
        log_warn(match lang {
            Lang::Zh => format!("暂无日志文件（{} 不存在）", LOG_PATH),
            Lang::En => format!("No log file yet ({} does not exist)", LOG_PATH),
        });
        return;
    }

    let content = fs::read_to_string(&log_path).unwrap_or_default();
    let all_lines: Vec<&str> = content.lines().collect();

    if all_lines.is_empty() {
        log_warn(match lang {
            Lang::Zh => "后台日志内容为空".to_string(),
            Lang::En => "Service log is empty".to_string(),
        });
        log_hint(match lang {
            Lang::Zh => "有文件落地并触发清理后这里才会有内容".to_string(),
            Lang::En => "Entries appear once a file event triggers cleanup".to_string(),
        });
        return;
    }

    let start = all_lines.len().saturating_sub(lines_to_show);

    println!();
    println!(
        "{} {}",
        "[执行]".cyan(),
        match lang {
            Lang::Zh => format!("读取最近 {} 条日志", all_lines.len() - start),
            Lang::En => format!("Read last {} log entries", all_lines.len() - start),
        }
        .bold()
    );
    println!("  {}", LOG_PATH.dimmed());
    println!();
    for line in &all_lines[start..] {
        println!("  {}", line);
    }
}

fn handle_lang_command(target_lang: Option<&String>, current_lang: Lang) {
    if let Some(target) = target_lang {
        if let Some(new_lang) = Lang::from_str(target) {
            if let Err(e) = save_lang(new_lang) {
                log_fail(format!("Failed to save language: {}", e));
            } else {
                log_done(match new_lang {
                    Lang::Zh => "默认语言已切换为中文 (zh)".to_string(),
                    Lang::En => "Default language set to English (en)".to_string(),
                });
            }
        } else {
            log_fail(format!("Unknown language '{}' · supported: zh, en", target));
            log_hint("uq lang zh | uq lang en".to_string());
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

    let launchctl_output = Command::new("launchctl")
        .arg("list")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut is_active = false;
    let mut pid_opt: Option<u32> = None;
    for line in launchctl_output.lines() {
        if line.contains(SERVICE_LABEL) {
            is_active = true;
            if let Some(pid) = line.split_whitespace().next().and_then(|s| s.parse::<u32>().ok()) {
                pid_opt = Some(pid);
            }
            break;
        }
    }

    let is_daemon = service_installed
        && fs::read_to_string(&plist_path)
            .map(|c| c.contains("<key>KeepAlive</key>"))
            .unwrap_or(false);

    let (status, mode, mem, cpu) = if !service_installed {
        (
            match lang {
                Lang::Zh => "未安装",
                Lang::En => "Not installed",
            },
            match lang {
                Lang::Zh => "—",
                Lang::En => "—",
            },
            "—".to_string(),
            "—".to_string(),
        )
    } else if !is_active {
        (
            match lang {
                Lang::Zh => "已停止",
                Lang::En => "Stopped",
            },
            match lang {
                Lang::Zh => "—",
                Lang::En => "—",
            },
            "—".to_string(),
            "—".to_string(),
        )
    } else if is_daemon {
        let (mem, cpu) = pid_opt
            .and_then(|pid| {
                Command::new("ps")
                    .arg("-o")
                    .arg("rss,%cpu")
                    .arg("-p")
                    .arg(pid.to_string())
                    .output()
                    .ok()
                    .and_then(|out| {
                        let text = String::from_utf8_lossy(&out.stdout).to_string();
                        let parts: Vec<String> = text
                            .lines()
                            .nth(1)?
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();
                        let rss: f64 = parts.first()?.parse().ok()?;
                        Some((format!("{:.2} MB", rss / 1024.0), format!("{}%", parts.get(1)?)))
                    })
            })
            .unwrap_or_else(|| ("~7.00 MB".to_string(), "0.0%".to_string()));
        (
            match lang {
                Lang::Zh => "运行中 (常驻守护)",
                Lang::En => "Running (daemon)",
            },
            match lang {
                Lang::Zh => "FSEvents 实时监听 (毫秒级触发)",
                Lang::En => "Realtime FSEvents (millisecond trigger)",
            },
            mem,
            cpu,
        )
    } else {
        (
            match lang {
                Lang::Zh => "就绪 (事件唤醒)",
                Lang::En => "Ready (event trigger)",
            },
            match lang {
                Lang::Zh => "WatchPaths 零常驻",
                Lang::En => "WatchPaths zero-memory",
            },
            "0.00 MB".to_string(),
            "0.0%".to_string(),
        )
    };

    let power = match lang {
        Lang::Zh => "< 0.05 W",
        Lang::En => "< 0.05 W",
    };
    let size = std::env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| format!("{:.0} KB", m.len() as f64 / 1024.0))
        .unwrap_or_else(|| "—".to_string());

    let home = home_dir().unwrap_or_default();
    let dirs = default_watch_dirs()
        .iter()
        .map(|p| p.display().to_string().replace(&home.display().to_string(), "~"))
        .collect::<Vec<_>>()
        .join(" ");

    let (dot, status_color) = if !service_installed || !is_active {
        ("○", "yellow")
    } else {
        ("●", "green")
    };
    let status_text = format!("{} {}", dot, status);
    let status_colored = match status_color {
        "green" => status_text.green().bold().to_string(),
        _ => status_text.yellow().bold().to_string(),
    };

    let (t_service, t_perf) = match lang {
        Lang::Zh => ("服务", "资源开销"),
        Lang::En => ("Service", "Footprint"),
    };
    let (l_status, l_mode, l_dirs, l_log, l_mem, l_power) = match lang {
        Lang::Zh => ("状态", "模式", "监听", "日志", "内存", "功耗"),
        Lang::En => ("State", "Mode", "Watch", "Log", "Memory", "Power"),
    };

    println!();
    println!(
        "  {} {}",
        "⚡".yellow(),
        format!("uq v{}", env!("CARGO_PKG_VERSION")).bold()
    );
    println!("  {}", match lang {
        Lang::Zh => "极速 macOS 隔离属性自动解除与实时守护",
        Lang::En => "Lightning-fast macOS quarantine remover & realtime watcher",
    }.dimmed());
    println!();
    println!("{}", box_top(t_service, t_service.cyan().bold().to_string()));
    println!("{}", box_row(l_status, &status_text, status_colored));
    println!("{}", box_row(l_mode, mode, mode.dimmed().to_string()));
    let dirs_cell = truncate_words(&dirs, 54);
    println!("{}", box_row(l_dirs, &dirs_cell, dirs_cell.dimmed().to_string()));
    println!("{}", box_row(l_log, LOG_PATH, LOG_PATH.dimmed().to_string()));
    println!("{}", box_sep(t_perf, t_perf.cyan().bold().to_string()));
    println!(
        "{}",
        box_row(
            l_mem,
            &format!("{}  ·  CPU {}", mem, cpu),
            format!("{}  ·  CPU {}", mem, cpu).dimmed().to_string()
        )
    );
    println!(
        "{}",
        box_row(
            l_power,
            &format!("{}  ·  {}", power, size),
            format!("{}  ·  {}", power, size).dimmed().to_string()
        )
    );
    println!("{}", box_bottom());
    println!();
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
            handle_do(&paths, active_lang, verbose, sub_m.get_flag("dry-run"));
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
                        let _ = sweep_fast(p, verbose, false);
                    }
                }
                return;
            }

            handle_do(&paths, active_lang, verbose, false);
        }
    }
}

