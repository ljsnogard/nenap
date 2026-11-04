# `kb_admin_desktop`：连接管理菜单、配置向导、文件选择器

- 日期：2026-11-05 03:28
- 状态：**本轮实施记录**。
- 范围：
  1. 配置界面里已有连接方式的**悬停齿轮菜单**：测试连接 / 删除配置；
  2. 首次连接 / 空配置时的**三步配置向导**，解释 kb-core、运行时目录、存储目录；
  3. 路径输入改为**系统文件浏览器**（`file_selector`）。
- 前置：
  - [`kb_admin_desktop-20261105-0215.md`](kb_admin_desktop-20261105-0215.md)：启动页与 `last_used`；
  - [`kb_admin_desktop-20261105-0233.md`](kb_admin_desktop-20261105-0233.md)：便携 base、存储目录无默认值。

---

## 0. 结论

| 问题 | 结论 |
| :--- | :--- |
| 怎么删旧配置 | 连接方式行悬停出齿轮 →「删除配置」；删除前确认，正在用会断开 |
| 怎么测试连接 | 同一个齿轮 →「测试连接」；真实两级握手后立刻断开，不写 `CURRENT` / 不改 `last_used` |
| 什么时候跑向导 | 配置文件不存在、**或 `connections` 为空**；空列表是合法配置，不再当成坏配置 |
| 向导几步 | 3 步：选方式 + 解释路径 → 填参数 → 确认 / 测试 / 保存并连接 |
| 路径怎么选 | `kb-core` 用 `openFile()`；运行时 / 存储目录用 `getDirectoryPath()`（官方 `file_selector`） |
| 连接期间能测试吗 | 不能；界面直接提示「先断开当前连接再测试」，避免 `local-launch` 动到现有 socket |

---

## 1. 配置：允许空 `connections`

之前 `ClientConfig::validate()` 拒绝空列表（`NoConnections`）。但用户明确要求
“配置文件没有连接项 = 运行配置向导的信号”，所以：

- `connections: []` 现在是**合法配置**；
- `startup_connection()` / `default_connection()` 仍然返回
  `ConfigError::NoConnections`——它们是“选不出默认项”，不是“文件坏了”；
- `ConnectionController.initialize()` 看到空列表时置 `firstRun = true`；
- `saveConfig` 保存空列表后也把 `firstRun` 置真，并 `notifyListeners()`；
- 删除最后一条配置后，主界面保持离线，下一次启动 / 启动页会运行向导。

`ConfigError::NoConnections` 的文档同步改成“空配置合法，但选不出默认项”。

---

## 2. 测试连接：`test_connection`

FRB 新增 `test_connection(profile) -> ConnectReport`：

```text
connect_once_(profile)
  ├─ Ok(client)  → 返回报告，client 在函数结束时 drop
  └─ Err(error)  → 返回失败报告
```

- **不写 `CURRENT`**：当前连接不受影响；
- **不写配置**：`last_used` 仍由 Dart 侧的 `connect` 负责；
- `local-launch` 起来的子进程随 `client` 被丢弃而结束；
- 与 `connect_to` 共用 `connect_once_`，避免两套握手代码。

Dart 侧 `ConnectionController.testConnection` 在**已连接时直接拒绝**：测试
`local-launch` 会动到运行时目录里的 socket，先断开再测才不会干扰现有会话。
这个限制写进了控制器与 README。

---

## 3. 公共表单与配置对话框

把原来长在 `connection_dialog.dart` 里的字段抽成
`lib/src/widgets/connection/connection_form.dart`：

- `ConnectionForm`：主机名 + kind 选择 + 三种 kind 的字段 + 必填校验；
- `ConnectionFormState` 通过 `GlobalKey` 暴露 `draft()` / `validate()` /
  `fillFrom()` / `resetToNew()` / `setKind()`；
- 对话框与向导共用同一份字段，不再各写一套。

`connection_dialog.dart` 现在负责：

- 已有连接方式列表；点一行填进表单；
- 「新建」按钮；
- 行悬停的齿轮菜单：`showMenu` 弹「测试连接 / 删除配置」；
- 删除确认（会写明“正在使用会断开”）；
- 空列表时显示「打开配置向导」入口。

齿轮用 `showMenu` 而不是 `PopupMenuButton`：行在鼠标移向菜单时会退出 hover，
`PopupMenuButton` 会随图标一起从树里消失；`showMenu` 的 future 独立于按钮是否
还在屏幕上，回调更稳。

---

## 4. 三步配置向导

`lib/src/widgets/connection/connection_wizard.dart`：

