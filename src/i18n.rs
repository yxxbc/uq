use std::fs;
use std::path::PathBuf;

const HELP_ZH: &str = "\
日常用法
  uq                    查看服务状态与资源开销
  uq do                 手动清理常用目录（~/Downloads、桌面、/Applications、/usr/local、已挂载镜像）
  uq do <路径>          清理指定目标，支持 .app / .dmg / .pkg
  uq check <路径>       查看隔离属性及其来源
  uq log [-f|-c]        查看 / 跟踪 / 清空后台日志
  uq service <动作>     安装（默认 FSEvents 守护）、卸载、查看状态
      --zero            改用零常驻内存的 WatchPaths 模式（更慢，粒度更粗）
  uq lang <zh|en>       切换默认语言

权限
  macOS 13+ 的 App Management 会阻止修改 /Applications 中已有的应用包，
  sudo 也无法绕过（权限跟随发起进程，也就是你的终端）。解决方式：
    open \"x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AppBundles\"
    把你的终端加入「App 管理」并勾选，重启终端后再执行 sudo uq do
  Homebrew Cask 安装的应用可从源头避免：export HOMEBREW_CASK_OPTS=\"--no-quarantine\"
";

const HELP_EN: &str = "\
Everyday use
  uq                    Show service state and resource usage
  uq do                 Manual sweep (~/Downloads, Desktop, /Applications, /usr/local, mounted images)
  uq do <path>          Clean a specific target, including .app / .dmg / .pkg
  uq check <path>       Show quarantine attributes and their origin
  uq log [-f|-c]        View / follow / clear service logs
  uq service <action>   Install (realtime FSEvents daemon by default), uninstall, status
      --zero            Switch to zero-idle-memory WatchPaths mode (slower, coarser)
  uq lang <zh|en>       Switch the default language

Permissions
  On macOS 13+ App Management blocks modifying existing app bundles in /Applications,
  and sudo cannot bypass it (the permission follows the invoking process: your terminal).
    open \"x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AppBundles\"
    Add and enable your terminal under App Management, restart the terminal, then run sudo uq do
  Homebrew Cask users can avoid it at the source: export HOMEBREW_CASK_OPTS=\"--no-quarantine\"
";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "zh" | "zh-cn" | "chinese" | "中文" => Some(Lang::Zh),
            "en" | "en-us" | "english" | "英文" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("unquarantine").join("config"))
}

pub fn load_saved_lang() -> Option<Lang> {
    if let Some(path) = config_path() {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("lang=") {
                    return Lang::from_str(val.trim());
                }
            }
        }
    }
    None
}

pub fn save_lang(lang: Lang) -> Result<(), std::io::Error> {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("lang={}\n", lang.code()))?;
    }
    Ok(())
}

pub fn detect_default_lang() -> Lang {
    // 1. Saved config
    if let Some(lang) = load_saved_lang() {
        return lang;
    }
    // 2. Environment variables
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let lower = val.to_lowercase();
            if lower.contains("zh") || lower.contains("chinese") {
                return Lang::Zh;
            }
        }
    }
    // 3. Fallback to English
    Lang::En
}

pub struct Messages {
    pub lang: Lang,
}

impl Messages {
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    pub fn about(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "极速 macOS 隔离属性（com.apple.quarantine）清除与后台监控工具",
            Lang::En => "Lightning-fast macOS quarantine remover & background watcher",
        }
    }

    pub fn arg_paths(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "需要清除隔离属性的文件或目录路径（缺省子命令时的默认操作）",
            Lang::En => "Paths to strip quarantine attributes from (default action if no subcommand is given)",
        }
    }

    pub fn arg_quiet(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "静默模式，减少日志输出",
            Lang::En => "Suppress detailed output",
        }
    }

    pub fn arg_lang(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "指定运行语言 (zh/en)",
            Lang::En => "Specify interface language (zh/en)",
        }
    }

    pub fn cmd_check(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "检查文件或目录是否存在隔离属性及其来源",
            Lang::En => "Inspect files/directories for quarantine attributes",
        }
    }

    pub fn cmd_watch(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "实时监听目录并在有新文件落盘时自动清除隔离属性",
            Lang::En => "Watch directories and automatically clear quarantine on new downloads/apps",
        }
    }

    pub fn cmd_watch_paths_help(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "指定监听的目录（默认: ~/Downloads, ~/Desktop, /Applications, /Volumes, /usr/local）",
            Lang::En => "Directories to monitor (default: ~/Downloads, ~/Desktop, /Applications, /Volumes, /usr/local)",
        }
    }

    pub fn cmd_service(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "管理 launchd 开机自启后台服务",
            Lang::En => "Manage background launchd service",
        }
    }

    pub fn cmd_service_install(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "安装并注册 launchd 服务（默认 FSEvents 实时守护模式）",
            Lang::En => "Install and register launchd agent (defaults to realtime FSEvents daemon)",
        }
    }

    pub fn arg_verbose(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "显示详细扫描与清除日志",
            Lang::En => "Show verbose scanning and removal output",
        }
    }

    pub fn cmd_service_zero(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "零常驻内存模式：仅在路径变化时由 launchd 唤醒（比默认守护模式慢，事件会丢失合并）",
            Lang::En => "Zero-idle-memory mode: woken by launchd WatchPaths on path changes (slower than the default daemon)",
        }
    }

    pub fn cmd_do(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "手动清理隔离属性（.dmg/.pkg 归档与已安装应用均可）",
            Lang::En => "Manually strip quarantine (archives like .dmg/.pkg and installed apps)",
        }
    }

    pub fn arg_do_paths(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "要清理的路径；留空则清理 ~/Downloads、桌面、/Applications、/usr/local 与已挂载镜像",
            Lang::En => "Paths to clean; empty sweeps ~/Downloads, Desktop, /Applications, /usr/local and mounted images",
        }
    }

    pub fn cmd_service_uninstall(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "停止并卸载 launchd 后台服务",
            Lang::En => "Stop and remove launchd background agent",
        }
    }

    pub fn cmd_service_status(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "查看后台服务运行状态",
            Lang::En => "Check background agent status",
        }
    }

    pub fn cmd_lang(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "查看或切换默认语言 (zh/en)",
            Lang::En => "Get or set preferred interface language (zh/en)",
        }
    }

    pub fn cmd_lang_target(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "要设置的目标语言 (zh/en)，留空则显示当前语言",
            Lang::En => "Target language to set (zh/en). Leave empty to display current language",
        }
    }

    pub fn cmd_log(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "查看或管理后台服务的运行日志",
            Lang::En => "View or manage background service logs",
        }
    }

    pub fn arg_log_follow(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "实时滚动追踪日志输出 (类似 tail -f)",
            Lang::En => "Follow log output in real time (like tail -f)",
        }
    }

    pub fn arg_log_lines(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "展示的日志行数 (默认: 20)",
            Lang::En => "Number of lines to show (default: 20)",
        }
    }

    pub fn after_help(&self) -> &'static str {
        match self.lang {
            Lang::Zh => HELP_ZH,
            Lang::En => HELP_EN,
        }
    }

    pub fn arg_log_clear(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "清空所有后台日志记录",
            Lang::En => "Clear all service log files",
        }
    }
}

