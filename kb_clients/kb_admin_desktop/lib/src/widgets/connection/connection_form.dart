// 一条连接方式的编辑表单：连接对话框与配置向导共用。
//
// 它只负责“字段 + kind 选择 + 预填 + 必填校验”，不负责保存 / 连接；
// 外面的容器拿 `GlobalKey<ConnectionFormState>` 调 `draft()` / `validate()`。
// 这样“管理已有连接”和“首次配置向导”不会各写一套字段。

import 'dart:async';

import 'package:flutter/material.dart';

import '../../services/kb_client_api.dart';
import '../../state/connection_controller.dart';
import '../../theme/app_theme.dart';
import '../../theme/dsw_tokens.dart';
import '../common/dsw_controls.dart';
import '../common/dsw_path_field.dart';

/// 通用连接表单。
class ConnectionForm extends StatefulWidget {
  /// 构造表单。
  const ConnectionForm({
    super.key,
    required this.connection,
    this.initial,
    this.initialKind,
    this.initialName = '本机',
    this.pathChooser,
  });

  /// 连接控制器（取 kind 说明、预填建议）。
  final ConnectionController connection;

  /// 要编辑的既有连接方式；为 `null` 时是“新建”。
  final ConnectionView? initial;

  /// 新建时预选的 kind；为 `null` 时用第一个可用的 kind。
  final String? initialKind;

  /// 新建时的默认主机名。
  final String initialName;

  /// 覆盖系统文件选择器（测试用）。
  final PathChooser? pathChooser;

  @override
  ConnectionFormState createState() => ConnectionFormState();
}

/// 表单状态；由外层通过 `GlobalKey` 调用 [draft] / [validate]。
class ConnectionFormState extends State<ConnectionForm> {
  late final TextEditingController _name;
  late final TextEditingController _kbCore;
  late final TextEditingController _runtimeDir;
  late final TextEditingController _storageDir;
  late final TextEditingController _address;

  late String _kind;
  bool _loadingSuggestion = true;

  @override
  void initState() {
    super.initState();
    _name = TextEditingController(
      text: widget.initial?.name ?? widget.initialName,
    );
    _kbCore = TextEditingController();
    _runtimeDir = TextEditingController();
    _storageDir = TextEditingController();
    _address = TextEditingController(text: '127.0.0.1:8788');
    _kind =
        widget.initial?.kind ??
        widget.initialKind ??
        widget.connection.kindDescriptions.keys.firstOrNull ??
        'local-launch';

    final ConnectionView? initial = widget.initial;
    if (initial != null) {
      _apply_(initial);
      _loadingSuggestion = false;
    } else {
      unawaited(_prefillSuggestion_());
    }
  }

  @override
  void dispose() {
    _name.dispose();
    _kbCore.dispose();
    _runtimeDir.dispose();
    _storageDir.dispose();
    _address.dispose();
    super.dispose();
  }

  /// 把表单内容填成一条既有连接方式。
  void fillFrom(ConnectionView profile) {
    setState(() {
      _apply_(profile);
      _loadingSuggestion = false;
    });
  }

  /// 清空成“新建”状态，并重新请求预填建议。
  void resetToNew([String? name]) {
    setState(() {
      _kind =
          widget.initialKind ??
          widget.connection.kindDescriptions.keys.firstOrNull ??
          'local-launch';
      _name.text = name ?? widget.initialName;
      _kbCore.clear();
      _runtimeDir.clear();
      _storageDir.clear();
      _address.text = '127.0.0.1:8788';
      _loadingSuggestion = true;
    });
    unawaited(_prefillSuggestion_());
  }

  /// 只改 kind（向导第一步选完之后调用）。
  void setKind(String kind) {
    if (_kind == kind) {
      return;
    }
    setState(() => _kind = kind);
  }

