// 「连接方式」管理对话框。
//
// 三种进入方式：
//
// - **首次运行**（配置文件没有连接项）：启动页会自动打开**配置向导**；
//   这个对话框里的空状态也提供「打开配置向导」入口；
// - **手动打开**（侧边栏的连接条 / 折叠轨道上的插件图标）：列出已有连接方式，
//   选一条编辑、测试或删除，也可以新建；
// - 表单本身由 [ConnectionForm] 提供，字段说明 / 路径选择 / 必填校验都在那里。
//
// 已有连接方式悬停时右侧出现齿轮，菜单里是「测试连接 / 删除配置」。

import 'dart:async';

import 'package:flutter/material.dart';

import '../../services/kb_client_api.dart';
import '../../state/connection_controller.dart';
import '../../theme/app_theme.dart';
import '../../theme/dsw_tokens.dart';
import '../common/dsw_controls.dart';
import 'connection_form.dart';
import 'connection_wizard.dart';

/// 打开「连接方式」对话框。
///
/// 返回是否成功连上（用户取消或失败都是 `false`/`null`）。
Future<bool?> showConnectionDialog(
  BuildContext context,
  ConnectionController connection, {
  bool firstRun = false,
}) {
  return showDialog<bool>(
    context: context,
    barrierDismissible: !firstRun,
    builder: (BuildContext context) =>
        _ConnectionDialog(connection: connection, firstRun: firstRun),
  );
}

/// 对话框本体。
class _ConnectionDialog extends StatefulWidget {
  const _ConnectionDialog({required this.connection, required this.firstRun});

  final ConnectionController connection;
  final bool firstRun;

  @override
  State<_ConnectionDialog> createState() => _ConnectionDialogState();
}

class _ConnectionDialogState extends State<_ConnectionDialog> {
  final GlobalKey<ConnectionFormState> _formKey =
      GlobalKey<ConnectionFormState>();

  /// 当前填在表单里的既有连接方式；`null` 表示“新建”。
  ConnectionView? _editing;
  bool _busy = false;
  String _message = '';

  @override
  void initState() {
    super.initState();
    _editing = widget.connection.profiles.firstOrNull;
  }

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    final List<ConnectionView> profiles = widget.connection.profiles;

    return Dialog(
      backgroundColor: c.bgLayer2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                widget.firstRun ? '连接 kb_core' : '连接方式',
                style: DswTypography.body.copyWith(
                  fontSize: 16,
                  fontWeight: FontWeight.w600,
                  color: c.labelPrimary,
                ),
              ),
              const SizedBox(height: 6),
              Text(
                '工作区与会话由 kb_core 保管；先告诉客户端怎么找到它。\n'
                '「主机名」只是客户端这边的花名，想怎么叫就怎么叫。',
                style: DswTypography.caption.copyWith(color: c.labelTertiary),
              ),
              if (widget.connection.configPath.isNotEmpty) ...<Widget>[
                const SizedBox(height: 6),
                Text(
                  '配置文件：${widget.connection.configPath}',
                  style: DswTypography.caption.copyWith(
                    fontSize: 11,
                    color: c.labelCaption,
                  ),
                ),
              ],
              if (profiles.isEmpty)
                _emptyBanner_(context)
              else ...<Widget>[
                const SizedBox(height: 16),
                Row(
                  children: <Widget>[
                    Expanded(child: _SectionLabel('已配置的连接方式')),
                    DswGhostButton(
                      label: '新建',
                      onPressed: _busy ? null : _startNew_,
                    ),
                  ],
                ),
                for (final ConnectionView profile in profiles)
                  _ProfileRow(
                    profile: profile,
                    selected: profile.name == _editing?.name,
                    busy: _busy,
                    onTap: () => _selectProfile_(profile),
                    onTest: () => unawaited(_testProfile_(profile)),
                    onDelete: () => unawaited(_deleteProfile_(profile)),
                  ),
                const SizedBox(height: 8),
              ],
              const SizedBox(height: 16),
              _SectionLabel(profiles.isEmpty ? '连接方式' : '新增 / 编辑'),
              const SizedBox(height: 8),
              ConnectionForm(
                key: _formKey,
                connection: widget.connection,
                initial: _editing,
                initialName: '本机',
              ),
              if (_message.isNotEmpty) ...<Widget>[
                const SizedBox(height: 12),
                Text(
                  _message,
                  style: DswTypography.caption.copyWith(color: c.stateError),
                ),
              ],
              const SizedBox(height: 20),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: <Widget>[
                  DswGhostButton(
                    label: '取消',
                    onPressed: _busy ? null : () => Navigator.of(context).pop(),
                  ),
                  const SizedBox(width: 8),
                  DswPrimaryButton(
                    label: _busy ? '连接中…' : '保存并连接',
                    onPressed: _busy ? null : _saveAndConnect_,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// 列表为空时的提示与向导入口。
  Widget _emptyBanner_(BuildContext context) {
    final DswColors c = context.dsw;
    return Container(
      margin: const EdgeInsets.only(top: 16),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: c.bgLayer3,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '还没有配置连接方式。',
            style: DswTypography.body.copyWith(
              fontSize: 13,
              color: c.labelPrimary,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            '建议先运行配置向导，它会解释 kb-core、运行时目录和存储目录分别是什么；'
            '也可以直接在下面手动填写。',
            style: DswTypography.caption.copyWith(color: c.labelTertiary),
          ),
          const SizedBox(height: 10),
          DswPrimaryButton(
            label: '打开配置向导',
            onPressed: _busy ? null : () => unawaited(_openWizard_()),
          ),
        ],
      ),
    );
  }

