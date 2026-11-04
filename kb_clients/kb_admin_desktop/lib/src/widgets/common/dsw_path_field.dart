// 文件 / 目录路径输入框：文本框 + 右侧「浏览…」按钮。
//
// 底层用官方 `file_selector`：`openFile()` 选文件、`getDirectoryPath()` 选目录。
// 它是 Flutter 团队维护的插件，Linux / macOS / Windows 都有实现；用户取消时
// 返回 `null`，输入框保持原值。
//
// 测试可以传 `chooser` 覆盖系统对话框，避免 widget 测试真的去调平台插件。

import 'dart:async';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';

import 'dsw_controls.dart';

/// 一次路径选择；用户取消时返回 `null`。
typedef PathChooser = Future<String?> Function();

/// 文件 / 目录路径输入框。
class DswPathField extends StatefulWidget {
  /// 构造一个路径输入框。
  const DswPathField({
    super.key,
    required this.label,
    required this.controller,
    this.hint,
    this.pickDirectory = false,
    this.chooser,
    this.enabled = true,
  });

  /// 字段标题。
  final String label;

  /// 文本控制器（由调用方持有）。
  final TextEditingController controller;

  /// 占位文字。
  final String? hint;

  /// `true` 选目录，`false` 选文件。
  final bool pickDirectory;

  /// 覆盖系统选择器（测试用）；为 `null` 时调 `file_selector`。
  final PathChooser? chooser;

  /// 是否允许操作。
  final bool enabled;

  @override
  State<DswPathField> createState() => _DswPathFieldState();
}

class _DswPathFieldState extends State<DswPathField> {
  bool _picking = false;

  /// 打开系统选择器并把结果写回输入框。
  Future<void> _pick_() async {
    if (_picking || !widget.enabled) {
      return;
    }
    setState(() => _picking = true);
    try {
      final String? path = await (widget.chooser ?? _systemChooser_)();
      if (!mounted || path == null || path.isEmpty) {
        return;
      }
      widget.controller.text = path;
    } catch (error) {
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.maybeOf(context)
          ?.showSnackBar(SnackBar(content: Text('打开文件浏览器失败: $error')));
    } finally {
      if (mounted) {
        setState(() => _picking = false);
      }
    }
  }

  /// 调平台插件：文件用 `openFile`，目录用 `getDirectoryPath`。
  Future<String?> _systemChooser_() async {
    if (widget.pickDirectory) {
      return getDirectoryPath();
    }
    final XFile? file = await openFile();
    return file?.path;
  }

  @override
  Widget build(BuildContext context) {
    return DswLabeledField(
      label: widget.label,
      controller: widget.controller,
      hint: widget.hint,
      suffix: DswIconButton(
        icon: widget.pickDirectory
            ? Icons.folder_open_outlined
            : Icons.insert_drive_file_outlined,
        tooltip: widget.pickDirectory ? '选择目录…' : '选择文件…',
        iconSize: 16,
        onPressed: widget.enabled && !_picking
            ? () => unawaited(_pick_())
            : null,
      ),
    );
  }
}
