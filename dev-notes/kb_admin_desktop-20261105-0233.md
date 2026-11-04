# `kb_admin_desktop`：便携 base、存储目录不再设默认值

- 日期：2026-11-05 02:33
- 状态：**本轮实施记录**；修订
  [`kb_admin_desktop-20261105-0215.md`](kb_admin_desktop-20261105-0215.md) 里的路径默认值。
- 范围：
  1. 确认 `kb_core_starter` 的形态（库 / 独立进程）；
  2. 配置文件目录是否作为默认运行时 / 启动目录；
  3. 新增 `KB_ADMIN_DESKTOP_HOME` 便携模式；
  4. **存储目录不再有默认值**，首次运行必须由用户明确选择。

> **后续 UI 补充**：[`kb_admin_desktop-20261105-0328.md`](kb_admin_desktop-20261105-0328.md)
> 增加了连接管理菜单、配置向导与文件选择器；路径策略以本文件为准。

---

## 0. 结论

| 问题 | 结论 |
| :--- | :--- |
| `kb_core_starter` 是库还是进程 | **目前只有库**；桌面与 rproxy 都链接它，由调用进程 spawn `kb-core` |
| 是否加独立 starter 进程 | **不加**；保持纯库，`Launched` 的 Drop 继续负责子进程收尾 |
| 默认目录策略 | **XDG 默认 + 可选便携 base**（`KB_ADMIN_DESKTOP_HOME`） |
| 便携 base 下的布局 | `<base>/config.toml`、`<base>/run/`；存储仍由用户选 |
| 存储目录默认值 | **没有**；`local-launch` 必须显式填一个持久目录 |
| `kb-core` 查找 | 配置里的**显式路径**优先；首次运行的建议值只猜“App 同目录” |
| 配置文件优先级 | `KB_ADMIN_DESKTOP_CONFIG` → 便携 base → 平台配置目录 |

---

## 1. `kb_core_starter` 的现状（核对结果）

`kb_plugins/crates/kb_core_starter`：

- 只有 `src/lib.rs`，**没有 `[[bin]]` / `src/main.rs`**；
- 内部用 `std::process::Command` 起 `kb-core` 子进程；
- 现有两个调用方都是**链接库**：

```text
kb_core_rproxy 进程
  └─ kb_core_starter (lib) ──spawn──► kb-core 子进程

kb_admin_desktop 进程
  └─ kb_client_conn_mgr (lib)
       └─ kb_core_starter (lib) ──spawn──► kb-core 子进程
```

所以“桌面启动一个 `kb_core_starter` 进程、再由它启动 `kb_core`”**不是现状**；
现状是桌面在自己的进程里调用 starter 库。两者都满足“不能绕开 starter”。

**为什么不改成独立进程**：现在子进程的所有权在 `Launched` 上，`Drop` 三条收场路径
（成功 / 取消 / future 被丢弃）都能结束它。如果 starter 是另一个进程，就要额外回答
“starter 退出后 `kb_core` 归谁、桌面怎么让它退出、握手通知怎么转交”等问题。
除非将来需要“starter 作为常驻 supervisor”，否则不值得拆。

---

## 2. 路径策略

### 2.1 三个目录语义不同

| 目录 | 默认 | 为什么 |
| :--- | :--- | :--- |
| 配置文件 | 平台配置目录 / 便携 base | 小、持久、可手改 |
| 运行时目录 | `$XDG_RUNTIME_DIR/llm_kb` / `<base>/run` | UDS socket、端点名字文件；通常是 tmpfs |
| 存储目录 | **无默认** | 知识库数据必须持久；不能默认落在 tmpfs |

之前 `default_storage_dir(runtime_dir) = runtime_dir/storage` 的问题是：普通模式下
`runtime_dir` 在 `$XDG_RUNTIME_DIR`（tmpfs），默认存储会随注销 / 重启消失。
本轮把它彻底去掉，而不是换一个“看起来合理”的默认。

### 2.2 便携模式 `KB_ADMIN_DESKTOP_HOME`

显式设置才生效：

```text
$KB_ADMIN_DESKTOP_HOME/
├── config.toml       # 配置文件
├── run/              # 默认运行时目录
└── data/             # 只是概念上的位置；存储目录仍由用户自己选
```

- 配置文件优先级：`KB_ADMIN_DESKTOP_CONFIG` > `$KB_ADMIN_DESKTOP_HOME/config.toml`
  > 平台配置目录；
- 默认运行时目录：有便携 base 用 `<base>/run`，否则仍走 XDG；
- **存储目录不会自动变成 `<base>/data`**：它没有默认值。

### 2.3 `kb-core` 查找

保持“配置里的显式路径 + App 同目录”：

- 用户保存的 `local-launch.kb_core` 是权威值；
- 首次运行的预填值仍由 `kb_core_starter::default_kb_core_path()` 猜“与本可执行文件
  同目录”；
- 猜不到时是空串，界面必须让用户填完整路径；
- 不引入 `KB_CORE_BIN` / `PATH` / 配置目录兜底（本轮明确不做）。

---

