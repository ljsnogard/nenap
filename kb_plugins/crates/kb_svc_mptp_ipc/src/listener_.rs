//! 服务端监听：bind 一个 UDS，accept 后完成 smux 握手。

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use compio::net::UnixListener;

use crate::config_::adapt_;
use crate::connection_::Connection;
use crate::error_::MptpIpcError;
use crate::rendezvous_::{clear_stale_sockets_, new_socket_path_in};

/// 服务端端点。
pub struct Listener {
    /// 本次启动的 socket 路径。
    socket_path_: PathBuf,

    /// 底层监听套接字。
    listener_: UnixListener,
}

impl Listener {
    /// 在 `runtime_dir` 下准备并绑定 socket。
    pub async fn bind(runtime_dir: impl Into<PathBuf>) -> Result<Self, MptpIpcError> {
        let runtime_dir = runtime_dir.into();
        std::fs::create_dir_all(&runtime_dir).map_err(|source| MptpIpcError::RuntimeDir {
            path: runtime_dir.clone(),
            source,
        })?;
        clear_stale_sockets_(&runtime_dir)?;

        let socket_path = new_socket_path_in(&runtime_dir);
        // 107 是 Linux `sun_path` 的上限（含结尾 NUL）。
        if socket_path.as_os_str().len() > 106 {
            return Err(MptpIpcError::PathTooLong { path: socket_path });
        }

        let listener =
            UnixListener::bind(&socket_path)
                .await
                .map_err(|source| MptpIpcError::Bind {
                    path: socket_path.clone(),
                    source,
                })?;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600)).map_err(
            |source| MptpIpcError::Bind {
                path: socket_path.clone(),
                source,
            },
        )?;
        Ok(Self {
            socket_path_: socket_path,
            listener_: listener,
        })
    }

    /// 本次启动的 socket 路径。
    pub fn socket_path(&self) -> &Path {
        &self.socket_path_
    }

    /// 等一个客户端连上来，并完成 smux 握手。
    pub async fn accept(&self) -> Result<Connection, MptpIpcError> {
        let (stream, _peer) =
            self.listener_
                .accept()
                .await
                .map_err(|source| MptpIpcError::Bind {
                    path: self.socket_path_.clone(),
                    source,
                })?;
        let (tx, rx) = adapt_(stream).map_err(|error| MptpIpcError::Connect(error.to_string()))?;
        let conn = crate::connection_::handshake_listen_(tx, rx).await?;
        log::debug!("客户端已连接");
        Ok(Connection::new_(conn))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path_);
    }
}