```text
第 1 步  选择连接方式
  local-launch / local-attach / tcp 三张卡片
  + 解释 kb-core、运行时目录、存储目录

第 2 步  填写参数
  复用 ConnectionForm；路径字段带文件 / 目录选择
  必填校验失败留在本步

第 3 步  确认并连接
  摘要 + 「测试连接」+「保存并连接」
```

- 表单从第 1 步起就一直在树里，第 3 步用 `Offstage` 收起而不是移出树：
  否则切换步骤会把 `ConnectionFormState` 丢掉，`draft()/validate()` 拿不到；
- 首次运行 / 空配置时启动页自动打开向导；连接对话框的空状态也能打开；
- 向导的「测试连接」只测当前草稿，不保存配置；
- 「保存并连接」先 `saveConfig` 再 `connect`，成功 `pop(true)`。

---

## 5. 文件 / 目录选择

新增依赖：`file_selector: ^1.1.0`（Flutter 团队维护的官方插件）。

`lib/src/widgets/common/dsw_path_field.dart`：

- `DswPathField` = `DswLabeledField` + 右侧「选择文件… / 选择目录…」按钮；
- 文件走 `openFile()`，目录走 `getDirectoryPath()`；
- 用户取消返回 `null`，输入框保持原值；
- 插件异常用 `ScaffoldMessenger` 提示；
- `chooser` 参数允许 widget 测试注入假选择器，避免真的弹系统对话框。

表单里的三个路径：

| 字段 | 选择方式 |
| :--- | :--- |
| `kb-core 路径` | `openFile()` |
| `运行时目录` | `getDirectoryPath()` |
| `存储目录` | `getDirectoryPath()` |

---

## 6. 验证

| 项 | 命令 | 结果 |
| :--- | :--- | :--- |
| 配置 crate | `cargo test -p kb_client_config` | **12 单元 + 2 文档**全绿 |
| 连接管理器 | `cargo test -p kb_client_conn_mgr` | 3 单元 + 8 集成 + 1 文档全绿 |
| starter | `cargo test -p kb_core_starter` | 2 单元 + 7 集成 + 3 文档全绿 |
| 静态检查 | `cargo clippy -p kb_client_config -p kb_client_conn_mgr -p kb_core_starter --all-targets` | 无告警 |
| 全 workspace | `cargo check --workspace` | 通过 |
| FRB 重生成 | `flutter_rust_bridge_codegen generate` | 生成 `testConnection` |
| Dart 静态检查 | `flutter analyze` | **No issues found** |
| Dart 测试 | `flutter test` | **64 项通过、1 项跳过** |
| Linux 构建 | `flutter build linux --debug` | 通过（含 `file_selector_linux` 注册） |

新增的界面 / 状态用例：

1. 配置向导能走完首次连接（含空存储被拦下）；
2. 连接方式悬停齿轮 → 测试连接；
3. 连接方式悬停齿轮 → 删除配置；
4. 删除最后一条配置后回到首次运行状态；
5. 连接表单提供「选择文件… / 选择目录…」；
6. 坏配置不会被当成空配置去打开向导。

> `file_selector` 的平台实现只在 Linux 上实测构建；macOS / Windows 没有实机工具链，
> 用的是同一套官方插件，第一次在那些平台构建时要留意插件注册（Flutter 会自动做）。

---

## 7. 边界 / 开放事项

1. **测试连接在已连接时被禁用**：`local-launch` 会动运行时目录里的 socket；
   而且它会真的起一个 `kb_core`——如果本机已有另一个实例在用同一个存储目录，
   测试进程也会打开那份数据。做“并行测试”需要给测试单独一个临时运行时 / 存储目录。
2. **向导只覆盖首次 / 新建**：编辑已有连接仍走连接对话框；两者共用
   `ConnectionForm`，字段行为一致。
3. **删除的是配置，不是服务器数据**：`kb_core` 的工作区 / 会话 / 存储目录不会
   被删；只从客户端配置里移除连接方式。
4. **file_selector 的过滤**：`kb-core` 没有扩展名过滤（避免过滤掉自定义命名的
   可执行文件）；用户需要自己选对文件。
5. **`kb-core` 查找**仍只有显式路径 + App 同目录（上一轮的决定），没有 PATH /
   配置目录兜底。

---

## 8. 相关文档

- [`kb_client_config`](../../kb_clients/crates/kb_client_config/README.md)：空列表合法、路径规则；
- [`kb_admin_desktop`](../../kb_clients/kb_admin_desktop/README.md)：启动页、向导、连接管理；
- [`kb_admin_desktop-20261105-0233.md`](kb_admin_desktop-20261105-0233.md)：便携 base 与存储目录无默认值；
- `file_selector`：<https://pub.dev/packages/file_selector>。
