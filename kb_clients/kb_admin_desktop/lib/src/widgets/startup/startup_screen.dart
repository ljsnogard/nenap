// 启动页：在进主界面之前决定“连哪个 kb_core”。
//
// 它把连接方式按传输分成两组：
//
// - **本机（Unix domain socket）**：`local-launch`（经 `kb_core_starter` 起一个
//   本机 `kb_core`）与 `local-attach`（附着到已经在跑的本机实例）；
// - **远程（TCP）**：`tcp`，经 `kb_core_rproxy` 连另一台机器上的 `kb_core`。
//
// 默认选中「最近一次使用」的那条（`last_used`，缺省回退到 `default` / 第一条），
// 倒数 5 秒自动连接；用户一旦操作（点选、点按钮、按键触发输入）就取消倒计时。
// 连接失败停在启动页显示错误，**不自动重试**；也可以「离线进入」主界面。
//
// 首次运行（没有配置文件）时自动打开连接对话框让用户输入；对话框本身仍然由
// `connection_dialog.dart` 提供，和主界面里的「管理连接方式…」是同一份。

import 'dart:async';

import 'package:flutter/material.dart';

import '../../services/kb_client_api.dart';
import '../../state/connection_controller.dart';
import '../../theme/app_theme.dart';
import '../../theme/dsw_tokens.dart';
import '../common/dsw_controls.dart';
import '../connection/connection_dialog.dart';
import '../connection/connection_wizard.dart';

/// 进主界面前的全屏启动页。
class StartupScreen extends StatefulWidget {
  /// 构造启动页。
  const StartupScreen({super.key, required this.connection});

  /// 连接配置与状态。
  final ConnectionController connection;

  /// 倒计时时长（需求：5 秒）。
  static const Duration countdownDuration = Duration(seconds: 10);

  @override
  State<StartupScreen> createState() => _StartupScreenState();
}

class _StartupScreenState extends State<StartupScreen> {
  Timer? _countdownTimer;
  int _secondsLeft = 0;
  bool _countdownRan = false;
  bool _countdownCancelled = false;
  bool _synced = false;
  bool _wizardOpened = false;
  bool _connecting = false;
  String? _selectedName;
  String _message = '';

  @override
  void initState() {
    super.initState();
    widget.connection.addListener(_onConnectionChanged_);
    _selectedName = widget.connection.startupProfile?.name;
    if (widget.connection.initialized) {
      _beginStartup_();
    }
  }