  /// 选中一条既有连接方式填进表单。
  void _selectProfile_(ConnectionView profile) {
    if (_busy) {
      return;
    }
    setState(() {
      _editing = profile;
      _message = '';
    });
    _formKey.currentState?.fillFrom(profile);
  }

  /// 切到“新建”。
  void _startNew_() {
    if (_busy) {
      return;
    }
    setState(() {
      _editing = null;
      _message = '';
    });
    _formKey.currentState?.resetToNew();
  }

  /// 保存配置（合并同名项）然后连。
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

  /// 测试一条连接方式：真实握手后立刻断开，不影响当前连接。
  Future<void> _testProfile_(ConnectionView profile) async {
    if (_busy) {
      return;
    }
    setState(() {
      _busy = true;
      _message = '正在测试「${profile.name}」…';
    });

    final ConnectReport report = await widget.connection.testConnection(
      profile,
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _busy = false;
      _message = report.ok
          ? '测试成功：「${profile.name}」服务端 ${report.serverVersion}'
                '（协议 v${report.protocolVersion}）'
          : '测试失败：${report.error}';
    });
  }

  /// 删除一条连接方式（先确认；如果正在使用会断开当前连接）。
  Future<void> _deleteProfile_(ConnectionView profile) async {
    if (_busy) {
      return;
    }
    final bool? confirmed = await _confirmDelete_(profile);
    if (confirmed != true || !mounted) {
      return;
    }

    setState(() {
      _busy = true;
      _message = '';
    });
    final String error = await widget.connection.removeProfile(profile.name);
    if (!mounted) {
      return;
    }

    final List<ConnectionView> remaining = widget.connection.profiles;
    setState(() {
      _busy = false;
      _message = error.isNotEmpty ? error : '已删除「${profile.name}」';
      _editing = remaining.firstOrNull;
    });
    final ConnectionView? editing = _editing;
    if (editing != null) {
      _formKey.currentState?.fillFrom(editing);
    } else {
      _formKey.currentState?.resetToNew();
    }
  }

  /// 打开配置向导；向导连上之后这个对话框也一起关掉。
  Future<void> _openWizard_() async {
    final bool? connected = await showConnectionWizard(
      context,
      widget.connection,
    );
    if (!mounted) {
      return;
    }
    if (connected == true) {
      Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _editing = widget.connection.profiles.firstOrNull;
      _message = '';
    });
    final ConnectionView? editing = _editing;
    if (editing != null) {
      _formKey.currentState?.fillFrom(editing);
    } else {
      _formKey.currentState?.resetToNew();
    }
  }

  Future<bool?> _confirmDelete_(ConnectionView profile) {
    final DswColors c = context.dsw;
    final bool active =
        widget.connection.connected &&
        widget.connection.profileName == profile.name;
    return showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        backgroundColor: c.bgLayer2,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
        title: Text(
          '删除连接方式',
          style: DswTypography.body.copyWith(
            fontSize: 16,
            fontWeight: FontWeight.w600,
            color: c.labelPrimary,
          ),
        ),
        content: Text(
          '要删除「${profile.name}」吗？\n'
          '${active ? '它正在使用，删除后会断开当前连接。' : '这个操作不可撤销。'}',
          style: DswTypography.caption.copyWith(
            fontSize: 13,
            color: c.labelSecondary,
            height: 1.6,
          ),
        ),
        actions: <Widget>[
          DswGhostButton(
            label: '取消',
            onPressed: () => Navigator.of(context).pop(false),
          ),
          DswPrimaryButton(
            label: '删除',
            onPressed: () => Navigator.of(context).pop(true),
          ),
        ],
      ),
    );
  }
}

