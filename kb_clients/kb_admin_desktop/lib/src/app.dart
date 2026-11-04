// 应用根组件。
//
// 只做三件事：把 [AppController] 挂到组件树上、按主题模式装配 [MaterialApp]、
// 把三栏框架塞进 `Scaffold`。DSH 没有全局顶栏，所以这里也没有 `AppBar`——
// 每一栏自己负责自己的头部。

import 'package:flutter/material.dart';

import 'state/app_controller.dart';
import 'state/connection_controller.dart';
import 'theme/app_theme.dart';
import 'widgets/app_shell.dart';
import 'widgets/startup/startup_screen.dart';

/// 知识库管理客户端。
class KbAdminApp extends StatelessWidget {
  /// 构造应用。
  const KbAdminApp({super.key, required this.controller, this.connection});

  /// 应用状态。
  final AppController controller;

  /// 与 `kb_core` 的连接状态；为 `null` 时不接连接（widget 测试用）。
  final ConnectionController? connection;

  @override
  Widget build(BuildContext context) {
    // 整个应用挂在一个 [AnimatedBuilder] 上：状态里的任何变化（布局、工作区、
    // 消息、主题）都会重建三栏。对当前这个体量足够简单可靠；等接入流式回答后，
    // 应当把高频变化的会话内容收窄到各自的倾听者。
    return AnimatedBuilder(
      animation: controller,
      builder: (BuildContext context, Widget? _) {
        return MaterialApp(
          title: 'llm_kb',
          debugShowCheckedModeBanner: false,
          theme: buildDswTheme(Brightness.light),
          darkTheme: buildDswTheme(Brightness.dark),
          themeMode: controller.themeMode,
          // 没有 AppBar：DSH 也没有全局顶栏，每一栏自己负责自己的头部。
          home: Scaffold(
            body: _StartupGate(
              connection: connection,
              child: AppShell(controller: controller, connection: connection),
            ),
          ),
        );
      },
    );
  }
}

/// 启动闸门：连接成功（或用户选择「离线进入」）之前先显示全屏启动页。
///
/// 它只听 [ConnectionController]：`startupFinished` 一旦为真就把主界面换上来，
/// 之后在主界面里断开 / 重连失败都不会退回启动页。
class _StartupGate extends StatelessWidget {
  const _StartupGate({required this.child, this.connection});

  final Widget child;
  final ConnectionController? connection;

  @override
  Widget build(BuildContext context) {
    final ConnectionController? controller = connection;
    if (controller == null) {
      // widget 测试可以不接连接，直接看主界面。
      return child;
    }
    return AnimatedBuilder(
      animation: controller,
      builder: (BuildContext context, Widget? _) {
        if (!controller.startupFinished) {
          return StartupScreen(connection: controller);
        }
        return child;
      },
    );
  }
}
