# NENAP - Not Everything Neccessarily A Plugin

`nenap` 是一个内核用 Rust 开发、多进程运行插件、依靠 RPC 通信完成协作的个人知识库。

知识内容主要来自 LLM 服务生成的内容以及用户个人编辑整理，
最终由一个以 **Turso** 为载体的知识数据库持有，对外提供增删查改与搜索、检索能力。
其他能力（与 LLM 通信等）以**插件**的形式存在；GUI 是独立客户端。

本机进程之间走 **UDS + smux_v1 + mptp_rpc**；
需要跨机（比如局域网另一台机器）时，由 `kb_core_rproxy` 把来自网络的访问转成
同一套 typed RPC。GUI 是独立客户端，目前用 Flutter 开发。

---

## 1. 目录结构

```text
nenap/
├── kb_svc/crates/       知识库服务本身：主进程 + 协议
├── kb_plugins/crates/   插件与配套进程：LLM 插件、跨机网关、本机传输、启动器
├── kb_clients/          客户端：应用（Flutter）+ 界面之外的客户端 crate
├── dev-notes/           当前有效的设计与原理（过时记录直接删除）
├── AGENTS.md            代码修改纪律
├── Cargo.toml           workspace 定义与共用依赖
└── rust-toolchain.toml  工具链锁定
```

## 2. 各 crate 的分工

### `kb_svc/crates/` —— 服务端

| crate | 职责 | 状态 |
| :--- | :--- | :--- |
| `kb_core` | **主进程**（可执行文件 `kb-core`）：持有工作区与会话的存储、把按域 RPC 实现接上 UDS/MPTP、常驻服务客户端 | ✅ 可跑 |
| `abs_kb_svc` | **协议聚合层**：把各协议 crate 挂到稳定的 `abs_kb_svc::v1::desktop::*` 路径下，**自己不定类型** | ✅ |
| `abs_kb_svc_v1_desktop_rpc` | **RPC 契约**：`#[rpc]` 的四个按域 service、`Ack`、业务错误映射 | ✅ |
| `abs_kb_svc_v1_desktop` | **协议 v1 桌面端的数据**：`kb_admin_desktop` × `kb_core` 的 17 请求 / 13 应答 / 9 事件、应用层握手。**协议内容的唯一出处**（不含 RPC 方法契约） | ✅ |
| `abs_kb_core_handshake` | **系统层握手**：`kb_core` 用 `--handshake-prompt=stdio` 公布的 `IpcReadyNotice`（启动方据此找到本机 socket） | ✅ |
| `abs_llm` | LLM 的**语义抽象**：对话角色、增量输出、用量、能力集等与 provider 无关的词汇 | ✅ |

### `kb_plugins/crates/` —— 插件与配套进程

| crate | 职责 | 状态 |
| :--- | :--- | :--- |
| `kb_rig_llm_v1_agent` | 用 `rig` 直连 LLM 服务商，自己保留完整对话上下文 | 🚧 |
| `kb_rig_llm_v1_adapt` | rig 的原始数据 → `abs_llm::v1` 的转换 | 🚧 |
| `kb_core_rproxy` | **跨机网关**：启动一个 `kb_core`，自己监听 TCP，把远程访问者当作"MPTP 客户端"转发 | ✅ 可跑（**无鉴权、无 TLS**，仅用于受信网络） |
| `kb_core_rproxy_wire` | 网关的 **TCP 帧格式**（纯编解码）：网关与远程客户端共用同一份 | ✅ |
| `kb_core_starter` | 启动 `kb_core` 子进程并**异步**等它公布 socket 路径的可取消 future；网关与客户端共用 | ✅ |
| `kb_svc_mptp_ipc` | 协议的本机**传输实现**：UDS + `smux_v1` + `mptp_rpc`；客户端门面带宿主线程 | ✅ 可跑 |

### `kb_clients/crates/` —— 客户端侧基础设施

界面之外的客户端逻辑放这里，以便复用与单独测试。