  /// 当前表单组装出来的连接方式。
  ConnectionView draft() {
    final String name = _name.text.trim().isEmpty ? '未命名连接' : _name.text.trim();
    switch (_kind) {
      case 'local-attach':
        return ConnectionView(
          name: name,
          kind: _kind,
          summary: widget.connection.describeKind(_kind),
          kbCore: '',
          runtimeDir: _runtimeDir.text.trim(),
          storageDir: '',
          address: '',
        );
      case 'tcp':
        return ConnectionView(
          name: name,
          kind: _kind,
          summary: widget.connection.describeKind(_kind),
          kbCore: '',
          runtimeDir: '',
          storageDir: '',
          address: _address.text.trim(),
        );
      default:
        return ConnectionView(
          name: name,
          kind: 'local-launch',
          summary: widget.connection.describeKind('local-launch'),
          kbCore: _kbCore.text.trim(),
          runtimeDir: _runtimeDir.text.trim(),
          storageDir: _storageDir.text.trim(),
          address: '',
        );
    }
  }

  /// 必填检查；`null` 表示通过。
  String? validate() {
    switch (_kind) {
      case 'local-launch':
        if (_kbCore.text.trim().isEmpty) {
          return '请填写 kb-core 的完整路径';
        }
        if (_runtimeDir.text.trim().isEmpty) {
          return '请填写运行时目录';
        }
        if (_storageDir.text.trim().isEmpty) {
          return '请填写存储目录（不会自动使用临时目录）';
        }
      case 'local-attach':
        if (_runtimeDir.text.trim().isEmpty) {
          return '请填写运行时目录';
        }
      case 'tcp':
        if (_address.text.trim().isEmpty) {
          return '请填写网关地址';
        }
    }
    return null;
  }

