# `kb_admin_desktop`：启动页选连接方式 + 「最近一次」持久化

- 日期：2026-11-05 02:20
- 状态：**本轮实施记录**。
- 范围：
  1. 进主界面前增加**全屏启动页**：选择「本机（Unix domain socket）」或「远程（TCP）」；
  2. 配置文件新增 `last_used`（最近一次启动 / 连接用的那条）；启动页默认选中它，
     **倒数 5 秒自动执行**；
  3. 没有配置过时由启动页让用户输入；连接失败停在启动页报错，不自动重试；
  4. 用户可「离线进入」主界面；
  5. 本机启动方式仍然**只经 `kb_core_starter`**，没有新增绕开它的入口。
> **路径默认值已被 [kb_admin_desktop-20261105-0233.md](kb_admin_desktop-20261105-0233.md) 修订**：
> 存储目录不再有默认值，并新增 `KB_ADMIN_DESKTOP_HOME` 便携模式。本文件保留当时的实施记录。

- 前置：
  - [`kb_svc_mptp_ipc-20261104-2129.md`](kb_svc_mptp_ipc-20261104-2129.md)：
    本机传输已是 UDS + `mptp_rpc`；远程是 `kb_core_rproxy` 的 TCP；
  - [`kb_svc_servo_ipc-20260917-1548.md`](kb_svc_servo_ipc-20260917-1548.md)：
    历史传输，只作背景；
  - `kb_clients/crates/kb_client_config/README.md`、`kb_client_conn_mgr/README.md`：
    本轮之前的连接配置与连接管理器。

---

## 0. 结论

| 问题 | 结论 |
| :--- | :--- |
| 「最近一次」存哪 | `config.toml` 新增可缺省的 `last_used`；`default` 保留为「用户偏好」 |
| 启动选择顺序 | `last_used` → `default` → 第一条 |
| 什么时候写 `last_used` | 用户点选连接方式时立即写；倒计时自动执行时在尝试连接前写（失败也会留下） |
| 启动页形态 | 独立全屏页面；连接成功才进三栏主界面；允许「离线进入」 |
| 倒计时 | 固定 5 秒；到点只连一次；用户任何指针操作取消倒计时 |
| 连接失败 | 停在启动页显示错误，**不自动重试** |
| 连接方式 | 三种 `kind` 不变：`local-launch` / `local-attach` / `tcp` |
| TCP 语义 | 连 `kb_core_rproxy`；`kb_core` 自身仍然只监听 UDS |
| 本机启动 | 沿用 `kb_client_conn_mgr` → `kb_core_starter::start`，没有旁路 |
| 配置版本 | 仍为 `CONFIG_VERSION = 1`：新字段可缺省，旧文件直接能读 |

---

## 1. 起点核对（为什么不是“从零加连接能力”）

动手前 HEAD `5a791a4` 已经具备：

- `kb_client_config`：TOML、三种 `kind`、平台路径、原子写、`default`；
- `kb_client_conn_mgr`：`local-launch` 经 `kb_core_starter`、`local-attach` 扫运行时目录、
  `tcp` 连 rproxy；
- FRB 薄层与连接对话框：能读 / 写配置、连接、列工作区与会话。

但启动体验是：

```text
有配置  → initialize() 直接静默连 default（没有启动页、没有倒计时）
无配置  → 弹一次首次运行对话框
```

而且 `default` 只在连接对话框保存时写；主界面左上角切换连接**不会**写回。
所以本轮真正缺的是：**启动页 + 最近一次持久化 + 把“选择”与“加载”分开**。

---

## 2. 拍板记录（2026-11-05）

| # | 问题 | 选择 | 理由 / 代价 |
| :--- | :--- | :--- | :--- |
| 1 | 配置里怎么记“最近一次” | **新增 `last_used`**，`default` 保留 | 语义分开：用户偏好 vs 上次用的；老配置缺字段时回退到 `default`，不必升版本 |
| 2 | 什么时候写 | **用户点选 / 尝试连接时立即写** | 即使这次连不上，下次启动仍记得用户最后选的是哪条；失败不静默改回旧值 |
| 3 | 启动页与主界面 | **独立全屏页 + 允许“离线进入”** | 启动时目标明确；同时保留未连接时用本地数据兜底的能力 |
| 4 | 倒计时与失败 | **到点只连一次；失败回启动页；用户操作取消倒计时** | 远程不可达时不会无限重试；把决定权交回用户 |
| 5 | 连接方式 | **保留三种；TCP 经 rproxy；启动页按本机 / 远程分组** | 不删已有能力；“本机”里再分启动 / 附着，正好对应 UDS 的两种用法 |
| 6 | 本机启动 | **继续只走 `kb_core_starter`** | 子进程归属、取消、future 被丢弃三条收场路径都由 starter 的守卫处理 |

