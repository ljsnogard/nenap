//! 配置与运行时目录的**位置**：显式覆盖 → 便携 base → 平台约定目录。
//!
//! 三个目录的语义不同：
//!
//! | 目录 | 默认值 | 说明 |
//! | :--- | :--- | :--- |
//! | 配置文件 | 平台配置目录（Linux `~/.config/…`） | 连接选择；小、持久 |
//! | 运行时目录 | `$XDG_RUNTIME_DIR/llm_kb` | UDS socket；通常是 tmpfs |
//! | 存储目录 | **没有默认值** | 知识库数据；必须由用户明确选择 |
//!
//! 设置 [`PORTABLE_HOME_ENV`] 后进入**便携模式**：配置文件与运行时目录都放在
//! 同一棵目录树下（`<base>/config.toml`、`<base>/run/`）。存储目录仍然由用户选
//! （通常选 `<base>/data`）。便携模式是显式 opt-in，不会改变普通用户的 XDG 默认。

use std::path::PathBuf;

/// 显式指定配置文件的**环境变量**。
///
/// 设置之后优先于便携 base 与平台约定目录；开发、排错、或者同时维护多份配置时用。
pub const CONFIG_PATH_ENV: &str = "KB_ADMIN_DESKTOP_CONFIG";

/// 便携模式的**基准目录**环境变量。
///
/// 设置之后：
///
/// - 配置文件默认是 `<base>/config.toml`（除非 [`CONFIG_PATH_ENV`] 另行覆盖）；
/// - 默认运行时目录是 `<base>/run`；
/// - 存储目录**仍然没有默认值**，界面要用户自己选（通常选 `<base>/data`）。
pub const PORTABLE_HOME_ENV: &str = "KB_ADMIN_DESKTOP_HOME";

/// 应用在配置目录下的子目录名。
pub const APP_DIR: &str = "kb_admin_desktop";

/// 配置文件名。
pub const CONFIG_FILE: &str = "config.toml";

/// 配置文件的路径：显式覆盖 → 便携 base → 平台约定目录。
///
/// 返回 `None` 表示既没有显式覆盖、没有便携 base、也推导不出平台目录（极少见，
/// 通常是环境变量被清空了）。这种情况下界面应当让用户直接指定一个路径。
pub fn config_path() -> Option<PathBuf> {
    config_path_from_(
        std::env::var_os(CONFIG_PATH_ENV).map(PathBuf::from),
        portable_home(),
        default_config_path(),
    )
}

/// 平台约定目录下的配置路径（**不含**便携 base）。
///
/// | 平台 | 位置 |
/// | :--- | :--- |
/// | Linux 等 | `$XDG_CONFIG_HOME/kb_admin_desktop/config.toml`，缺省 `~/.config/...` |
/// | macOS | `~/Library/Application Support/kb_admin_desktop/config.toml` |
/// | Windows | `%APPDATA%\kb_admin_desktop\config.toml` |
pub fn default_config_path() -> Option<PathBuf> {
    config_dir_().map(|dir| dir.join(APP_DIR).join(CONFIG_FILE))
}

/// 便携模式的基准目录（[`PORTABLE_HOME_ENV`] 非空时）。
///
/// 只是把环境变量读成路径；是否使用它由 [`config_path`] / [`default_runtime_dir`]
/// 决定。空值视作未设置。
pub fn portable_home() -> Option<PathBuf> {
    std::env::var_os(PORTABLE_HOME_ENV)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// `kb_core` 的**缺省运行时目录**（UDS socket 放在这里）。
///
/// - 便携模式：`<base>/run`；
/// - 否则与 `kb_core` 自己的推导保持一致：`$XDG_RUNTIME_DIR/llm_kb`，没有
///   `XDG_RUNTIME_DIR` 时退到系统临时目录下的 `llm_kb`。
///
/// 单独放在这里，是为了让"首次运行"的界面能把它作为输入框的缺省值。
/// 普通模式下它与 `kb_core` 自己的推导一致；便携模式只是客户端的约定，
/// 手工跑 `kb-core` 时仍要显式给 `--runtime-dir`。
///
/// **存储目录没有对应的默认值**：它必须由用户明确选择，见 crate 根文档。
pub fn default_runtime_dir() -> PathBuf {
    runtime_dir_from_(
        portable_home(),
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty()),
    )
}

