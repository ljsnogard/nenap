# kb_admin_desktop：客户端设计原理

- 日期：2026-11-05 03:45
- 状态：**当前实现说明**。操作 / 运行说明见
  [`kb_clients/kb_admin_desktop/README.md`](../kb_clients/kb_admin_desktop/README.md)。

---

## 0. 分层

```text
Flutter UI（Dart）
  └─ KbClientApi（lib/src/services/kb_client_api.dart）
       └─ FRB 生成的绑定（lib/src/rust/）
            └─ rust/src/api/kb.rs：扁平 DTO + 全局连接
                 ├─ kb_client_config：连接配置（TOML）
                 └─ kb_client_conn_mgr：连接管理
                      ├─ kb_core_starter：本机启动
                      ├─ kb_svc_mptp_ipc：本机 UDS
                      └─ TcpClient：跨机（经 rproxy）
```

界面**不直接**调用生成的 FRB 代码，而是经过 `KbClientApi`：widget 测试可以注入
假实现，完全不初始化原生库。FRB 层只做 DTO 翻译与“持有当前连接”，真正逻辑在
两个客户端 crate 里。

---

## 1. 启动页

`kb_admin_desktop` 启动时**不自动连接**：

```text
main()
  ├─ LocalStore：界面偏好
  ├─ ConnectionController.initialize()
  │    └─ kb_client_config：读配置（不连接）
  │         ├─ 没有 / 连接列表为空 → firstRun = true，启动页打开配置向导
  │         └─ 有 → startupProfile = last_used → default → 第一条
  └─ 启动页
       ├─ 默认选中 startupProfile，按配置的秒数自动 connect（默认 10 秒）
       ├─ 用户任何指针操作（点击 / 滚动）取消倒计时
       └─ 失败停在启动页报错；可改选、重试或「离线进入」
```

- **只有连接成功（或用户选择离线进入）才进三栏主界面**；之后断开 / 重连失败
  不会把用户赶回启动页；
- 启动页按传输分组：本机 UDS（启动 / 附着）与远程 TCP；
- 「最近一次」在用户点选 / 尝试连接时写回 `last_used`，不是等连接成功。

---

## 2. 连接配置

TOML，由 `kb_client_config` 管理：

```toml
version = 1
startup_countdown_seconds = 10  # 启动页自动连接倒计时；0 = 关闭
default = "本机"      # 用户偏好（可省）
last_used = "实验室"  # 最近一次启动 / 连接用的（客户端写回，可省）

[[connections]]
name = "本机"
kind = "local-launch"
kb_core = "/usr/local/bin/kb-core"
runtime_dir = "/run/user/1000/llm_kb"
storage_dir = "/home/me/.local/share/kb_admin_desktop/storage"

[[connections]]
name = "已在本机跑着的"
kind = "local-attach"
runtime_dir = "/run/user/1000/llm_kb"

[[connections]]
name = "实验室"
kind = "tcp"
address = "192.168.1.5:8788"
```

### 2.1 三种 `kind`

| kind | 系统层怎么走 | 说明 |
| :--- | :--- | :--- |
| `local-launch` | `kb_core_starter::start` 起 `kb-core` 子进程，等 socket 路径，再连 UDS | 子进程由 `KbClient` 持有，`KbClient` 被丢弃时结束 |
| `local-attach` | 直接在运行时目录里找 `kb-*.sock` 连上 | 只连已经在跑的本机实例，不启动新进程 |
| `tcp` | 连 `kb_core_rproxy` 的 TCP 端口 | 跨机；网关无鉴权、无 TLS，仅受信网络 |

**本机启动只能经过 `kb_core_starter`**，没有旁路。

### 2.2 路径

| 路径 | 规则 |
| :--- | :--- |
| 配置文件 | `KB_ADMIN_DESKTOP_CONFIG` → `KB_ADMIN_DESKTOP_HOME/config.toml` → 平台配置目录 |
| 运行时目录 | 便携模式 `<base>/run` → `$XDG_RUNTIME_DIR/llm_kb` → 系统临时目录 |
| 存储目录 | **没有默认值**；`local-launch` 必须让用户明确选一个持久目录 |
| `kb-core` | 配置里的显式路径；首次运行的预填值只猜“App 同目录” |

- 便携模式 `KB_ADMIN_DESKTOP_HOME=<base>` 是显式 opt-in；
- 普通模式不把配置目录当运行时目录——socket 应属于 `$XDG_RUNTIME_DIR`；
- 存储目录刻意不默认：把知识库放进 tmpfs（`$XDG_RUNTIME_DIR`）会在重启后消失。

### 2.3 空列表 = 配置向导信号

`connections: []` 是**合法配置**，不是坏配置。启动页看到它时自动打开配置向导；
删除最后一条连接方式也会回到这个状态。`ConfigError::NoConnections` 只表示
“选不出默认 / 上一次”，不表示文件坏了。