| crate | 职责 | 状态 |
| :--- | :--- | :--- |
| `kb_client_config` | 客户端**自己的连接配置**（TOML）：三种连接方式、`last_used`（最近一次）、平台路径 / 便携 base、首次生成、原子写 | ✅ |
| `kb_client_conn_mgr` | **连接管理器**：按一条连接方式接上 `kb_core`（本机启动 / 本机附着 / 远程 TCP）+ 应用层握手 + 列工作区 / 会话 | ✅ |

### `kb_clients/` —— 客户端应用

| 目录 | 职责 | 状态 |
| :--- | :--- | :--- |
| `kb_admin_desktop` | Flutter 桌面客户端。Rust 侧（flutter_rust_bridge）接上配置与连接管理器：启动页 / 配置向导、连接管理、工作区 / 会话 / 正文 / 提问 | ✅ 主要流程可跑；流式生成与多连接未做 |

## 3. 一次请求怎么走

```text
本机：kb_admin_desktop
  │ ① 系统层：starter 读 IpcReadyNotice 得到 socket 路径（附着时直接扫目录）
  │ ② 应用层：Request::Hello → Reply::Hello
  ▼
  kb_svc_mptp_ipc::Client ──UDS/MPTP──► kb_svc_mptp_ipc::Listener
                                              │
                                      kb_core::ipc_::KbService  ← 四个按域 RPC 服务
                                              │
                                      kb_core::store_::Store    ← MessagePack 文件（将来换 Turso）
                                              │
      ◄──────── typed RPC 应答 ───────────────┘

跨机：远程客户端 ──TCP──► kb_core_rproxy ──UDS/MPTP──► kb_core
                             │ 帧 = [u32 长度][种类][MessagePack]
                             │ 上行有界通道做背压；信封 ↔ typed RPC
                             └ 启动 kb_core 时同样经 kb_core_starter 做系统层握手
```

- **协议**（数据、RPC 方法契约、握手语义）只在协议 crate 里定义——
  `abs_kb_svc_v1_desktop`（数据 + 应用层握手）、
  `abs_kb_svc_v1_desktop_rpc`（`#[rpc]` 方法契约与业务错误映射）与
  `abs_kb_core_handshake`（系统层）；`abs_kb_svc` 只是把它们聚合到稳定路径下。
  换传输只换实现 crate。
- 两条链路的细节：本机 IPC 见 `kb_svc_mptp_ipc` 的 crate 文档，
  跨机见 `kb_core_rproxy` 的 README。

### 客户端怎么选连接方式

客户端的连接方式是**它自己的配置**（`kb_core` 不参与）：

```text
启动 kb_admin_desktop
  ├─ kb_client_config：读配置
  │    ├─ 没有 → 启动页自动打开**配置向导**，问用户：启动本机 kb_core /
  │    │          连已在跑的本机 kb_core / 连远程网关 → 写回 TOML（原子写）
  │    │          （配置存在但连接列表为空，也是运行向导的信号）
  │    └─ 有   → 启动页默认选中 last_used（最近一次用的那条；旧配置回退到 default）
  │              按配置的秒数自动执行（默认 10 秒）；用户一操作就取消
  └─ kb_client_conn_mgr：按这条连接方式做两级握手，然后进主界面 / 列工作区、会话
     （失败停在启动页报错；也可以「离线进入」主界面）
```

三种 `kind`（`local-launch` / `local-attach` / `tcp`）的语义与时限见
`kb_clients/crates/kb_client_conn_mgr/README.md`。

路径默认：配置文件与运行时目录走 XDG（`~/.config/…`、`$XDG_RUNTIME_DIR/llm_kb`）；
设置 `KB_ADMIN_DESKTOP_HOME=<base>` 进入便携模式，同树下放 `config.toml` 与 `run/`。
**存储目录没有默认值**：`local-launch` 必须由用户明确选一个持久目录，避免把知识库
放进 tmpfs / 临时目录。

