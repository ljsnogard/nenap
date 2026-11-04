// kb_admin_desktop 的入口。
//
// 应用结构与 DSH v0.1.5-rc 对齐（左侧工作区列表 + 新会话、中间对话、右侧工作区
// 文件浏览器）。
//
// 启动顺序：
//
// 1. `LocalStore.open()` 读出上一次的界面偏好；
// 2. `AppController` 用这份快照构造（布局 / 主题 / LLM 服务）；
// 3. `RustLib.init()` 初始化 flutter_rust_bridge —— 连接 kb_core 全靠它，
//    失败时界面照常起来，只是连不上；
// 4. `ConnectionController.initialize()` 读客户端自己的连接配置，但**不自动连接**：
//    界面先显示全屏启动页，默认选中「最近一次使用」的那条，按配置的秒数自动执行（默认 10 秒）；
//    没有配置过时由启动页打开连接对话框让用户输入；
// 5. 连接成功（或用户点「离线进入」）之后才进三栏主界面；连接与工作区 / 会话
//    数据都来自 `kb_core`，`LocalStore` 只留界面偏好。

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:kb_admin_desktop/src/app.dart';
import 'package:kb_admin_desktop/src/rust/frb_generated.dart';
import 'package:kb_admin_desktop/src/services/kb_client_api.dart';
import 'package:kb_admin_desktop/src/services/local_store.dart';
import 'package:kb_admin_desktop/src/state/app_controller.dart';
import 'package:kb_admin_desktop/src/state/connection_controller.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final LocalStore store = await LocalStore.open();
  final AppController controller = AppController(store, snapshot: store.load());

  final ConnectionController connection = ConnectionController(
    const FrbKbClientApi(),
  );

  // 原生库初始化失败也照常起界面：连接那块会显示失败原因。
  unawaited(_bootstrap(connection));

  runApp(KbAdminApp(controller: controller, connection: connection));
}

/// 初始化原生库，然后读连接配置（**不自动连接**，交给启动页决定连谁）。
///
/// 它不阻塞 `runApp`：界面先出来（启动页显示加载态），配置读完后启动页自己
/// 开始倒计时或打开首次运行对话框。
Future<void> _bootstrap(ConnectionController connection) async {
  try {
    await RustLib.init();
  } catch (error) {
    debugPrint('RustLib 初始化失败，连接 kb_core 将不可用: $error');
  }
  await connection.initialize();
}