> 第 1 条是**公开配置格式变更**（`kb_client_config::ClientConfig` 多一个字段）。
> 采用可缺省字段、不升 `CONFIG_VERSION`：旧文件没有它时 `None`，
> 启动选择自动回退到 `default`，不会把老用户判成“坏配置”。

---

## 3. 实现

### 3.1 `kb_client_config`：字段、选择规则、校验

`ClientConfig`：

```toml
version = 1
default = "本机"      # 用户偏好（可省）
last_used = "实验室"  # 最近一次启动 / 连接用的（客户端写回，可省）
```

新增：

- `ClientConfig::startup_connection()`：`last_used` → `default` → 第一条；
- `ClientConfig::set_last_used(name)`：只接受已经存在的连接方式名；
- `ConfigError::UnknownLastUsed`：手改配置把 `last_used` 指到不存在的项时明确报错；
- `validate()` 同时校验 `default` 与 `last_used`。

`with_single` 仍然只写 `default`；首次保存的配置不强行写 `last_used`，
启动页回退到 `default` 即可。

### 3.2 FRB 层：只加两个小口子

- `ConfigView` 增加 `last_used_name`（空串 = 没有）；
- 新增 `remember_last_used(name) -> String`：读配置 → `set_last_used` → 原子写回；
  返回空串表示成功。配置文件还不存在时返回空串（首次运行还没保存，没东西可记）；
- `save_config` **保留**原文件里的 `last_used`（只要它指向的连接方式还在）：
  “保存配置”不等于“使用某条连接”，两者分开；
- 两个写入口共用 `CONFIG_LOCK`，避免 `读 → 改 → 写` 并发时后写覆盖前写。

### 3.3 `ConnectionController`：把“加载”与“连接”分开

| 变化 | 说明 |
| :--- | :--- |
| `initialize()` 不再自动连接 | 只读配置；有配置时把 `startupProfile` 算好，`phase` 保持 `idle` |
| 新增 `lastUsedName` / `startupProfile` | 启动页据此默认选中、显示「上次使用」标签 |
| 新增 `selectStartupProfile()` | 用户改选时立即写回 `last_used`，但不连接 |
| 新增 `connectStartupProfile()` | 连启动页当前目标（测试与“立即连接”共用） |
| `connect()` 开头先写 `last_used` | 倒计时自动执行也走这里；失败也已留下这次选择 |
| 新增 `enterOffline()` | 结束启动页，进主界面，不连接 |
| 新增 `reloadConfig()` | 配置文件坏掉时在启动页「重新加载」 |
| `startupFinished` | 连接成功或离线进入后为真；之后断开 / 重连失败不再回启动页 |

连接成功时 `_startupFinished = true`；主界面里的主机切换仍然可用，
`connect()` 同样会把新的那条写进 `last_used`。

### 3.4 启动页（`lib/src/widgets/startup/startup_screen.dart`）

```text
┌──────────────────────────────────────────────┐
│ [dns] 连接 kb_core                            │
│       工作区与会话由 kb_core 保管……            │
│                                              │
│ 本机（Unix domain socket）                     │
│  ● 本机   [本机·启动] [上次使用]                │
│  ○ 已在本机跑着的 [本机·附着]                   │
│                                              │
│ 远程（TCP）                                    │
│  ○ 实验室 [远程·TCP]                           │
│                                              │
│  [5 秒后自动连接「本机」] ───────────────       │
│  [连接] [取消倒计时] [新建/编辑连接式…]  离线进入 │
└──────────────────────────────────────────────┘
```

行为：

1. **默认选中** `last_used`（回退 `default` / 第一条），显示「上次使用」标签；
2. **倒计时**固定 5 秒（`StartupScreen.countdownDuration`），每秒刷新一次；
   到点调用 `connect(startupProfile)`，只尝试一次；
3. **任何指针操作**（`Listener.onPointerDown`）都取消倒计时；点选连接方式、
   点「立即连接」、打开对话框同理；
