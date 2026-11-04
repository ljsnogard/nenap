//! 客户端宿主线程：建 compio Runtime、连 UDS、跑作业队列。

use std::rc::Rc;
use std::time::{Duration, Instant};

use compio::net::UnixStream;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_lite::StreamExt;
use rpc_core_v1::preludes::{CoreAlloc, MptpClient, Shared};

use crate::config_::{CliCfg, adapt_};
use crate::connection_::handshake_invite_;
use crate::error_::MptpIpcError;
use crate::rendezvous_::find_socket_in;

/// 一个作业：在宿主线程里借用 `MptpClient` 构造 future 并执行。
pub(crate) type Job = Box<
    dyn FnOnce(Rc<MptpClient<CliCfg>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>
        + Send
        + 'static,
>;

/// 起宿主线程并等它就绪。
pub(crate) fn connect_host_(
    runtime_dir: std::path::PathBuf,
    timeout: Duration,
) -> Result<UnboundedSender<Job>, MptpIpcError> {
    let (job_tx, job_rx) = unbounded::<Job>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), MptpIpcError>>();

    std::thread::Builder::new()
        .name("kb-mptp-client".to_string())
        .spawn(move || host_main_(runtime_dir, timeout, ready_tx, job_rx))
        .map_err(MptpIpcError::HostThread)?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(job_tx),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(MptpIpcError::HostGone),
    }
}

/// 宿主线程主体。
fn host_main_(
    runtime_dir: std::path::PathBuf,
    timeout: Duration,
    ready_tx: std::sync::mpsc::Sender<Result<(), MptpIpcError>>,
    mut job_rx: UnboundedReceiver<Job>,
) {
    let outcome = (|| -> Result<(), MptpIpcError> {
        let runtime = compio::runtime::Runtime::new()
            .map_err(|error| MptpIpcError::Runtime(error.to_string()))?;
        let ready_in_async = ready_tx.clone();
        runtime.block_on(async move {
            let rt = abs_art_facade::default_rt_();
            let _scope = rt.local_scope();
            let deadline = Instant::now() + timeout;

            let socket = find_socket_until_(&runtime_dir, deadline).await?;
            let stream = connect_until_(&socket, deadline).await?;
            let (tx, rx) =
                adapt_(stream).map_err(|error| MptpIpcError::Connect(error.to_string()))?;
            let conn = handshake_invite_(tx, rx).await?;
            let shared = Shared::new(conn, CoreAlloc);
            let client = Rc::new(MptpClient::new(shared));

            let _ = ready_in_async.send(Ok(()));
            while let Some(job) = job_rx.next().await {
                let client = Rc::clone(&client);
                compio::runtime::spawn(job(client)).detach();
            }
            Ok(())
        })
    })();

    if let Err(error) = outcome {
        let _ = ready_tx.send(Err(error));
    }
}

/// 等服务端公布 socket；目录还不存在时继续等。
async fn find_socket_until_(
    runtime_dir: &std::path::Path,
    deadline: Instant,
) -> Result<std::path::PathBuf, MptpIpcError> {
    loop {
        match find_socket_in(runtime_dir) {
            Ok(Some(path)) => return Ok(path),
            Ok(None) => {}
            Err(MptpIpcError::RuntimeDir { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(MptpIpcError::ConnectTimeout {
                timeout: deadline.saturating_duration_since(Instant::now()),
            });
        }
        compio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 带重试地连 socket。
async fn connect_until_(
    socket: &std::path::Path,
    deadline: Instant,
) -> Result<UnixStream, MptpIpcError> {
    loop {
        match UnixStream::connect(socket).await {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                if Instant::now() >= deadline {
                    return Err(MptpIpcError::Connect(format!(
                        "连不上 {}: {error}",
                        socket.display()
                    )));
                }
            }
        }
        compio::time::sleep(Duration::from_millis(20)).await;
    }
}
