use clap::{Parser, Subcommand};
use colored::*;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::Duration;
use walkdir::WalkDir;

const QUARANTINE_ATTR: &str = "com.apple.quarantine";
const SERVICE_LABEL: &str = "com.user.unquarantine";

#[derive(Parser)]
#[command(name = "unquarantine")]
#[command(author = "Shorin & Miyu")]
#[command(version = "0.1.0")]
#[command(about = "Lightning-fast macOS quarantine remover & background watcher", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Paths to strip quarantine attributes from (default action if no subcommand is given)
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Suppress detailed output
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect files/directories for quarantine attributes
    Check {
        /// Paths to inspect
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<PathBuf>,
    },
    /// Watch directories and automatically clear quarantine on new downloads/apps
    Watch {
        /// Directories to monitor (default: ~/Downloads, /Applications)
        #[arg(short, long, value_name = "DIR")]
        paths: Vec<PathBuf>,
    },
    /// Manage background launchd service
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install and start launchd background agent
    Install,
    /// Stop and remove launchd background agent
    Uninstall,
    /// Check background agent status
    Status,
}

fn remove_quarantine(path: &Path) -> Result<bool, std::io::Error> {
    if let Some(_) = xattr::get(path, QUARANTINE_ATTR)? {
        xattr::remove(path, QUARANTINE_ATTR)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn process_path(target: &Path, quiet: bool) -> (usize, usize) {
    let mut total_scanned = 0;
    let mut total_removed = 0;

    if !target.exists() {
        if !quiet {
            eprintln!("{} Path does not exist: {}", "[-]".red(), target.display());
        }
        return (0, 0);
    }

    for entry in WalkDir::new(target).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        total_scanned += 1;
        match remove_quarantine(p) {
            Ok(true) => {
                total_removed += 1;
            }
            Ok(false) => {}
            Err(e) => {
                if !quiet && e.kind() != std::io::ErrorKind::NotFound {
                    // Ignore permission / not found errors quietly unless verbose
                }
            }
        }
    }

    if !quiet {
        if total_removed > 0 {
            println!(
                "{} Removed quarantine from {} ({} item{})",
                "[+]".green().bold(),
                target.display(),
                total_removed,
                if total_removed > 1 { "s" } else { "" }
            );
        } else {
            println!(
                "{} No quarantine found on {} (scanned {} items)",
                "[=]".blue(),
                target.display(),
                total_scanned
            );
        }
    }

    (total_scanned, total_removed)
}

fn handle_check(paths: &[PathBuf]) {
    for path in paths {
        if !path.exists() {
            println!("{} {} (not found)", "[-]".red(), path.display());
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
            println!("{} Clean: {}", "[OK]".green().bold(), path.display());
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

fn handle_watch(custom_paths: &[PathBuf]) {
    let watch_paths = if custom_paths.is_empty() {
        default_watch_dirs()
    } else {
        custom_paths.to_vec()
    };

    println!("{}", "=== Unquarantine Watcher Started ===".cyan().bold());
    println!("Monitoring directories:");
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
                        // Avoid processing inside app packages repeatedly during deep copy
                        if let Some(ext) = path.extension() {
                            if ext == "download" || ext == "crdownload" || ext == "part" {
                                continue;
                            }
                        }
                        let (_, removed) = process_path(&path, true);
                        if removed > 0 {
                            println!(
                                "{} [Auto-Clean] Cleared {} quarantine attribute(s) from {}",
                                "[+]".green().bold(),
                                removed,
                                path.display()
                            );
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

fn handle_service(action: ServiceAction) {
    let plist_path = get_plist_path();

    match action {
        ServiceAction::Install => {
            let exe_path = get_current_exe();
            let parent_dir = plist_path.parent().unwrap();
            fs::create_dir_all(parent_dir).expect("Failed to create LaunchAgents directory");

            let plist_content = format!(
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
            );

            fs::write(&plist_path, plist_content).expect("Failed to write plist file");
            let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
            let status = Command::new("launchctl").arg("load").arg(&plist_path).status();

            match status {
                Ok(s) if s.success() => {
                    println!(
                        "{} Background agent installed and started successfully!",
                        "[+]".green().bold()
                    );
                    println!("Service Plist: {}", plist_path.display());
                    println!("Logs: /tmp/unquarantine.log");
                }
                _ => {
                    eprintln!("{} Failed to load launchd service with launchctl.", "[-]".red());
                }
            }
        }
        ServiceAction::Uninstall => {
            if plist_path.exists() {
                let _ = Command::new("launchctl").arg("unload").arg(&plist_path).output();
                let _ = fs::remove_file(&plist_path);
                println!("{} Background agent stopped and uninstalled.", "[+]".green().bold());
            } else {
                println!("{} Background agent is not installed.", "[=]".blue());
            }
        }
        ServiceAction::Status => {
            let output = Command::new("launchctl")
                .arg("list")
                .output()
                .expect("Failed to execute launchctl");
            let list = String::from_utf8_lossy(&output.stdout);
            if list.contains(SERVICE_LABEL) {
                println!("{} Service is ACTIVE (running)", "[RUNNING]".green().bold());
                println!("Plist: {}", plist_path.display());
                println!("Logs: /tmp/unquarantine.log");
            } else if plist_path.exists() {
                println!("{} Service plist exists but is not running", "[STOPPED]".yellow().bold());
            } else {
                println!("{} Service is not installed", "[NOT INSTALLED]".blue());
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { paths }) => {
            handle_check(&paths);
        }
        Some(Commands::Watch { paths }) => {
            handle_watch(&paths);
        }
        Some(Commands::Service { action }) => {
            handle_service(action);
        }
        None => {
            if cli.paths.is_empty() {
                eprintln!("{} No paths specified. Run `unquarantine --help` for usage.", "[-]".yellow());
                std::process::exit(1);
            }
            let mut total_s = 0;
            let mut total_r = 0;
            for p in &cli.paths {
                let (s, r) = process_path(p, cli.quiet);
                total_s += s;
                total_r += r;
            }
            if !cli.quiet && cli.paths.len() > 1 {
                println!(
                    "{} Done. Processed {} items total, stripped quarantine from {}.",
                    "[*]".cyan().bold(),
                    total_s,
                    total_r
                );
            }
        }
    }
}
