//! 传输层错误。

use thiserror::Error;

/// 建立 / 维持本机 mptp 连接时的失败。
#[derive(Debug, Error)]
pub enum MptpIpcError {
    /// 运行时目录建不出来。
    #[error("运行时目录不可用: {}: {source}", path.display())]
    RuntimeDir {
        /// 目录。
        path: std::path::PathBuf,
        /// 底层错误。
        #[source]
        source: std::io::Error,
    },

    /// socket 路径超过系统上限。
    #[error("socket 路径过长（上限约 107 字节）: {}", path.display())]
    PathTooLong {
        /// 路径。
        path: std::path::PathBuf,
    },

    /// bind / 权限设置失败。
    #[error("绑定本机 socket 失败: {}: {source}", path.display())]
    Bind {
        /// 路径。
        path: std::path::PathBuf,
        /// 底层错误。
        #[source]
        source: std::io::Error,
    },

    /// 连接超时。
    #[error("等待 kb_core 的 socket 超时（{} ms）", timeout.as_millis())]
    ConnectTimeout {
        /// 等多久。
        timeout: std::time::Duration,
    },

    /// 连接 / smux 握手失败。
    #[error("连接 kb_core 失败: {0}")]
    Connect(String),

    /// 编译期/运行期配置问题。
    #[error("运行时配置失败: {0}")]
    Runtime(String),

    /// 服务端处理失败。
    #[error("服务端处理失败: {0}")]
    Serve(String),

    /// RPC 层失败。
    #[error(transparent)]
    Rpc(#[from] rpc_core_v1::messaging::RpcError),

    /// 客户端宿主线程已经退出。
    #[error("客户端宿主线程已退出")]
    HostGone,

    /// 起宿主线程失败。
    #[error("起客户端宿主线程失败: {0}")]
    HostThread(#[source] std::io::Error),
}