  @override
  void didUpdateWidget(StartupScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.connection != widget.connection) {
      oldWidget.connection.removeListener(_onConnectionChanged_);
      widget.connection.addListener(_onConnectionChanged_);
      _synced = false;
      _beginStartup_();
    }
  }

  @override
  void dispose() {
    _countdownTimer?.cancel();
    widget.connection.removeListener(_onConnectionChanged_);
    super.dispose();
  }

  /// 配置读完 / 变化时同步选中项；只在第一次做“自动打开首次运行对话框”和启动倒计时。
  void _onConnectionChanged_() {
    if (!mounted) {
      return;
    }
    if (!_synced) {
      _beginStartup_();
      return;
    }
    // 配置被改过（例如保存了新连接、删了旧连接）时，别让选中项悬空。
    if (_selectedName == null ||
        !widget.connection.profiles.any(
          (ConnectionView profile) => profile.name == _selectedName,
        )) {
      _selectedName = widget.connection.startupProfile?.name;
    }
  }

  /// 第一次拿到已初始化的配置：同步选中项、按需打开配置向导、开始倒计时。
  ///
  /// “没有配置”与“配置里一条连接都没有”都视为**应该运行配置向导**的信号。
  void _beginStartup_() {
    if (_synced || !widget.connection.initialized) {
      return;
    }
    _synced = true;
    _selectedName = widget.connection.startupProfile?.name;

    final bool needsWizard =
        widget.connection.firstRun ||
        (widget.connection.profiles.isEmpty &&
            widget.connection.configError.isEmpty);
    if (needsWizard) {
      if (!_wizardOpened) {
        _wizardOpened = true;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            unawaited(_openWizard_());
          }
        });
      }
      return;
    }

    if (widget.connection.configError.isEmpty &&
        widget.connection.profiles.isNotEmpty) {
      // 在下一帧启动，避免在 `notifyListeners()` 的调用栈里 setState。
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          _startCountdown_();
        }
      });
    }
  }

  /// 开始 5 秒倒计时；只启动一次。
  void _startCountdown_() {
    final ConnectionController connection = widget.connection;
    if (_countdownRan ||
        _countdownTimer != null ||
        connection.firstRun ||
        connection.profiles.isEmpty ||
        connection.configError.isNotEmpty ||
        _selectedName == null) {
      return;
    }
    _countdownRan = true;

    setState(() {
      _secondsLeft = StartupScreen.countdownDuration.inSeconds;
      _countdownCancelled = false;
      _message = '';
    });

    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (Timer timer) {
      if (!mounted) {
        timer.cancel();
        return;
      }
      if (_secondsLeft <= 1) {
        timer.cancel();
        _countdownTimer = null;
        setState(() => _secondsLeft = 0);
        unawaited(_connectSelected_());
      } else {
        setState(() => _secondsLeft -= 1);
      }
    });
  }

  /// 用户一操作就取消倒计时（倒计时结束后不再显示“已取消”）。
  void _cancelCountdown_() {
    if (_countdownTimer == null) {
      return;
    }
    _countdownTimer!.cancel();
    _countdownTimer = null;
    if (!mounted) {
      return;
    }
    setState(() {
      _secondsLeft = 0;
      _countdownCancelled = true;
    });
  }

  /// 选中一条连接方式（并写回“最近一次”），但先不连接。
  void _selectProfile_(ConnectionView profile) {
    if (_connecting || widget.connection.phase == ConnectionPhase.connecting) {
      return;
    }
    _cancelCountdown_();
    widget.connection.clearError();
    setState(() {
      _selectedName = profile.name;
      _message = '';
    });
    unawaited(widget.connection.selectStartupProfile(profile));
  }

  /// 连接当前选中的那条；失败留在启动页显示错误。
  Future<void> _connectSelected_() async {
    final ConnectionView? profile = _profileByName_(_selectedName);
    if (profile == null || _connecting) {
      return;
    }
    _cancelCountdown_();
    setState(() {
      _connecting = true;
      _message = '';
    });

    final bool ok = await widget.connection.connect(profile);
    if (!mounted) {
      return;
    }
    setState(() {
      _connecting = false;
      _message = ok
          ? ''
          : (widget.connection.error.isEmpty
                ? '连接失败'
                : widget.connection.error);
    });
  }

  /// 打开配置向导（首次运行 / 空配置）。连接成功后启动页会被主界面替换。
  Future<void> _openWizard_() async {
    _cancelCountdown_();
    final bool? connected = await showConnectionWizard(
      context,
      widget.connection,
    );
    if (!mounted || connected == true) {
      return;
    }
    // 用户取消：保持当前选择（用户操作已经取消过倒计时，不重新开始）。
    _selectedName = widget.connection.startupProfile?.name ?? _selectedName;
  }

  /// 打开连接对话框（新建 / 编辑已有连接）。连接成功后启动页会被主界面替换。
  Future<void> _openConnectionDialog_() async {
    _cancelCountdown_();
    final bool? connected = await showConnectionDialog(
      context,
      widget.connection,
    );
    if (!mounted || connected == true) {
      return;
    }
    // 用户取消：保持当前选择（用户操作已经取消过倒计时，不重新开始）。
    _selectedName = widget.connection.startupProfile?.name ?? _selectedName;
  }

  ConnectionView? _profileByName_(String? name) {
    if (name == null) {
      return null;
    }
    for (final ConnectionView profile in widget.connection.profiles) {
      if (profile.name == name) {
        return profile;
      }
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    // 底部文案要能反映倒计时 / 连接中 / 失败，所以整页挂在控制器的通知上。
    return AnimatedBuilder(
      animation: widget.connection,
      builder: (BuildContext context, Widget? _) {
        return Listener(
          // 任何指针操作都算“用户介入了”，取消自动执行（点「连接」会自己再连）。
          onPointerDown: (_) => _cancelCountdown_(),
          onPointerSignal: (_) => _cancelCountdown_(),
          child: ColoredBox(
            color: c.bgBase,
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 640),
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: c.bgLayer2,
                      borderRadius: BorderRadius.circular(20),
                      border: Border.all(color: c.borderL2),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.all(24),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          _header_(context),
                          const SizedBox(height: 20),
                          _body_(context),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  /// 标题与一行说明。
  Widget _header_(BuildContext context) {
    final DswColors c = context.dsw;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Container(
          width: 40,
          height: 40,
          decoration: BoxDecoration(
            color: c.bgLayer3,
            borderRadius: BorderRadius.circular(12),
          ),
          alignment: Alignment.center,
          child: Icon(Icons.dns_outlined, size: 20, color: c.labelSecondary),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                '连接 kb_core',
                style: DswTypography.body.copyWith(
                  fontSize: 17,
                  fontWeight: FontWeight.w600,
                  color: c.labelPrimary,
                ),
              ),
              const SizedBox(height: 3),
              Text(
                '工作区与会话由 kb_core 保管。先选择本机（Unix domain socket）'
                '或远程（TCP）的连法。',
                style: DswTypography.caption.copyWith(color: c.labelTertiary),
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// 主体：加载态 / 配置错误 / 首次运行 / 连接方式列表。
  Widget _body_(BuildContext context) {
    final ConnectionController connection = widget.connection;
    final DswColors c = context.dsw;

    if (!connection.initialized) {
      return Row(
        children: <Widget>[
          const SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 10),
          Text(
            '正在读取连接配置…',
            style: DswTypography.caption.copyWith(color: c.labelTertiary),
          ),
        ],
      );
    }

    // 配置读不出来，或者原生侧 / 读配置本身失败（此时列表为空）。
    // 连接失败（列表非空）不走这里，而是在下面保留列表 + 错误条。
    final String startupError = connection.configError.isNotEmpty
        ? connection.configError
        : (connection.profiles.isEmpty ? connection.error : '');
    if (startupError.isNotEmpty) {
      return _configError_(context, startupError);
    }

    if (connection.firstRun || connection.profiles.isEmpty) {
      return _noConfig_(context);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ..._group_(context, '本机（Unix domain socket）', <String>[
          'local-launch',
          'local-attach',
        ]),
        ..._group_(context, '远程（TCP）', <String>['tcp']),
        ..._group_(context, '其它', <String>[]),
        const SizedBox(height: 16),
        _status_(context),
        const SizedBox(height: 14),
        _actions_(context),
        const SizedBox(height: 10),
        Text(
          '配置文件：${connection.configPath}',
          style: DswTypography.caption.copyWith(
            fontSize: 11,
            color: c.labelCaption,
          ),
        ),
      ],
    );
  }

  /// 配置坏了 / 读不出来：让用户修或重新加载，别静默覆盖。
  Widget _configError_(BuildContext context, String message) {
    final DswColors c = context.dsw;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _errorBox_(context, message),
        const SizedBox(height: 14),
        Row(
          children: <Widget>[
            DswPrimaryButton(
              label: '重新加载',
              onPressed: () => unawaited(widget.connection.reloadConfig()),
            ),
            const SizedBox(width: 8),
            DswGhostButton(
              label: '编辑连接方式…',
              onPressed: () => unawaited(_openConnectionDialog_()),
            ),
            const Spacer(),
            DswGhostButton(
              label: '离线进入',
              onPressed: () => widget.connection.enterOffline(),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Text(
          '配置文件：${widget.connection.configPath}',
          style: DswTypography.caption.copyWith(
            fontSize: 11,
            color: c.labelCaption,
          ),
        ),
      ],
    );
  }

  /// 没有配置：让用户输入。
  Widget _noConfig_(BuildContext context) {
    final DswColors c = context.dsw;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          '还没有配置过连接方式。',
          style: DswTypography.body.copyWith(
            fontSize: 14,
            color: c.labelPrimary,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          '选一种本机或远程的连法，填好地址 / 目录保存后即可连接；'
          '下次启动会自动选中这次用过的方案。',
          style: DswTypography.caption.copyWith(color: c.labelTertiary),
        ),
        const SizedBox(height: 16),
        Row(
          children: <Widget>[
            DswPrimaryButton(
              label: '打开配置向导',
              onPressed: () => unawaited(_openWizard_()),
            ),
            const Spacer(),
            DswGhostButton(
              label: '离线进入',
              onPressed: () => widget.connection.enterOffline(),
            ),
          ],
        ),
        if (widget.connection.configPath.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          Text(
            '配置文件：${widget.connection.configPath}',
            style: DswTypography.caption.copyWith(
              fontSize: 11,
              color: c.labelCaption,
            ),
          ),
        ],
      ],
    );
  }

  /// 渲染一组连接方式；`kinds` 为空表示“其余 kind”。
  List<Widget> _group_(BuildContext context, String title, List<String> kinds) {
    final List<ConnectionView> profiles = widget.connection.profiles
        .where(
          (ConnectionView profile) => kinds.isEmpty
              ? !const <String>[
                  'local-launch',
                  'local-attach',
                  'tcp',
                ].contains(profile.kind)
              : kinds.contains(profile.kind),
        )
        .toList();
    if (profiles.isEmpty) {
      return const <Widget>[];
    }
    return <Widget>[
      DswSectionLabel(title),
      for (final ConnectionView profile in profiles)
        _ProfileChoice(
          profile: profile,
          selected: profile.name == _selectedName,
          isLastUsed: profile.name == widget.connection.lastUsedName,
          description: widget.connection.describeKind(profile.kind),
          onTap: () => _selectProfile_(profile),
        ),
      const SizedBox(height: 12),
    ];
  }

  /// 倒计时 / 连接中 / 失败说明。
  Widget _status_(BuildContext context) {
    final DswColors c = context.dsw;
    final ConnectionView? selected = _profileByName_(_selectedName);
    final String selectedName = selected?.name ?? '';

    if (_connecting || widget.connection.phase == ConnectionPhase.connecting) {
      return Row(
        children: <Widget>[
          const SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              '正在连接「$selectedName」…',
              style: DswTypography.caption.copyWith(color: c.labelTertiary),
            ),
          ),
        ],
      );
    }

    if (_message.isNotEmpty) {
      return _errorBox_(context, _message);
    }
    if (widget.connection.phase == ConnectionPhase.failed &&
        widget.connection.error.isNotEmpty) {
      return _errorBox_(context, widget.connection.error);
    }

    if (_secondsLeft > 0) {
      return Row(
        children: <Widget>[
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
            decoration: BoxDecoration(
              color: c.bgLayer3,
              borderRadius: BorderRadius.circular(999),
            ),
            child: Text(
              '$_secondsLeft 秒后自动连接「$selectedName」',
              style: DswTypography.caption.copyWith(
                fontSize: 11,
                color: c.labelSecondary,
              ),
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(999),
              child: LinearProgressIndicator(
                value: _secondsLeft / StartupScreen.countdownDuration.inSeconds,
                minHeight: 3,
                backgroundColor: c.bgLayer3,
              ),
            ),
          ),
        ],
      );
    }

    if (_countdownCancelled) {
      return Text(
        '已取消自动连接；选好后点「连接」。',
        style: DswTypography.caption.copyWith(color: c.labelTertiary),
      );
    }

    return Text(
      '已选「$selectedName」；点「连接」开始。',
      style: DswTypography.caption.copyWith(color: c.labelTertiary),
    );
  }

  /// 底部按钮。
  Widget _actions_(BuildContext context) {
    final bool busy =
        _connecting || widget.connection.phase == ConnectionPhase.connecting;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        DswPrimaryButton(
          label: busy ? '连接中…' : '连接',
          onPressed: busy ? null : () => unawaited(_connectSelected_()),
        ),
        const SizedBox(width: 8),
        DswGhostButton(
          label: '取消倒计时',
          onPressed: _secondsLeft > 0 ? _cancelCountdown_ : null,
        ),
        const SizedBox(width: 8),
        DswGhostButton(
          label: '新建 / 编辑连接方式…',
          onPressed: busy ? null : () => unawaited(_openConnectionDialog_()),
        ),
        const Spacer(),
        DswGhostButton(
          label: '离线进入',
          onPressed: busy ? null : () => widget.connection.enterOffline(),
        ),
      ],
    );
  }

  /// 红色错误条。
  Widget _errorBox_(BuildContext context, String message) {
    final DswColors c = context.dsw;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
        color: c.bgLayer3,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: c.stateError.withValues(alpha: 0.35)),
      ),
      child: Text(
        message,
        style: DswTypography.caption.copyWith(color: c.stateError),
      ),
    );
  }
}