/// 小标题。
class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: DswTypography.caption.copyWith(
        color: context.dsw.labelTertiary,
        fontWeight: FontWeight.w500,
      ),
    );
  }
}

/// 已配置的连接方式行：点一下填进表单；悬停出现齿轮菜单。
class _ProfileRow extends StatefulWidget {
  const _ProfileRow({
    required this.profile,
    required this.selected,
    required this.busy,
    required this.onTap,
    required this.onTest,
    required this.onDelete,
  });

  final ConnectionView profile;
  final bool selected;
  final bool busy;
  final VoidCallback onTap;
  final VoidCallback onTest;
  final VoidCallback onDelete;

  @override
  State<_ProfileRow> createState() => _ProfileRowState();
}

class _ProfileRowState extends State<_ProfileRow> {
  bool _hovered = false;

  /// 在齿轮图标下方弹菜单；`showMenu` 的 future 拿到选择后再回调。
  Future<void> _openMenu_(BuildContext buttonContext) async {
    final RenderBox button = buttonContext.findRenderObject()! as RenderBox;
    final RenderBox overlay =
        Overlay.of(buttonContext).context.findRenderObject()! as RenderBox;
    final RelativeRect position = RelativeRect.fromRect(
      Rect.fromPoints(
        button.localToGlobal(Offset.zero, ancestor: overlay),
        button.localToGlobal(
          button.size.bottomRight(Offset.zero),
          ancestor: overlay,
        ),
      ),
      Offset.zero & overlay.size,
    );
    final DswColors c = context.dsw;
    final String? selected = await showMenu<String>(
      context: context,
      position: position,
      color: c.bgLayer2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      items: const <PopupMenuEntry<String>>[
        PopupMenuItem<String>(value: 'test', child: Text('测试连接')),
        PopupMenuItem<String>(value: 'delete', child: Text('删除配置')),
      ],
    );
    if (!mounted) {
      return;
    }
    if (selected == 'test') {
      widget.onTest();
    } else if (selected == 'delete') {
      widget.onDelete();
    }
  }

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.busy ? null : widget.onTap,
        child: Container(
          margin: const EdgeInsets.only(top: 4),
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          decoration: BoxDecoration(
            color: widget.selected ? c.interactiveActive : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            children: <Widget>[
              Icon(
                widget.selected
                    ? Icons.radio_button_checked
                    : Icons.radio_button_unchecked,
                size: 15,
                color: widget.selected ? c.brandPrimary : c.labelCaption,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      '${widget.profile.name}  ·  ${widget.profile.kind}',
                      style: DswTypography.body.copyWith(
                        fontSize: 13,
                        color: c.labelPrimary,
                      ),
                    ),
                    if (widget.profile.summary.isNotEmpty)
                      Text(
                        widget.profile.summary,
                        style: DswTypography.caption.copyWith(
                          color: c.labelTertiary,
                        ),
                      ),
                  ],
                ),
              ),
              if (_hovered && !widget.busy)
                Builder(
                  builder: (BuildContext buttonContext) => DswIconButton(
                    icon: Icons.settings_outlined,
                    tooltip: '连接方式操作',
                    iconSize: 16,
                    onPressed: () => unawaited(_openMenu_(buttonContext)),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