`save` 先写 `.tmp` 再 `rename`，读到的要么是旧内容、要么是新内容。

---

## 3. 配置向导与连接管理

### 3.1 三步向导

```text
第 1 步  选择连接方式
         本机启动 / 本机附着 / 远程 TCP 三张卡片
         + 解释 kb-core、运行时目录、存储目录
第 2 步  填写参数
         复用 ConnectionForm；路径带文件 / 目录选择；必填校验
第 3 步  确认并连接
         摘要 + 测试连接 + 保存并连接
```

- 表单从第 1 步起常驻（第 3 步用 `Offstage` 收起），否则切步会丢掉
  `ConnectionFormState`，`draft()/validate()` 拿不到；
- 首次运行 / 空配置自动打开；连接对话框的空状态也能打开。

### 3.2 连接管理

- 已有连接行悬停出现齿轮：**测试连接** / **删除配置**；
- 测试连接走 FRB `test_connection`：真实两级握手后立刻丢弃客户端；
  **不写 `CURRENT`、不写 `last_used`**；
- 已连接时禁止测试：`local-launch` 会动运行时目录里的 socket，先断开再测；
- 删除最后一条后变成空配置；正在使用的连接会先断开；
- 路径字段用官方 `file_selector`：`kb-core` 用 `openFile()`，目录用
  `getDirectoryPath()`。

### 3.3 为什么 FRB 层是同步 + `block_on`

`kb_client_conn_mgr` 的接口是异步的，但 `rust/src/api/kb.rs` 用
`block_on` 把它同步化：

- flutter_rust_bridge 2.13 为 `async fn` 生成的 `wrap_async` 在当前 nightly 上
  会因 HRTB 限制编译不过；
- FRB 会把同步函数放到自己的工作线程池，Dart 侧仍拿 `Future`，界面不被阻塞；
- 真正的阻塞等待（起进程、连 socket）本来就在专职线程上。

等 FRB 升级到兼容版本后可以改回 `async fn`，Dart API 形状不变。

---

## 4. 连接生命周期

- 同一时刻只有**一条活动连接**；左上角切换连接 = 替换旧客户端；
- `local-launch` 的子进程由 `KbClient` 持有，替换 / 断开 / 丢弃时结束；
- 启动页倒计时失败不自动重试；`startupFinished` 一旦为真，主界面里的失败只显示
  错误；
- 「离线进入」进入主界面但不连接，沿用未连接时的本地数据兜底。

**多连接（同时保持多条、各自缓存）没有做**。

---

## 5. 工作区 / 会话 / 提问

- 工作区 / 会话列表与正文都来自 `kb_core`，客户端不镜像存储；
- 新建工作区只提交名字 + 路径；路径是 **`kb_core` 所在主机上**的目录；
- 「新会话」是客户端**草稿**：不立刻发 `CreateSession`，等用户发第一条消息时
  用这条消息作为首条 `Turn` 建会话（名字由服务端从问题推导），再用同一个
  `turn_id` 调 `Ask`；服务端去重，问题不会记两遍；
- 空会话（没有消息、只有默认名）由服务端拒绝落盘；
- 改名只改展示名，不碰磁盘目录；
- `Ask` 是同步一问一答（`kb_core` 里是临时模拟 LLM，逆序输出），返回提问之后的
  完整会话。

---

## 6. 当前边界

1. **流式生成 / 事件 / `Cancel`** 没做；`Ask` 是同步一问一答。
2. **真正的多连接**没做；只有切换活动连接。
3. **工作区改路径**没做；只有改名。
4. **文件浏览器内容**只有骨架，等 `kb_core` 的目录浏览域。
5. **Windows / macOS** 没有实机验证；Linux 已构建 / 测试。

---

## 7. 验证

```bash
cargo test -p kb_client_config
cargo test -p kb_client_conn_mgr
( cd kb_clients/kb_admin_desktop/rust && cargo check )
( cd kb_clients/kb_admin_desktop && flutter analyze && flutter test )
```

当前结果：`kb_client_config` 13 单元 + 2 文档；`kb_client_conn_mgr` 3 单元 +
8 集成 + 1 文档；Flutter **65 项通过、1 项跳过**；`flutter build linux --debug`
通过。

---

## 8. 相关文档

- [`kb_admin_desktop/README.md`](../kb_clients/kb_admin_desktop/README.md)：操作 / 运行说明；
- [`kb_client_config/README.md`](../kb_clients/crates/kb_client_config/README.md)：配置格式与路径；
- [`kb_client_conn_mgr/README.md`](../kb_clients/crates/kb_client_conn_mgr/README.md)：三种连接方式；
- [`kb_core_starter/README.md`](../kb_plugins/crates/kb_core_starter/README.md)：本机启动与子进程归属；
- [`kb_svc_mptp_ipc-20261105-0345.md`](kb_svc_mptp_ipc-20261105-0345.md)：本机传输。
