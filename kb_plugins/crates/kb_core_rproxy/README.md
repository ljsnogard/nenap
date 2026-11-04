# kb_core_rproxy

`kb_core` 的**局域网应用层网关**。

```text
远程客户端 ──TCP──► kb_core_rproxy ──UDS/MPTP──► kb_core（本机子进程）
                    （本进程）                    （子进程）
```

它做三件事：

1. 用 `kb_core_starter` 启动一个 `kb_core` 子进程，读它就绪通知里的本机
   socket 路径——这是**系统层握手**；
2. 用 `kb_svc_mptp_ipc::Client` 连上 `kb_core`；
3. 监听 TCP，把远程客户端发来的 `RequestEnvelope` 转成 typed RPC，再把
   `ReplyEnvelope` 原样送回。它**不解释业务**——不懂工作区，也不懂会话。

## ⚠️ 没有鉴权、没有 TLS

它存在的目的是**局域网跨机测试**，所以缺省监听 `0.0.0.0:8788`，
**刻意不只绑 `127.0.0.1`**：

> 任何能访问到该端口的人都能读写知识库。

只想给本机用时显式传 `--listen 127.0.0.1:8788`。在受信网络之外使用之前
必须先做鉴权——这不是“以后顺手加”的东西。

## 跑起来

```bash
# 1. 网关（它会自己去启动 kb_core）
cargo run -p kb_core_rproxy -- \
  --listen 0.0.0.0:8788 \
  --runtime-dir /tmp/kb-rp/run \
  --storage-dir /tmp/kb-rp/data

# 2. 另一个终端：用现成的客户端 CLI 连上去
cargo run -p kb_client_conn_mgr --example connect -- tcp 127.0.0.1:8788
```

`connect` 会做应用层握手并列出工作区。远程路径的帧格式与客户端实现见
[`kb_core_rproxy_wire`](../kb_core_rproxy_wire/README.md) 与
[`kb_client_conn_mgr`](../../../kb_clients/crates/kb_client_conn_mgr/README.md)。

## 命令行

```text
kb-core-rproxy [选项]

--listen <地址:端口>   监听地址，缺省 0.0.0.0:8788
--kb-core <路径>       kb_core 可执行文件，缺省与本程序同目录下的 kb-core
--runtime-dir <目录>   传给 kb_core 的运行时目录（缺省 $XDG_RUNTIME_DIR/llm_kb）
--storage-dir <目录>   传给 kb_core 的数据目录（缺省 <运行时目录>/storage）
-h, --help             打印用法
```

`--kb-core` 找不到时程序会明确报错；Cargo 产物名是 `kb-core`（连字符）。

## 两层握手

| 层面 | 谁管 | 在本 crate 里的位置 |
| :--- | :--- | :--- |
| **系统层** | 传输实现 | `kb_core_starter`：启动 `kb_core`，读一行 `IpcReadyNotice` 拿 socket 路径 |
| **应用层** | 协议 | `main.rs`：**等真有远程客户端连上来**才发起一次 `Hello`；启动时不打扰 `kb_core` |

应用层握手只做一次（第一个远程客户端时），用来确认版本一致、上游真的能干活。
远程客户端随后自己发来的 `Hello` 会被原样转发，服务端会再答一次。

## TCP 帧

```text
[u32 BE 长度][1 字节种类][MessagePack 载荷]
     长度 = 种类 + 载荷的字节数
     种类：0 = 请求（上行）；1 = 应答（下行）；2 = 事件（下行，预留）
     载荷：RequestEnvelope / ReplyEnvelope / Event 的 denco MessagePack 编码
```

- 用 `mp_denco`，与本机 MPTP body、本地存储同一套 denco 编解码；
- 单帧载荷上限 8 MiB；
- 帧格式本身在 [`kb_core_rproxy_wire`](../kb_core_rproxy_wire/README.md)，
  服务端与远程客户端共用。

## 背压：上行有界管道

```text
TCP 读半 ──► futures_channel::mpsc（64 KiB 有界）──► 帧解码 ──► UDS/MPTP ──► 应答 ──► TCP 写半
   feed_uplink                                    forward_uplink
```

- 通道满了写端就等，于是读侧不再从 socket 取数据，TCP 窗口自然关上；
- 下行是严格的一问一答，攒不出队列，暂不做额外背压；
- 收尾：转发侧无论因为什么收工，都先发**中止信号**让读侧任务整体被丢弃，
  而不是指望有界通道自己解开。

`ring_` 用 `futures_channel::mpsc` 而不是环形缓冲：只需要“有界 + 异步等待 +
关闭后读到 EOF”，不涉及 `unsafe` 与段搬运。细节见 `src/ring_.rs` 的模块文档。

## 当前能力边界

| 限制 | 说明 |
| :--- | :--- |
| 一次只服务一个远程客户端 | 上一个断开才接受下一个；并发要等 `kb_core` 侧也能并发服务连接 |
| 事件还没转发 | 服务端目前不推事件；帧格式已经留好种类位 |
| 背压只做在上行 | 下行一问一答，暂时不需要 |
| 无鉴权、无 TLS | 见上 |

## 验证

```bash
cargo test -p kb_core_rproxy
```

3 个命令行解析测试 + 3 个有界管道测试。

## 相关文档

- [`kb_core_starter`](../kb_core_starter/README.md)：启动 `kb_core` 并等 socket 路径；
- [`kb_core_rproxy_wire`](../kb_core_rproxy_wire/README.md)：TCP 帧格式；
- [`kb_svc_mptp_ipc`](../kb_svc_mptp_ipc/)：本机 UDS + MPTP；
- [`kb_client_conn_mgr`](../../../kb_clients/crates/kb_client_conn_mgr/README.md)：远程客户端。