4. **失败**：停在启动页，红色条显示 `connection.error`；没有自动重试；
5. **首次运行**：配置不存在时自动打开既有的连接对话框（`firstRun: true`），
   用户取消后停在启动页，可以再次打开或「离线进入」；
6. **配置坏了**：显示错误 + 「重新加载」/「编辑连接方式…」/「离线进入」，
   不静默覆盖；
7. **「离线进入」**：调用 `enterOffline()`，主界面沿用未连接时的本地数据兜底。

### 3.5 `app.dart`：闸门替换

原来的 `_FirstRunGate`（有配置就自动连、没配置才弹对话框）换成 `_StartupGate`：

```dart
if (!connection.startupFinished) return StartupScreen(connection: connection);
return child; // AppShell
```

它只听 `ConnectionController`；`startupFinished` 一旦为真，之后主界面里的
断开 / 重连失败都不会把用户赶回启动页。

---

## 4. 验证

环境：Flutter 3.47.9 / Dart 3.13.7；Rust nightly 1.101。

| 项 | 命令 | 结果 |
| :--- | :--- | :--- |
| 配置 crate | `cargo test -p kb_client_config` | **11 单元 + 2 文档**全绿（新增启动选择顺序、旧配置兼容、`last_used` 校验） |
| 配置 crate 静态检查 | `cargo clippy -p kb_client_config --all-targets` | 无告警 |
| FRB 重新生成 | `flutter_rust_bridge_codegen generate` | 生成 `ConfigView.lastUsedName` 与 `rememberLastUsed` |
| 客户端 Rust 侧 | `(cd kb_clients/kb_admin_desktop/rust && cargo check)` | 通过 |
| Dart 静态检查 | `flutter analyze` | **No issues found** |
| Dart 测试 | `flutter test` | **57 项通过、1 项跳过**（golden 组照旧跳过） |
| Linux 构建 | `flutter build linux --debug` | 通过（启动页与 FRB 新字段编进原生库） |
| 变更文件 import 规范 | `python3 format_use_imports.py --check <kb.rs>` | 合规 |

新增 5 个界面用例 + 1 个状态机用例（启动目标回退顺序）：

1. 启动页默认选最近一次并 5 秒后自动连接；
2. 用户改选连接方式会取消倒计时（只写 `rememberLastUsed`，不连接）；
3. 连接失败停在启动页且不自动重试；
4. 启动页可以离线进入主界面；
5. 首次运行启动页自动打开连接对话框，取消后停在启动页。

> 仓库里另有 30 处**既有的** `format_use_imports.py` 违规（都在本轮未触碰的文件里），
> 按 `AGENTS.md` 的“先建回退点再跑 `--fix`”要求，本轮没有全仓自动修复。

---

## 5. 没做 / 开放事项

1. **倒计时时长不可配**：固定 5 秒；如果以后要可配，再加配置字段（会动公开格式）。
2. **`last_used` 是全局一条**，不按 `kind` 分别记“上次本机 / 上次远程”。
   需要的话可以在界面上做“本机组 / 远程组各自默认”，但那要再定字段形状。
3. **真正的多连接**仍然没做：启动页只决定“这一次连谁”，主界面里仍是切换活动连接。
4. **离线进入后的本地数据兜底**沿用旧逻辑；等本地那一套退役时，启动页的
   「离线进入」也要重新定义（可能改成“退出”或“仅浏览缓存”）。
5. **Windows / macOS 没有实机跑 GUI**（本机只有 Linux 工具链）；本轮只跑 Dart
   测试与静态检查，平台差异风险低（纯 Flutter 页面 + FRB 生成代码）。

---

## 6. 相关文档

- [`kb_client_config`](../../kb_clients/crates/kb_client_config/README.md)：`last_used` 与启动选择顺序；
- [`kb_admin_desktop`](../../kb_clients/kb_admin_desktop/README.md)：启动页与手动跑一遍；
- [`kb_client_conn_mgr`](../../kb_clients/crates/kb_client_conn_mgr/README.md)：三种连接方式的系统层实现；
- [`kb_core_starter`](../../kb_plugins/crates/kb_core_starter/README.md)：本机启动的子进程归属；
- [`kb_svc_mptp_ipc-20261104-2129.md`](kb_svc_mptp_ipc-20261104-2129.md)：UDS + mptp_rpc 与端点公布。
