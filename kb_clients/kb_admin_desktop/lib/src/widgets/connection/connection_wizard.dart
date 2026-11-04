// 首次连接 / 空配置时的配置向导。
//
// 三步：
//   1. 选连接方式，并解释 kb-core / 运行时目录 / 存储目录分别是什么；
//   2. 填参数（复用 [ConnectionForm]，带文件 / 目录选择与必填校验）；
//   3. 确认、可选测试连接，然后保存并连接。
//
// 它是“配置文件没有连接项”时的首选入口；管理已有连接仍用连接对话框。

import 'dart:async';

import 'package:flutter/material.dart';

import '../../services/kb_client_api.dart';
import '../../state/connection_controller.dart';
import '../../theme/app_theme.dart';
import '../../theme/dsw_tokens.dart';
import '../common/dsw_controls.dart';
import 'connection_form.dart';

/// 打开配置向导。
///
/// 返回是否成功连上（用户取消或失败都是 `false`/`null`）。
Future<bool?> showConnectionWizard(
  BuildContext context,
  ConnectionController connection, {
  ConnectionView? initial,
}) {
  return showDialog<bool>(
    context: context,
    barrierDismissible: true,
    builder: (BuildContext context) =>
        _ConnectionWizard(connection: connection, initial: initial),
  );
}

/// 向导本体。
class _ConnectionWizard extends StatefulWidget {
  const _ConnectionWizard({required this.connection, this.initial});

  final ConnectionController connection;
  final ConnectionView? initial;

  @override
  State<_ConnectionWizard> createState() => _ConnectionWizardState();
}

class _ConnectionWizardState extends State<_ConnectionWizard> {
  static const int _steps = 3;

  final GlobalKey<ConnectionFormState> _formKey =
      GlobalKey<ConnectionFormState>();

  int _step = 0;
  late String _kind;
  bool _busy = false;
  String _message = '';