  /// 渲染表单。
  @override
  Widget build(BuildContext context) {
    final List<String> kinds = widget.connection.kindDescriptions.keys.toList();
    if (kinds.isNotEmpty && !kinds.contains(_kind)) {
      _kind = kinds.first;
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        DswLabeledField(
          label: '主机名',
          controller: _name,
          hint: '例如：本机 / 实验室（自己起的花名）',
        ),
        const SizedBox(height: 10),
        _KindPicker(
          kinds: kinds,
          value: _kind,
          describe: widget.connection.describeKind,
          onChanged: (String value) => setState(() => _kind = value),
        ),
        const SizedBox(height: 10),
        ..._fieldsFor_(context),
      ],
    );
  }

  /// 当前 `kind` 需要的输入框（不适用的不显示）。
  List<Widget> _fieldsFor_(BuildContext context) {
    switch (_kind) {
      case 'local-launch':
        return <Widget>[
          DswPathField(
            label: 'kb-core 路径',
            controller: _kbCore,
            hint: '/usr/local/bin/kb-core',
            chooser: widget.pathChooser,
          ),
          _help_('可执行文件。Cargo 构建产物通常叫 kb-core（连字符）；选中的必须是可执行文件。'),
          const SizedBox(height: 10),
          DswPathField(
            label: '运行时目录',
            controller: _runtimeDir,
            pickDirectory: true,
            chooser: widget.pathChooser,
          ),
          _help_(
            '只放 UDS socket / 端点名字文件，不是知识库数据。'
            r'普通模式用 $XDG_RUNTIME_DIR/llm_kb；便携模式是 <base>/run。',
          ),
          const SizedBox(height: 10),
          DswPathField(
            label: '存储目录（必填）',
            controller: _storageDir,
            hint: '自己选一个持久目录，例如 ~/.local/share/kb_admin_desktop/storage',
            pickDirectory: true,
            chooser: widget.pathChooser,
          ),
          _help_('知识库数据真正落盘的地方；必须持久，不要用 /tmp 或 XDG_RUNTIME_DIR。'),
          if (_loadingSuggestion) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              '正在读取缺省值…',
              style: DswTypography.caption.copyWith(
                color: context.dsw.labelCaption,
              ),
            ),
          ] else ...<Widget>[
            if (_kbCore.text.trim().isEmpty) ...<Widget>[
              const SizedBox(height: 6),
              Text(
                '没猜到 kb-core 在哪，请手动填它的完整路径。',
                style: DswTypography.caption.copyWith(
                  color: context.dsw.stateWarn,
                ),
              ),
            ],
            if (_storageDir.text.trim().isEmpty) ...<Widget>[
              const SizedBox(height: 6),
              Text(
                '存储目录没有默认值：不会放进运行时 / 临时目录，请自己选一个持久位置。',
                style: DswTypography.caption.copyWith(
                  color: context.dsw.stateWarn,
                ),
              ),
            ],
          ],
        ];
      case 'local-attach':
        return <Widget>[
          DswPathField(
            label: '运行时目录',
            controller: _runtimeDir,
            pickDirectory: true,
            chooser: widget.pathChooser,
          ),
          _help_('客户端在这里找已经在跑的 kb_core 的 UDS socket；不会启动新进程。'),
        ];
      case 'tcp':
        return <Widget>[
          DswLabeledField(
            label: '地址',
            controller: _address,
            hint: '192.168.1.5:8788',
          ),
          const SizedBox(height: 6),
          Text(
            '⚠️ 网关没有鉴权与 TLS，仅用于受信网络。',
            style: DswTypography.caption.copyWith(color: context.dsw.stateWarn),
          ),
        ];
      default:
        return const <Widget>[];
    }
  }

  /// 把一条连接方式写进控制器（不触发 setState）。
  void _apply_(ConnectionView profile) {
    _kind = profile.kind;
    _name.text = profile.name;
    _kbCore.text = profile.kbCore;
    _runtimeDir.text = profile.runtimeDir;
    _storageDir.text = profile.storageDir;
    _address.text = profile.address.isEmpty
        ? '127.0.0.1:8788'
        : profile.address;
  }

  /// 首次运行时向原生侧要一条预填的本机连接方式；只填空字段，不覆盖用户输入。
  Future<void> _prefillSuggestion_() async {
    try {
      final String name = _name.text.trim().isEmpty ? '本机' : _name.text.trim();
      final ConnectionView suggested = await widget.connection
          .suggestedLocalConnection(name);
      if (!mounted) {
        return;
      }
      setState(() {
        if (_kbCore.text.trim().isEmpty) {
          _kbCore.text = suggested.kbCore;
        }
        if (_runtimeDir.text.trim().isEmpty) {
          _runtimeDir.text = suggested.runtimeDir;
        }
        if (_storageDir.text.trim().isEmpty &&
            suggested.storageDir.isNotEmpty) {
          _storageDir.text = suggested.storageDir;
        }
        _loadingSuggestion = false;
      });
    } catch (_) {
      if (mounted) {
        setState(() => _loadingSuggestion = false);
      }
    }
  }

  /// 字段下方的一行说明。
  Widget _help_(String text) {
    return Padding(
      padding: const EdgeInsets.only(top: 4, left: 2),
      child: Text(
        text,
        style: DswTypography.caption.copyWith(
          fontSize: 11,
          color: context.dsw.labelCaption,
        ),
      ),
    );
  }
}

/// 连接方式下拉框 + 说明。
class _KindPicker extends StatelessWidget {
  const _KindPicker({
    required this.kinds,
    required this.value,
    required this.describe,
    required this.onChanged,
  });

  final List<String> kinds;
  final String value;
  final String Function(String kind) describe;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            SizedBox(
              width: 74,
              child: Text(
                '方式',
                style: DswTypography.caption.copyWith(color: c.labelTertiary),
              ),
            ),
            Expanded(
              child: DropdownButton<String>(
                value: kinds.contains(value) ? value : null,
                isExpanded: true,
                underline: const SizedBox.shrink(),
                dropdownColor: c.bgLayer2,
                items: <DropdownMenuItem<String>>[
                  for (final String kind in kinds)
                    DropdownMenuItem<String>(value: kind, child: Text(kind)),
                ],
                onChanged: (String? next) {
                  if (next != null) {
                    onChanged(next);
                  }
                },
              ),
            ),
          ],
        ),
        if (describe(value).isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(left: 74, top: 2),
            child: Text(
              describe(value),
              style: DswTypography.caption.copyWith(
                fontSize: 11,
                color: c.labelCaption,
              ),
            ),
          ),
      ],
    );
  }
}