## 3. 实现

### 3.1 `kb_client_config::path`

- 新增 `PORTABLE_HOME_ENV = "KB_ADMIN_DESKTOP_HOME"` 与 `portable_home()`；
- `config_path()`：显式 → 便携 base → 平台目录；
- `default_runtime_dir()`：便携 base → XDG → 系统临时目录；
- **删除** `default_storage_dir()`（公开 API 移除）：不再提供任何存储默认值；
- 路径选择拆成纯函数 `config_path_from_` / `runtime_dir_from_`，便于确定性测试；
- 新增配置优先级、便携运行时目录两条单元测试。

### 3.2 `kb_client_config::Connection::LocalLaunch`

- `storage_dir` 文档明确“必填、没有默认值”；
- `Connection::local_launch` 的文档说明调用方必须给出；
- 校验逻辑本来就会拒绝空的 `storage_dir`，本轮不改语义。

### 3.3 `kb_client_conn_mgr::suggested_local_launch`

- 运行时目录仍用 `default_runtime_dir()`；
- **存储目录传空**，等界面让用户填；
- 新增单元测试 `suggested_local_launch_requires_an_explicit_storage_dir_`；
- 手工 CLI `examples/connect.rs` 的 `launch` 子命令从“存储目录可选”改为**必填**。

### 3.4 连接对话框

- 「存储目录」改名为「存储目录（必填）」，占位提示“自己选一个持久目录”；
- 预填值缺存储目录时显示警告；
- `_saveAndConnect` 之前先做一次用户可读的必填检查（kb-core 路径 / 运行时目录 /
  存储目录 / TCP 地址），不再依赖底层 `EmptyField` 错误文案；
- 新增 widget 测试：预填值缺存储目录时点「保存并连接」会被拦下，且不会发起
  `saveConfig` / `connect`。

### 3.5 后续修补：`kb-core` 与 `kb_core` 的误填

用户把 `kb-core` 误填成 `kb_core`（下划线），启动时得到 `KbCoreNotUsable`。
实际 Cargo 产物是 `target/debug/kb-core`（连字符）。这暴露的是错误提示不够直白：

- `LaunchError::KbCoreNotUsable` 的文案补上“Cargo 构建产物通常叫 kb-core（连字符），
  不是 kb_core”；
- 没有改查找逻辑：仍然只认用户填的显式路径（找不到就明确报错，不自动猜测）。

### 3.6 文档

- `kb_client_config` / `kb_admin_desktop` / 根 README 同步路径策略；
- 上一轮的 `kb_admin_desktop-20261105-0215.md` 加一条“路径默认值已被本轮修订”的说明。

---

## 4. 验证

| 项 | 命令 | 结果 |
| :--- | :--- | :--- |
| 配置 crate | `cargo test -p kb_client_config` | **11 单元 + 2 文档**全绿 |
| 连接管理器 | `cargo test -p kb_client_conn_mgr` | **3 单元 + 8 集成 + 1 文档**全绿（新增 1 个单元测试） |
| 静态检查 | `cargo clippy -p kb_client_config -p kb_client_conn_mgr --all-targets` | 无告警 |
| 全 workspace 编译 | `cargo check --workspace` | 通过 |
| Dart 静态检查 | `flutter analyze` | **No issues found** |
| Dart 测试 | `flutter test` | **58 项通过、1 项跳过**（新增 1 个 widget 测试） |
| Linux 构建 | `flutter build linux --debug` | 通过 |
| starter 错误文案修补 | `cargo test -p kb_core_starter` / `cargo clippy -p kb_core_starter --all-targets` | 2 单元 + 7 集成 + 3 文档全绿；无告警 |

> 本轮只改路径默认值与交互校验，没有改协议、FRB 函数签名或线格式。

---

## 5. 没做 / 开放事项

1. **便携 base 下没有自动的 `data/`**：存储目录仍要用户选；如果以后希望
   “一键使用 `<base>/data`”，可以给对话框加一个“使用建议值”按钮，但不能静默预填。
2. **`kb_core_starter` 的独立 CLI** 没加；如果以后需要手工起 `kb_core` 并打印 socket
   路径，可以加一个只用于运维的 `[[bin]]`，桌面仍链接库。
3. **`kb-core` 的查找**仍只有显式路径 + App 同目录；没有 `PATH` / 环境变量 /
   配置目录兜底。
4. **已有配置不受影响**：`storage_dir` 在文件里一直必填，旧值原样保留；本轮只改
   “首次运行建议值”和默认运行时目录。

---

## 6. 相关文档

- [`kb_client_config`](../../kb_clients/crates/kb_client_config/README.md)：路径优先级、便携模式、无存储默认；
- [`kb_admin_desktop`](../../kb_clients/kb_admin_desktop/README.md)：手动跑一遍与路径说明；
- [`kb_core_starter`](../../kb_plugins/crates/kb_core_starter/README.md)：库形态与子进程归属；
- [`kb_admin_desktop-20261105-0215.md`](kb_admin_desktop-20261105-0215.md)：上一轮的启动页与 `last_used`。
