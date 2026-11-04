//! 有界字节管道：上行（远程客户端 → 网关）的背压点。
//!
//! 网关用它给**上行**加一个有界中间缓冲：TCP 读到的字节先送进有界通道，
//! 转发任务再从通道里取。通道满了写端就等，于是**读端不再从 socket 取数据**，
//! TCP 窗口自然关上——这就是背压，而不是"内存无声涨上去"。
//!
//! # 为什么用 mpsc
//!
//! 这里只需要的语义是「有界 + 异步等待 + 关闭后读到 EOF」，用
//! `futures_channel::mpsc` 的有界通道即可表达，而且不涉及 `unsafe` 与段搬运。
//! 通道容量按**字节数**换算成消息数（每块按 [`K_CHUNK_BYTES_`] 计）。
//!
//! # 收尾
//!
//! 显式 [`close`] 会关闭通道：已排队的块仍可读完，之后读到 `None`（EOF）。
//! 读侧收工后通道被丢弃，仍在等待的写者会收到发送失败，不会永久停等。

use futures::SinkExt;
use futures_channel::mpsc::{Receiver, Sender, channel};
use futures_lite::StreamExt;
use thiserror::Error;

/// 估算容量时假定的一块大小（与调用方的读块一致）。
const K_CHUNK_BYTES_: usize = 4096;

/// 上行缓冲的容量（字节）。
///
/// 请求信封都很小（几百字节），64 KiB 足够吸收突发；调大只会推迟背压生效。
pub const UPLINK_CAPACITY: usize = 64 * 1024;

/// 环形缓冲的失败。
#[derive(Debug, Error)]
pub enum RingError {
    /// 创建失败（容量不合法等）。
    #[error("创建上行缓冲失败: {0}")]
    Build(String),

    /// 写入失败（通道已关闭）。
    #[error("写入上行缓冲失败: {0}")]
    Write(String),
}

/// 写入端。
pub struct Sink {
    /// 通道；`None` 表示已经显式关闭。
    tx_: Option<Sender<Vec<u8>>>,
}

/// 读出端。
pub struct Source {
    /// 通道。
    rx_: Receiver<Vec<u8>>,

    /// 上一块没有取完的剩余字节。
    pending_: Vec<u8>,
}

/// 建一对有界字节管道。
///
/// `capacity` 是**字节数**；内部按 [`K_CHUNK_BYTES_`] 换算成通道消息数。
///
/// # Errors
///
/// 容量小于 2 字节时返回 [`RingError::Build`]。
pub async fn pipe(capacity: usize) -> Result<(Sink, Source), RingError> {
    if capacity < 2 {
        return Err(RingError::Build("容量必须至少 2 字节".to_string()));
    }
    let messages = capacity.div_ceil(K_CHUNK_BYTES_).max(2);
    let (tx, rx) = channel::<Vec<u8>>(messages);
    Ok((
        Sink { tx_: Some(tx) },
        Source {
            rx_: rx,
            pending_: Vec::new(),
        },
    ))
}

/// 把 `data` 全部写进管道；通道满了就等。
///
/// 这是背压的落点：等待期间调用方不会继续从 socket 取数据。
///
/// # Errors
///
/// 写端已关闭（或读端已丢弃）时返回 [`RingError::Write`]。
pub async fn write_all(sink: &mut Sink, data: &[u8]) -> Result<(), RingError> {
    if data.is_empty() {
        return Ok(());
    }
    let Some(tx) = sink.tx_.as_mut() else {
        return Err(RingError::Write("写端已关闭".to_string()));
    };
    tx.send(data.to_vec())
        .await
        .map_err(|_| RingError::Write("读端已关闭".to_string()))
}

/// 读一段字节。
///
/// - 通道为空**且写端仍开着**：等待，直到有数据（或写端关闭）；
/// - 写端已关闭且已读空：返回 `None`（EOF）。
pub async fn read_some(source: &mut Source, max: usize) -> Option<Vec<u8>> {
    if source.pending_.is_empty() {
        source.pending_ = source.rx_.next().await?;
    }
    let count = source.pending_.len().min(max.max(1));
    let rest = source.pending_.split_off(count);
    let bytes = core::mem::replace(&mut source.pending_, rest);
    Some(bytes)
}

/// 关闭写端：置 EOF 并让读者读完残留后拿到 `None`。
pub fn close(sink: &mut Sink) {
    if let Some(mut tx) = sink.tx_.take() {
        tx.close_channel();
    }
}

#[cfg(test)]
mod tests_ {
    use super::*;

    /// 测试有序往返：写进去多少就读出来多少，且字节序一致。
    /// Test that bytes written to the bounded pipe come back in order.
    ///
    /// - 手段：建一个 8 字节容量的管道，按 2 字节一段写入 200 字节，同时用
    ///   `compio::runtime::spawn` 起一个读者按 7 字节一段反复读。
    /// - 判断：读到的字节与写入的完全一致、总数相等——这同时验证了
    ///   「通道满时写者等待」与「读者继续读就释放写者」。
    ///
    /// - Method: build an 8-byte pipe, write 200 bytes in 2-byte pieces while a
    ///   spawned reader drains 7 bytes at a time.
    /// - Assert: the collected bytes equal the payload exactly.
    #[compio::test]
    async fn bytes_round_trip_through_a_full_pipe_() {
        let (mut sink, mut source) = pipe(8).await.expect("应当能建管道");
        let payload: Vec<u8> = (0..200u32).map(|index| index as u8).collect();

        let expected = payload.clone();
        let reader = compio::runtime::spawn(async move {
            let mut collected = Vec::new();
            while collected.len() < expected.len() {
                let Some(chunk) = read_some(&mut source, 7).await else {
                    break;
                };
                collected.extend_from_slice(&chunk);
            }
            collected
        });

        for piece in payload.chunks(2) {
            write_all(&mut sink, piece).await.expect("写应当成功");
        }
        close(&mut sink);

        let collected = reader.await.expect("读者任务不应当异常结束");
        assert_eq!(collected, payload);
    }

    /// 测试关闭写端之后读者能读完残留，然后拿到 EOF。
    /// Test that a closed writer yields the remaining bytes and then EOF.
    ///
    /// - 手段：写入 5 字节后就 `close`，然后连续 `read_some` 直到返回 `None`。
    /// - 判断：先读回全部 5 字节，再得到 `None`。
    ///
    /// - Method: write 5 bytes, close, then read until `None`.
    /// - Assert: the 5 bytes come back before EOF.
    #[compio::test]
    async fn close_marks_eof_after_remaining_bytes_() {
        let (mut sink, mut source) = pipe(16).await.expect("应当能建管道");
        write_all(&mut sink, &[1, 2, 3, 4, 5])
            .await
            .expect("写应当成功");
        close(&mut sink);

        let mut collected = Vec::new();
        while let Some(chunk) = read_some(&mut source, 8).await {
            collected.extend_from_slice(&chunk);
        }
        assert_eq!(collected, vec![1, 2, 3, 4, 5]);
    }

    /// 测试容量不合法会被拒绝，而不是 panic。
    /// Test that an invalid capacity is rejected instead of panicking.
    ///
    /// - 手段：用容量 1（低于下限 2）建管道。
    /// - 判断：返回 `RingError::Build`。
    ///
    /// - Method: build a pipe with capacity 1.
    /// - Assert: `RingError::Build` is returned.
    #[compio::test]
    async fn invalid_capacity_is_rejected_() {
        let outcome = pipe(1).await;
        assert!(
            matches!(outcome, Err(RingError::Build(_))),
            "容量 1 应当被拒绝"
        );
    }
}
