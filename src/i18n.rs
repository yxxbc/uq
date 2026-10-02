use std::fs;
use std::path::PathBuf;

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
            Lang::Zh => "指定监听的目录（默认: ~/Downloads, /Applications）",
            Lang::En => "Directories to monitor (default: ~/Downloads, /Applications)",
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
            Lang::Zh => "安装并注册 launchd 服务（默认零内存 WatchPaths 模式）",
            Lang::En => "Install and register launchd agent (defaults to 0-memory WatchPaths event mode)",
        }
    }

    pub fn cmd_service_daemon(&self) -> &'static str {
        match self.lang {
            Lang::Zh => "以持续长驻守护进程运行（watch 模式），而不是零内存事件唤醒模式",
            Lang::En => "Run as a persistent background daemon (watch mode) instead of zero-memory WatchPaths event trigger",
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
}