## 4. 当前状态

**已经跑通的**：UDS + `smux_v1` + `mptp_rpc` 上的工作区 / 会话增删查改、
应用层握手与版本校验、跨机网关的请求转发与上行背压、桌面端的启动页 / 配置向导 /
连接管理、同步一问一答的 `Ask`（临时模拟 LLM）。

**还没做的**：设置与目录浏览的 RPC、流式生成 / 事件 / `Cancel`、并发服务多客户端、
优雅退出、鉴权、以及把存储从本地 MessagePack 文件换成 Turso。
当前有效的设计原理见 `dev-notes/` 下的几份文档。

## 5. 构建环境

工具链由仓库根的 `rust-toolchain.toml` 锁定为 **nightly**：可取消 future 的宏
`gen_mcf2`、denco 派生与按域 RPC trait 都需要
`#![feature(impl_trait_in_assoc_type)]`，stable 编译不过。

`rustup` 会按该文件自动准备工具链与 `rustfmt` / `clippy`，无需手工 `rustup default`。

客户端 `kb_admin_desktop` 里的 flutter_rust_bridge 子项目由 Cargokit 驱动，
它不读 `rust-toolchain.toml`（`rustup run` 会覆盖工具链文件），因此另有一份
`kb_clients/kb_admin_desktop/rust/cargokit.yaml` 把工具链对齐到同一条通道。

客户端构建需要 Flutter SDK 与 `flutter_rust_bridge_codegen`（版本与
`rust/Cargo.toml` 一致）；Cargokit 驱动 Rust 侧编译。常用验证：

```bash
cargo test -p kb_client_config -p kb_client_conn_mgr -p kb_core
( cd kb_clients/kb_admin_desktop/rust && cargo check )
( cd kb_clients/kb_admin_desktop && flutter analyze && flutter test )
```

## 6. 文档在哪

- `kb_svc/crates/abs_kb_svc_v1_desktop/README.md`：协议数据、线格式与实现契约；
- `kb_svc/crates/abs_kb_svc_v1_desktop_rpc/README.md`：`#[rpc]` 方法契约与业务错误映射；
- `kb_svc/crates/abs_kb_core_handshake/README.md`：系统层握手（`IpcReadyNotice`）。
- `kb_svc/crates/abs_kb_svc/README.md`：聚合层的用途与“什么时候不必经过它”；
- `kb_svc/crates/abs_llm/README.md`：与 provider 无关的 LLM 语义抽象；
- `kb_svc/crates/kb_core/README.md`：主进程的操作手册（命令行、存储布局、实测命令）。
- `kb_plugins/crates/kb_core_rproxy/README.md`：跨机网关的用法与能力边界。
- `kb_plugins/crates/kb_core_starter/README.md`：启动 `kb_core` 并等 socket 路径的那个 crate。
- `kb_plugins/crates/kb_core_rproxy_wire/README.md`：跨机网关的 TCP 帧格式（服务端与客户端共用）。
- `kb_plugins/crates/kb_svc_mptp_ipc/README.md`：本机 UDS + MPTP 传输实现。
- `kb_clients/crates/kb_client_config/README.md`：客户端的连接配置（TOML）。
- `kb_clients/crates/kb_client_conn_mgr/README.md`：连接管理器（三种连接方式 + 查询）。
- `dev-notes/`：当前有效的设计与原理（过时记录直接删除）：
  - [`kb_svc_mptp_ipc-20261105-0345.md`](dev-notes/kb_svc_mptp_ipc-20261105-0345.md)：本机传输与错误映射；
  - [`kb_core-20261105-0345.md`](dev-notes/kb_core-20261105-0345.md)：存储与服务端原理；
  - [`kb_admin_desktop-20261105-0345.md`](dev-notes/kb_admin_desktop-20261105-0345.md)：客户端启动、路径、连接管理；
  - [`dev-notes/README.md`](dev-notes/README.md)：本目录的收录规则。