/// 启动页里的一条连接方式。
class _ProfileChoice extends StatelessWidget {
  const _ProfileChoice({
    required this.profile,
    required this.selected,
    required this.isLastUsed,
    required this.description,
    required this.onTap,
  });

  final ConnectionView profile;
  final bool selected;
  final bool isLastUsed;
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
            margin: const EdgeInsets.only(bottom: 4),
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
            decoration: BoxDecoration(
              color: selected
                  ? c.interactiveActive
                  : (hovered ? c.interactiveHover : Colors.transparent),
              borderRadius: BorderRadius.circular(10),
              border: Border.all(
                color: selected ? c.brandPrimary : Colors.transparent,
              ),
            ),
            child: Row(
              children: <Widget>[
                Icon(
                  selected
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                  size: 16,
                  color: selected ? c.brandPrimary : c.labelCaption,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Flexible(
                            child: Text(
                              profile.name,
                              overflow: TextOverflow.ellipsis,
                              style: DswTypography.body.copyWith(
                                fontSize: 13,
                                fontWeight: FontWeight.w500,
                                color: c.labelPrimary,
                              ),
                            ),
                          ),
                          if (isLastUsed) ...<Widget>[
                            const SizedBox(width: 6),
                            _Tag(label: '上次使用'),
                          ],
                          const SizedBox(width: 6),
                          _Tag(label: _kindLabel_(profile.kind)),
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        description,
                        style: DswTypography.caption.copyWith(
                          fontSize: 11,
                          color: c.labelTertiary,
                        ),
                      ),
                      if (profile.summary.isNotEmpty)
                        Text(
                          profile.summary,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
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
          ),
        );
      },
    );
  }

  /// kind → 界面上的短标签。
  static String _kindLabel_(String kind) {
    switch (kind) {
      case 'local-launch':
        return '本机·启动';
      case 'local-attach':
        return '本机·附着';
      case 'tcp':
        return '远程·TCP';
      default:
        return kind;
    }
  }
}

/// 小标签（kind / 上次使用）。
class _Tag extends StatelessWidget {
  const _Tag({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final DswColors c = context.dsw;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(
        color: c.bgLayer3,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: DswTypography.caption.copyWith(
          fontSize: 10,
          color: c.labelTertiary,
        ),
      ),
    );
  }
}