  @override
  void initState() {
    super.initState();
    _kind = widget.initial?.kind ?? 'local-launch';
  }

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    return Dialog(
      backgroundColor: c.bgLayer2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 680),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              _header_(context),
              const SizedBox(height: 18),
              ..._body_(context),
              if (_message.isNotEmpty) ...<Widget>[
                const SizedBox(height: 14),
                Text(
                  _message,
                  style: DswTypography.caption.copyWith(color: c.stateError),
                ),
              ],
              const SizedBox(height: 20),
              _actions_(context),
            ],
          ),
        ),
      ),
    );
  }

  /// 标题 + 步骤指示。
  Widget _header_(BuildContext context) {
    final DswColors c = context.dsw;
    final List<String> titles = <String>['选择连接方式', '填写参数', '确认并连接'];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            Container(
              width: 36,
              height: 36,
              decoration: BoxDecoration(
                color: c.bgLayer3,
                borderRadius: BorderRadius.circular(10),
              ),
              alignment: Alignment.center,
              child: Icon(
                Icons.explore_outlined,
                size: 18,
                color: c.labelSecondary,
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    '配置向导 · ${titles[_step]}',
                    style: DswTypography.body.copyWith(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                      color: c.labelPrimary,
                    ),
                  ),
                  Text(
                    '第 ${_step + 1} 步，共 $_steps 步',
                    style: DswTypography.caption.copyWith(
                      fontSize: 11,
                      color: c.labelCaption,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Row(
          children: <Widget>[
            for (int index = 0; index < _steps; index++)
              Expanded(
                child: Container(
                  height: 3,
                  margin: EdgeInsets.only(right: index == _steps - 1 ? 0 : 6),
                  decoration: BoxDecoration(
                    color: index <= _step ? c.brandPrimary : c.bgLayer3,
                    borderRadius: BorderRadius.circular(999),
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }

  /// 每一步的主体。
  ///
  /// 表单从第 1 步开始就一直留在树里（非当前步用 `Offstage` 收起）：第 3 步的
  /// 摘要 / 测试 / 保存都要通过 `_formKey` 拿它的 `draft()` 与 `validate()`，
  /// 不能因为切步而把 state 丢掉。
  List<Widget> _body_(BuildContext context) {
    final Widget form = ConnectionForm(
      key: _formKey,
      connection: widget.connection,
      initial: widget.initial,
      initialKind: _kind,
      initialName: '本机',
    );

    return <Widget>[
      Offstage(offstage: _step != 1, child: form),
      if (_step == 0) ..._chooseKind_(context),
      if (_step == 2) ..._confirm_(context),
    ];
  }

  /// 第 1 步：选连接方式 + 路径解释。
  List<Widget> _chooseKind_(BuildContext context) {
    final DswColors c = context.dsw;
    return <Widget>[
      Text(
        '先决定客户端怎么找到 kb_core；下一步再填具体路径。',
        style: DswTypography.caption.copyWith(color: c.labelTertiary),
      ),
      const SizedBox(height: 12),
      _KindCard(
        kind: 'local-launch',
        selected: _kind == 'local-launch',
        icon: Icons.terminal_outlined,
        title: '本机 · 启动 kb_core',
        description:
            '由客户端通过 kb_core_starter 启动一个本机 kb_core 子进程。'
            '适合本机还没有常驻实例的时候；关掉客户端时子进程也会结束。',
        onTap: () => setState(() => _kind = 'local-launch'),
      ),
      _KindCard(
        kind: 'local-attach',
        selected: _kind == 'local-attach',
        icon: Icons.link,
        title: '本机 · 连接已在运行的 kb_core',
        description:
            '只连接已经在跑的本机实例，不启动新进程。'
            '适合你自己用命令行 / 服务管理器拉起 kb_core 的情况。',
        onTap: () => setState(() => _kind = 'local-attach'),
      ),
      _KindCard(
        kind: 'tcp',
        selected: _kind == 'tcp',
        icon: Icons.public,
        title: '远程 · 经 kb_core_rproxy 的 TCP',
        description: '连接另一台机器上的 kb_core。网关没有鉴权与 TLS，仅适用于受信网络。',
        onTap: () => setState(() => _kind = 'tcp'),
      ),
      const SizedBox(height: 14),
      Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: c.bgLayer3,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              '这几个路径分别是什么？',
              style: DswTypography.body.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w500,
                color: c.labelPrimary,
              ),
            ),
            const SizedBox(height: 6),
            _bullet_(
              context,
              'kb-core',
              '真正跑起来的服务进程。Cargo 构建产物通常是 target/debug/kb-core（连字符）。',
            ),
            _bullet_(
              context,
              '运行时目录',
              '只放 UDS socket / 端点名字文件，用来让客户端找到 kb_core；它不是知识库数据。',
            ),
            _bullet_(
              context,
              '存储目录',
              '知识库数据真正落盘的地方。必须选一个持久目录，不能放 /tmp 或 XDG_RUNTIME_DIR。',
            ),
          ],
        ),
      ),
    ];
  }

  /// 第 3 步：确认摘要 + 测试连接。
  List<Widget> _confirm_(BuildContext context) {
    final DswColors c = context.dsw;
    final ConnectionView? draft = _formKey.currentState?.draft();
    if (draft == null) {
      return <Widget>[
        Text(
          '表单还没有准备好，请返回上一步。',
          style: DswTypography.caption.copyWith(color: c.stateError),
        ),
      ];
    }

    return <Widget>[
      Text(
        '确认无误后保存并连接；下次启动会默认选中这一条。',
        style: DswTypography.caption.copyWith(color: c.labelTertiary),
      ),
      const SizedBox(height: 12),
      _summaryRow_(context, '主机名', draft.name),
      _summaryRow_(context, '方式', widget.connection.describeKind(draft.kind)),
      if (draft.kind == 'local-launch') ...<Widget>[
        _summaryRow_(context, 'kb-core', draft.kbCore),
        _summaryRow_(context, '运行时目录', draft.runtimeDir),
        _summaryRow_(context, '存储目录', draft.storageDir),
      ] else if (draft.kind == 'local-attach') ...<Widget>[
        _summaryRow_(context, '运行时目录', draft.runtimeDir),
      ] else ...<Widget>[_summaryRow_(context, '地址', draft.address)],
    ];
  }

  /// 底部的导航 / 提交按钮。
  Widget _actions_(BuildContext context) {
    if (_step == 0) {
      return Row(
        children: <Widget>[
          DswGhostButton(
            label: '取消',
            onPressed: _busy ? null : () => Navigator.of(context).pop(),
          ),
          const Spacer(),
          DswPrimaryButton(label: '下一步', onPressed: _busy ? null : _next_),
        ],
      );
    }
    if (_step == 1) {
      return Row(
        children: <Widget>[
          DswGhostButton(label: '上一步', onPressed: _busy ? null : _back_),
          const Spacer(),
          DswPrimaryButton(label: '下一步', onPressed: _busy ? null : _next_),
        ],
      );
    }
    return Row(
      children: <Widget>[
        DswGhostButton(label: '上一步', onPressed: _busy ? null : _back_),
        const SizedBox(width: 8),
        DswGhostButton(
          label: _busy ? '测试中…' : '测试连接',
          onPressed: _busy ? null : () => unawaited(_test_()),
        ),
        const Spacer(),
        DswPrimaryButton(
          label: _busy ? '连接中…' : '保存并连接',
          onPressed: _busy ? null : () => unawaited(_saveAndConnect_()),
        ),
      ],
    );
  }

  /// 下一步：第 1→2 步把选中的 kind 同步给表单；第 2→3 步先校验。
  void _next_() {
    if (_step == 0) {
      setState(() {
        _step = 1;
        _message = '';
      });
      // 表单可能已经存在（用户来回走过），把 step 0 的选择同步过去。
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _formKey.currentState?.setKind(_kind);
      });
      return;
    }
    if (_step == 1) {
      final String? invalid = _formKey.currentState?.validate();
      if (invalid != null) {
        setState(() => _message = invalid);
        return;
      }
      setState(() {
        _step = 2;
        _message = '';
      });
    }
  }

  /// 上一步：从第 3 步回退时，把表单里可能改过的 kind 同步回 step 0 的选择。
  void _back_() {
    if (_step == 2) {
      final String? kind = _formKey.currentState?.draft().kind;
      if (kind != null) {
        _kind = kind;
      }
    }
    if (_step > 0) {
      setState(() {
        _step -= 1;
        _message = '';
      });
    }
  }

  /// 测试当前草稿（不保存、不改 last_used）。
  Future<void> _test_() async {
    final ConnectionFormState? form = _formKey.currentState;
    if (form == null) {
      return;
    }
    final String? invalid = form.validate();
    if (invalid != null) {
      setState(() => _message = invalid);
      return;
    }
    final ConnectionView draft = form.draft();
    setState(() {
      _busy = true;
      _message = '正在测试「${draft.name}」…';
    });
    final ConnectReport report = await widget.connection.testConnection(draft);
    if (!mounted) {
      return;
    }
    setState(() {
      _busy = false;
      _message = report.ok
          ? '测试成功：服务端 ${report.serverVersion}（协议 v${report.protocolVersion}）'
          : '测试失败：${report.error}';
    });
  }

  /// 保存配置并连接；成功时关闭向导。
  Future<void> _saveAndConnect_() async {
    final ConnectionFormState? form = _formKey.currentState;
    if (form == null) {
      return;
    }
    final String? invalid = form.validate();
    if (invalid != null) {
      setState(() => _message = invalid);
      return;
    }
    final ConnectionView draft = form.draft();
    final List<ConnectionView> merged = <ConnectionView>[
      for (final ConnectionView profile in widget.connection.profiles)
        if (profile.name != draft.name) profile,
      draft,
    ];

    setState(() {
      _busy = true;
      _message = '';
    });

    final String saveError = await widget.connection.saveConfig(
      defaultName: draft.name,
      connections: merged,
    );
    if (saveError.isNotEmpty) {
      if (mounted) {
        setState(() {
          _busy = false;
          _message = saveError;
        });
      }
      return;
    }

    final bool ok = await widget.connection.connect(draft);
    if (!mounted) {
      return;
    }
    if (ok) {
      Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _busy = false;
      _message = widget.connection.error.isEmpty
          ? '连接失败'
          : widget.connection.error;
    });
  }

  /// 摘要里的一行。
  Widget _summaryRow_(BuildContext context, String label, String value) {
    final DswColors c = context.dsw;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          SizedBox(
            width: 84,
            child: Text(
              label,
              style: DswTypography.caption.copyWith(color: c.labelTertiary),
            ),
          ),
          Expanded(
            child: SelectableText(
              value,
              style: DswTypography.caption.copyWith(color: c.labelPrimary),
            ),
          ),
        ],
      ),
    );
  }

  /// 解释框里的一条。
  Widget _bullet_(BuildContext context, String term, String text) {
    final DswColors c = context.dsw;
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Text.rich(
        TextSpan(
          children: <InlineSpan>[
            TextSpan(
              text: '$term：',
              style: DswTypography.caption.copyWith(
                color: c.labelSecondary,
                fontWeight: FontWeight.w500,
              ),
            ),
            TextSpan(
              text: text,
              style: DswTypography.caption.copyWith(color: c.labelTertiary),
            ),
          ],
        ),
      ),
    );
  }
}

/// 第 1 步的一张连接方式卡片。
class _KindCard extends StatelessWidget {
  const _KindCard({
    required this.kind,
    required this.selected,
    required this.icon,
    required this.title,
    required this.description,
    required this.onTap,
  });

  final String kind;
  final bool selected;
  final IconData icon;
  final String title;
  final String description;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    return HoverBuilder(
      builder: (BuildContext context, bool hovered) {
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: onTap,
          child: AnimatedContainer(
            duration: DswMotion.fast,
            curve: DswMotion.easeInOut,
            margin: const EdgeInsets.only(bottom: 8),
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: selected
                  ? c.interactiveActive
                  : (hovered ? c.interactiveHover : c.bgLayer3),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: selected ? c.brandPrimary : Colors.transparent,
              ),
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  selected
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                  size: 16,
                  color: selected ? c.brandPrimary : c.labelCaption,
                ),
                const SizedBox(width: 8),
                Icon(icon, size: 18, color: c.labelSecondary),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        title,
                        style: DswTypography.body.copyWith(
                          fontSize: 13,
                          fontWeight: FontWeight.w500,
                          color: c.labelPrimary,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        description,
                        style: DswTypography.caption.copyWith(
                          color: c.labelTertiary,
                          height: 1.5,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