/// 显式覆盖 → 便携 base → 平台路径。
///
/// 做成纯函数只是为了可测：读环境变量的那半边在 [`config_path`] 里。
fn config_path_from_(
    explicit: Option<PathBuf>,
    portable: Option<PathBuf>,
    platform: Option<PathBuf>,
) -> Option<PathBuf> {
    match explicit {
        Some(path) if !path.as_os_str().is_empty() => Some(path),
        _ => match portable {
            Some(home) if !home.as_os_str().is_empty() => Some(home.join(CONFIG_FILE)),
            _ => platform,
        },
    }
}

/// 便携 base 优先；否则 XDG（再回退系统临时目录）下的 `llm_kb`。
fn runtime_dir_from_(portable: Option<PathBuf>, xdg_runtime_dir: Option<PathBuf>) -> PathBuf {
    if let Some(home) = portable.filter(|home| !home.as_os_str().is_empty()) {
        return home.join("run");
    }
    xdg_runtime_dir
        .unwrap_or_else(std::env::temp_dir)
        .join("llm_kb")
}

/// 平台配置目录（不含应用子目录）。
fn config_dir_() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        non_empty_env_("APPDATA")
    }

    #[cfg(target_os = "macos")]
    {
        home_dir_().map(|home| home.join("Library").join("Application Support"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        non_empty_env_("XDG_CONFIG_HOME").or_else(|| home_dir_().map(|home| home.join(".config")))
    }
}

/// 读一个"非空才算数"的环境变量。
#[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
fn non_empty_env_(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// 用户主目录。
#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn home_dir_() -> Option<PathBuf> {
    non_empty_env_("HOME")
}

#[cfg(test)]
mod tests_ {
    use super::*;

    /// 测试运行时目录：便携 base 优先，否则 XDG，再否则系统临时目录。
    ///
    /// - 手段：直接调用纯函数 `runtime_dir_from_`，分别给
    ///   `便携 + XDG`、`空便携 + XDG`、`无便携 + XDG`、`两者都无` 四种输入。
    /// - 判断：便携 base 给出 `<base>/run`；空便携被忽略；无便携时给出
    ///   `<XDG>/llm_kb`；两者都无时给出 `<临时目录>/llm_kb`。这条钉住“便携模式
    ///   不会意外把普通用户的 socket 挪到别处”。
    #[test]
    fn runtime_dir_prefers_portable_home_() {
        let portable = PathBuf::from("/opt/kb");
        let xdg = PathBuf::from("/run/user/1000");

        assert_eq!(
            runtime_dir_from_(Some(portable.clone()), Some(xdg.clone())),
            portable.join("run")
        );
        assert_eq!(
            runtime_dir_from_(Some(PathBuf::new()), Some(xdg.clone())),
            xdg.join("llm_kb"),
            "空便携 base 应当被忽略"
        );
        assert_eq!(
            runtime_dir_from_(None, Some(xdg.clone())),
            xdg.join("llm_kb")
        );
        assert_eq!(
            runtime_dir_from_(None, None),
            std::env::temp_dir().join("llm_kb")
        );
    }

    /// 测试配置路径优先级：显式覆盖 → 便携 base → 平台约定目录。
    ///
    /// - 手段：直接调用纯函数 `config_path_from_`，分别给
    ///   `显式 + 便携 + 平台`、`空显式 + 便携`、`无显式 + 便携`、
    ///   `无便携 + 平台`、`空便携 + 平台`、`三者都无`。
    /// - 判断：非空显式路径原样胜出；空显式退回便携 base 的 `config.toml`；
    ///   便携 base 优先于平台目录；空便携被忽略；全无时返回 `None`。
    #[test]
    fn config_path_precedence_() {
        let explicit = PathBuf::from("/tmp/kb-admin-desktop.toml");
        let portable = PathBuf::from("/opt/kb");
        let platform = PathBuf::from("/home/me/.config/kb_admin_desktop/config.toml");

        assert_eq!(
            config_path_from_(
                Some(explicit.clone()),
                Some(portable.clone()),
                Some(platform.clone())
            ),
            Some(explicit)
        );
        assert_eq!(
            config_path_from_(
                Some(PathBuf::new()),
                Some(portable.clone()),
                Some(platform.clone())
            ),
            Some(portable.join(CONFIG_FILE))
        );
        assert_eq!(
            config_path_from_(None, Some(portable.clone()), Some(platform.clone())),
            Some(portable.join(CONFIG_FILE))
        );
        assert_eq!(
            config_path_from_(None, Some(PathBuf::new()), Some(platform.clone())),
            Some(platform.clone()),
            "空便携 base 应当退回平台目录"
        );
        assert_eq!(
            config_path_from_(None, None, Some(platform.clone())),
            Some(platform)
        );
        assert_eq!(config_path_from_(None, None, None), None);
    }
}
