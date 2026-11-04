// 连接 kb_core 这条链路的测试。
//
// 用一份**[假的 KbClientApi]** 驱动，因此完全不需要初始化原生库
// （`RustLib.init()`）——这正是把界面收在 `KbClientApi` 后面的目的。
//
// 覆盖两件事：
//   1. 状态机：首次运行 / 启动页默认选择 / 最近一次持久化 / 列工作区 /
//      按需拉会话 / 失败 / 断开 / 离线进入；
//   2. 界面：启动页的倒计时与分组、连上之后侧边栏显示服务端的工作区与会话、
//      首次运行的对话框能保存并连接。

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kb_admin_desktop/src/app.dart';
import 'package:kb_admin_desktop/src/services/kb_client_api.dart';
import 'package:kb_admin_desktop/src/services/local_store.dart';
import 'package:kb_admin_desktop/src/state/app_controller.dart';
import 'package:kb_admin_desktop/src/state/connection_controller.dart';
import 'package:kb_admin_desktop/src/widgets/common/dsw_controls.dart';
import 'package:kb_admin_desktop/src/widgets/common/dsw_icons.dart';
import 'package:kb_admin_desktop/src/widgets/connection/connection_dialog.dart';
import 'package:kb_admin_desktop/src/widgets/sidebar/server_workspace_list.dart';
import 'package:kb_admin_desktop/src/widgets/startup/startup_screen.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// 一条假的工作区。
WorkspaceView _workspace(String id, String name) =>
    WorkspaceView(id: id, name: name, path: '/tmp/$id');

/// 一条假的会话。
SessionView _session(String id, String workspaceId, String title) =>
    SessionView(
      id: id,
      workspaceId: workspaceId,
      title: title,
      updatedAtMillis: 1760000000000,
      turnCount: 3,
    );

/// 一条假的连接方式。
ConnectionView _profile(String name, {String kind = 'local-launch'}) =>
    ConnectionView(
      name: name,
      kind: kind,
      summary: kind == 'tcp'
          ? '经 kb_core_rproxy 连 192.168.1.5:8788'
          : '启动本机 kb-core 并连 /run/user/1000/llm_kb',
      kbCore: kind == 'local-launch' ? '/usr/local/bin/kb-core' : '',
      runtimeDir: kind == 'tcp' ? '' : '/run/user/1000/llm_kb',
      storageDir: kind == 'local-launch' ? '/run/user/1000/llm_kb/storage' : '',
      address: kind == 'tcp' ? '192.168.1.5:8788' : '',
    );

/// 假的原生接口。
///
/// 它维护一份**可变的**服务端状态（工作区列表 + 每个工作区的会话），这样
/// "增删之后列表确实变了"可以被断言；纯查询类用例拿到的仍是预置的那两条。
class _FakeApi implements KbClientApi {
  _FakeApi({
    this.configExists = true,
    this.profiles = const <ConnectionView>[],
    this.defaultName,
    this.lastUsedName = '',
    this.connectOk = true,
    this.connectError = '',
    this.suggestionOverride,
    this.configError = '',
  });

  bool configExists;
  List<ConnectionView> profiles;
  String? defaultName;
  String lastUsedName;
  bool connectOk;
  String connectError;

  /// 覆盖 `suggestLocalConnection` 的返回值（用来测“预填值缺存储目录”）。
  ConnectionView? suggestionOverride;

  /// 读取配置时返回的错误（用来测“坏配置不当成首次运行”）。
  String configError;

  /// 记录调用，便于断言"界面确实走了这条路"。
  final List<String> calls = <String>[];
  final List<ConnectionView> savedConnections = <ConnectionView>[];

  /// 服务端状态。
  final List<WorkspaceView> workspaces = <WorkspaceView>[
    _workspace('w-1', '笔记'),
    _workspace('w-2', '资料'),
  ];
  final Map<String, List<SessionView>> sessions = <String, List<SessionView>>{
    'w-1': <SessionView>[_session('s-w-1', 'w-1', '会话@w-1')],
    'w-2': <SessionView>[_session('s-w-2', 'w-2', '会话@w-2')],
  };

  /// 每个会话的正文（`getSession` / `ask` 读写它）。
  final Map<String, SessionDetailReport> details =
      <String, SessionDetailReport>{};

  /// 给新建出来的对象发一个可预期的标识。
  int _sequence = 0;

  @override
  Future<String> configFilePath() async => '/tmp/fake/config.toml';

  @override
  Future<ConfigView> loadConfig() async {
    calls.add('loadConfig');
    return ConfigView(
      path: '/tmp/fake/config.toml',
      exists: configExists,
      defaultName: defaultName ?? (profiles.isEmpty ? '' : profiles.first.name),
      lastUsedName: lastUsedName,
      connections: profiles,
      error: configError,
    );
  }

  @override
  Future<String> saveConfig({
    required String defaultName,
    required List<ConnectionView> connections,
  }) async {
    calls.add('saveConfig:$defaultName');
    profiles = connections;
    configExists = true;
    savedConnections
      ..clear()
      ..addAll(connections);
    if (!connections.any(
      (ConnectionView profile) => profile.name == lastUsedName,
    )) {
      lastUsedName = '';
    }
    return '';
  }

  @override
  Future<String> rememberLastUsed(String name) async {
    calls.add('rememberLastUsed:$name');
    lastUsedName = name;
    return '';
  }

  @override
  Future<List<String>> connectionKinds() async => <String>[
    'local-launch',
    'local-attach',
    'tcp',
  ];

  @override
  Future<String> connectionKindDescription(String kind) async => '说明:$kind';

  @override
  Future<ConnectionView> suggestLocalConnection(String name) async {
    calls.add('suggest:$name');
    return suggestionOverride ?? _profile(name);
  }

  @override
  Future<ConnectReport> connect(ConnectionView profile) async {
    calls.add('connect:${profile.name}');
    if (!connectOk) {
      return ConnectReport(
        ok: false,
        profileName: profile.name,
        serverVersion: '',
        protocolVersion: 0,
        isLocal: false,
        launchedPid: 0,
        error: connectError,
      );
    }
    return ConnectReport(
      ok: true,
      profileName: profile.name,
      serverVersion: '9.9.9',
      protocolVersion: 1,
      isLocal: profile.kind != 'tcp',
      launchedPid: profile.kind == 'local-launch' ? 4242 : 0,
      error: '',
    );
  }

  @override
  Future<ConnectReport> testConnection(ConnectionView profile) async {
    calls.add('testConnection:${profile.name}');
    if (!connectOk) {
      return ConnectReport(
        ok: false,
        profileName: profile.name,
        serverVersion: '',
        protocolVersion: 0,
        isLocal: false,
        launchedPid: 0,
        error: connectError,
      );
    }
    return ConnectReport(
      ok: true,
      profileName: profile.name,
      serverVersion: '9.9.9',
      protocolVersion: 1,
      isLocal: profile.kind != 'tcp',
      launchedPid: 0,
      error: '',
    );
  }

  @override
  Future<void> disconnect() async => calls.add('disconnect');

  @override
  Future<WorkspacesReport> listWorkspaces() async {
    calls.add('listWorkspaces');
    return WorkspacesReport(
      ok: true,
      workspaces: List<WorkspaceView>.of(workspaces),
      error: '',
    );
  }

  @override
  Future<SessionsReport> listSessions(String workspaceId) async {
    calls.add('listSessions:$workspaceId');
    return SessionsReport(
      ok: true,
      sessions: List<SessionView>.of(
        sessions[workspaceId] ?? const <SessionView>[],
      ),
      error: '',
    );
  }

  @override
  Future<WorkspaceReport> addWorkspace({
    required String name,
    required String path,
  }) async {
    calls.add('addWorkspace:$name');
    final WorkspaceView workspace = WorkspaceView(
      id: 'w-new-${_sequence++}',
      name: name,
      path: path,
    );
    workspaces.add(workspace);
    return WorkspaceReport(ok: true, workspace: workspace, error: '');
  }

  @override
  Future<OpReport> removeWorkspace(String workspaceId) async {
    calls.add('removeWorkspace:$workspaceId');
    workspaces.removeWhere((WorkspaceView item) => item.id == workspaceId);
    sessions.remove(workspaceId);
    return const OpReport(ok: true, error: '');
  }

  @override
  Future<WorkspaceReport> renameWorkspace({
    required String workspaceId,
    required String name,
  }) async {
    calls.add('renameWorkspace:$workspaceId');
    final int index = workspaces.indexWhere(
      (WorkspaceView item) => item.id == workspaceId,
    );
    final WorkspaceView renamed = WorkspaceView(
      id: workspaceId,
      name: name.trim(),
      path: index >= 0 ? workspaces[index].path : '',
    );
    if (index >= 0) {
      workspaces[index] = renamed;
    }
    return WorkspaceReport(ok: true, workspace: renamed, error: '');
  }

  @override
  Future<SessionReport> createSession({
    required String workspaceId,
    required String title,
    required String turnId,
    required String question,
  }) async {
    calls.add('createSession:$workspaceId');
    final bool withQuestion = turnId.trim().isNotEmpty;
    // 与 kb_core 同口径：没有显式标题时，从第一个问题推导。
    final String resolvedTitle = title.trim().isNotEmpty
        ? title.trim()
        : (withQuestion ? question : '新会话');
    final SessionView session = SessionView(
      id: 's-new-${_sequence++}',
      workspaceId: workspaceId,
      title: resolvedTitle,
      updatedAtMillis: 1760000000000,
      turnCount: withQuestion ? 1 : 0,
    );
    sessions.putIfAbsent(workspaceId, () => <SessionView>[]).insert(0, session);
    if (withQuestion) {
      details[session.id] = SessionDetailReport(
        ok: true,
        session: session,
        turns: <TurnView>[_turn(turnId, 'user', question)],
        error: '',
      );
    }
    return SessionReport(ok: true, session: session, error: '');
  }

  @override
  Future<OpReport> removeSession({
    required String workspaceId,
    required String sessionId,
  }) async {
    calls.add('removeSession:$workspaceId:$sessionId');
    sessions[workspaceId]?.removeWhere(
      (SessionView item) => item.id == sessionId,
    );
    return const OpReport(ok: true, error: '');
  }

  @override
  Future<SessionReport> renameSession({
    required String workspaceId,
    required String sessionId,
    required String title,
  }) async {
    calls.add('renameSession:$sessionId');
    final String resolved = title.trim().isNotEmpty
        ? title.trim()
        : _deriveTitle_(sessionId);
    final List<SessionView>? list = sessions[workspaceId];
    final int index =
        list?.indexWhere((SessionView item) => item.id == sessionId) ?? -1;
    final SessionView renamed = list != null && index >= 0
        ? SessionView(
            id: list[index].id,
            workspaceId: list[index].workspaceId,
            title: resolved,
            updatedAtMillis: list[index].updatedAtMillis,
            turnCount: list[index].turnCount,
          )
        : _session(sessionId, workspaceId, resolved);
    if (list != null && index >= 0) {
      list[index] = renamed;
    }
    final SessionDetailReport? detail = details[sessionId];
    if (detail != null) {
      details[sessionId] = SessionDetailReport(
        ok: true,
        session: renamed,
        turns: detail.turns,
        error: '',
      );
    }
    return SessionReport(ok: true, session: renamed, error: '');
  }

  @override
  Future<SessionDetailReport> getSession({
    required String workspaceId,
    required String sessionId,
  }) async {
    calls.add('getSession:$sessionId');
    final SessionDetailReport detail =
        details[sessionId] ?? _detailFor_(workspaceId, sessionId);
    details[sessionId] = detail;
    return detail;
  }

  @override
  Future<SessionDetailReport> ask({
    required String workspaceId,
    required String sessionId,
    required String turnId,
    required String question,
  }) async {
    calls.add('ask:$sessionId');
    // 与 kb_core 里的临时模拟 LLM 同口径：回答是问题的逆序（按字符），
    // 且回答标识由用户回合标识确定性推导。
    final String answer = String.fromCharCodes(
      question.runes.toList().reversed,
    );
    final String answerId = '$turnId-a';
    final SessionDetailReport current =
        details[sessionId] ?? _detailFor_(workspaceId, sessionId);

    // 幂等：已经有这一轮回答就原样返回（草稿流程会先建会话带上问题，再 Ask）。
    if (current.turns.any((TurnView turn) => turn.id == answerId)) {
      return current;
    }
    final bool hasUser = current.turns.any(
      (TurnView turn) => turn.id == turnId,
    );
    final List<TurnView> turns = <TurnView>[
      ...current.turns,
      if (!hasUser) _turn(turnId, 'user', question),
      _turn(answerId, 'assistant', answer),
    ];
    final SessionDetailReport next = SessionDetailReport(
      ok: true,
      session: SessionView(
        id: sessionId,
        workspaceId: workspaceId,
        title: current.session.title,
        updatedAtMillis: 1760000001000,
        turnCount: turns.length,
      ),
      turns: turns,
      error: '',
    );
    details[sessionId] = next;

    // 会话摘要里的 `turn_count` 也跟着变，列表下次读到的应当是新的。
    final List<SessionView>? list = sessions[workspaceId];
    final int index =
        list?.indexWhere((SessionView item) => item.id == sessionId) ?? -1;
    if (list != null && index >= 0) {
      list[index] = next.session;
    }
    return next;
  }

  /// 空白改名时的退路：取首条用户消息（与 `kb_core` 的推导口径一致）。
  String _deriveTitle_(String sessionId) {
    for (final TurnView turn
        in details[sessionId]?.turns ?? const <TurnView>[]) {
      if (turn.role == 'user' && turn.text.trim().isNotEmpty) {
        return turn.text.trim();
      }
    }
    return '新会话';
  }

  /// 没拉过正文时给一个空会话。
  SessionDetailReport _detailFor_(String workspaceId, String sessionId) {
    SessionView? summary;
    for (final SessionView item
        in sessions[workspaceId] ?? const <SessionView>[]) {
      if (item.id == sessionId) {
        summary = item;
        break;
      }
    }
    return SessionDetailReport(
      ok: true,
      session: summary ?? _session(sessionId, workspaceId, '会话'),
      turns: const <TurnView>[],
      error: '',
    );
  }
}

/// 造一条服务端消息。
TurnView _turn(String id, String role, String text) => TurnView(
  id: id,
  role: role,
  text: text,
  reasoning: '',
  state: 'done',
  notice: '',
  noticeIsError: false,
);

/// 跑在内存里的应用状态（与 `app_shell_test.dart` 同一套做法）。
Future<AppController> _appController() async {
  SharedPreferences.setMockInitialValues(<String, Object>{});
  final LocalStore store = await LocalStore.open();
  return AppController(store, snapshot: store.load());
}

/// 初始化并连上启动页默认选中的那条。
///
/// 大多数用例只关心“已经连上之后”的行为；这一步把新的启动流程（initialize
/// 不自动连接 → 连接 startupProfile）收起来，免得每个用例都写两遍。
Future<ConnectionController> _connected(_FakeApi api) async {
  final ConnectionController connection = ConnectionController(api);
  await connection.initialize();
  await connection.connectStartupProfile();
  return connection;
}

/// 渲染整个应用（宽屏）；启动页与主界面用例共用。
Future<void> _pumpApp(
  WidgetTester tester,
  ConnectionController connection,
) async {
  tester.view.physicalSize = const Size(1400, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final AppController app = await _appController();
  await tester.pumpWidget(KbAdminApp(controller: app, connection: connection));
}

void main() {
  /// 测试"配置不存在"会走到首次运行，而不是直接报错。
  ///
  /// - 手段：假接口说 `exists == false`，跑一次 `initialize()`。
  /// - 判断：`firstRun` 为真、状态仍是 `idle`、没有错误——
  ///   界面据此弹「连接 kb_core」对话框。
  test('没有配置文件时进入首次运行', () async {
    final _FakeApi api = _FakeApi(configExists: false);
    final ConnectionController connection = ConnectionController(api);

    await connection.initialize();

    expect(connection.initialized, isTrue);
    expect(connection.firstRun, isTrue);
    expect(connection.phase, ConnectionPhase.idle);
    expect(connection.error, isEmpty);
    expect(connection.profiles, isEmpty);
  });

  /// 测试 initialize 只读配置、不自动连接；启动目标按 last_used 选择。
  ///
  /// - 手段：两条连接（本机 / 实验室），`default` 是本机、`last_used` 是实验室；
  ///   先 `initialize()`，再 `connectStartupProfile()`。
  /// - 判断：initialize 之后仍是 `idle`、`connected` 为假、没有发起连接；
  ///   `startupProfile` 是「实验室」；连接后状态是 `connected`，调用记录依次是
  ///   `rememberLastUsed:实验室` → `connect:实验室` → `listWorkspaces`。
  test('初始化不自动连接，启动目标是最近一次使用的那条', () async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[
        _profile('本机'),
        _profile('实验室', kind: 'tcp'),
      ],
      defaultName: '本机',
      lastUsedName: '实验室',
    );
    final ConnectionController connection = ConnectionController(api);

    await connection.initialize();

    expect(connection.initialized, isTrue);
    expect(connection.connected, isFalse);
    expect(connection.phase, ConnectionPhase.idle);
    expect(connection.startupProfile?.name, '实验室');
    expect(api.calls, <String>['loadConfig']);

    await connection.connectStartupProfile();

    expect(connection.connected, isTrue);
    expect(connection.profileName, '实验室');
    expect(connection.startupFinished, isTrue);
    expect(api.calls, <String>[
      'loadConfig',
      'rememberLastUsed:实验室',
      'connect:实验室',
      'listWorkspaces',
    ]);
    expect(connection.isLocal, isFalse, reason: 'tcp 是远程连接');
  });

  /// 测试启动目标的回退顺序：last_used → default → 第一条。
  ///
  /// - 手段：同一组两条连接，分别把 `last_used` / `default` 置为有效或无效名字，
  ///   调 `startupProfile`。
  /// - 判断：有效 `last_used` 胜出；它为空时用 `default`；两者都为空或指向不存在的
  ///   项时用第一条。界面据此决定启动页默认选中哪条。
  test('启动目标按 last_used → default → 第一条回退', () async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[
        _profile('本机'),
        _profile('实验室', kind: 'tcp'),
      ],
      defaultName: '实验室',
      lastUsedName: '',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    expect(connection.startupProfile?.name, '实验室', reason: '回退到 default');

    api.lastUsedName = '本机';
    await connection.reloadConfig();
    expect(connection.startupProfile?.name, '本机', reason: 'last_used 优先');

    api.lastUsedName = '不存在';
    api.defaultName = '也不存在';
    await connection.reloadConfig();
    expect(connection.startupProfile?.name, '本机', reason: '都无效时用第一条');
  });

  /// 测试"选中工作区 → 按需拉会话"，且同一个工作区不会重复拉。
  ///
  /// - 手段：连上之后 `selectWorkspace('w-2')` 两次。
  /// - 判断：`sessionsOf` 拿到那条会话；假接口里 `listSessions:w-2` **只被调用一次**
  ///   （第二次命中缓存）。
  test('选中工作区时按需拉会话且不重复拉', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    await connection.selectWorkspace('w-2');
    await connection.selectWorkspace('w-2');

    expect(connection.selectedWorkspaceId, 'w-2');
    expect(
      connection.sessionsOf('w-2').map((SessionView s) => s.title),
      <String>['会话@w-2'],
    );
    expect(
      api.calls.where((String call) => call == 'listSessions:w-2').length,
      1,
    );
  });

  /// 测试连接失败会把原因留在状态里，而不是抛出去。
  ///
  /// - 手段：假接口的 `connect` 返回 `ok == false` 且带一句错误。
  /// - 判断：状态是 `failed`、`connected` 为假、`error` 就是那句话，
  ///   工作区列表为空，`startupFinished` 仍为假（启动页还没结束）。
  test('连接失败时状态是 failed 且带错误说明', () async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机')],
      connectOk: false,
      connectError: '本机 IPC 传输失败: 等待服务端端点超时（5000 ms）',
    );
    final ConnectionController connection = ConnectionController(api);

    await connection.initialize();
    await connection.connectStartupProfile();

    expect(connection.phase, ConnectionPhase.failed);
    expect(connection.connected, isFalse);
    expect(connection.startupFinished, isFalse);
    expect(connection.error, contains('等待服务端端点超时'));
    expect(connection.workspaces, isEmpty);
  });

  /// 测试断开之后列表被清空。
  ///
  /// - 手段：连上并列出工作区之后 `disconnect()`。
  /// - 判断：状态回到 `idle`、工作区与已拉到的会话都没了、假接口收到 `disconnect`。
  test('断开后清空工作区与会话', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);
    await connection.selectWorkspace('w-1');

    await connection.disconnect();

    expect(connection.phase, ConnectionPhase.idle);
    expect(connection.workspaces, isEmpty);
    expect(connection.selectedWorkspaceId, isNull);
    expect(api.calls, contains('disconnect'));
  });

  /// 测试删除最后一条连接方式会变成“空配置”（配置向导信号）并断开当前连接。
  ///
  /// - 手段：只有一条「本机」并已连上，调 `removeProfile('本机')`。
  /// - 判断：返回空串；`profiles` 为空；`firstRun` 变回真；当前连接被断开
  ///   （假接口收到 `disconnect`）；存下去的连接列表为空。
  test('删除最后一条配置后回到首次运行状态', () async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机')],
      lastUsedName: '本机',
    );
    final ConnectionController connection = await _connected(api);

    final String error = await connection.removeProfile('本机');

    expect(error, isEmpty);
    expect(connection.profiles, isEmpty);
    expect(connection.firstRun, isTrue);
    expect(connection.connected, isFalse);
    expect(api.calls, contains('disconnect'));
    expect(api.savedConnections, isEmpty);
  });

  /// 测试首次运行的对话框：预填本机连接，点「保存并连接」会写配置并连上。
  ///
  /// - 手段：把对话框单独放进一个最小 App 里（配置不存在），等预填完成，
  ///   点「保存并连接」。
  /// - 判断：对话框自己关掉；假接口收到 `suggest:本机`、`saveConfig:本机`、
  ///   `connect:本机`；控制器进入 `connected`。
  testWidgets('首次运行对话框保存并连接', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(configExists: false);
    final ConnectionController connection = ConnectionController(api);

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (BuildContext context) => TextButton(
            onPressed: () =>
                showConnectionDialog(context, connection, firstRun: true),
            child: const Text('open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.text('连接 kb_core'), findsOneWidget);

    await tester.tap(find.text('保存并连接'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('suggest:本机'));
    expect(api.calls, contains('saveConfig:本机'));
    expect(api.calls, contains('connect:本机'));
    expect(connection.connected, isTrue);
    expect(find.text('连接 kb_core'), findsNothing);
  });

  /// 测试本机启动的存储目录没有默认值，界面必须拦下空值。
  ///
  /// - 手段：假接口的预填建议里 `storageDir` 为空；打开首次运行对话框，
  ///   等预填完成，直接点「保存并连接」。
  /// - 判断：出现「请填写存储目录（不会自动使用临时目录）」；假接口没有收到
  ///   `saveConfig` / `connect`（不会拿空存储目录去起进程）。
  testWidgets('本机启动必须自己填存储目录', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      configExists: false,
      suggestionOverride: const ConnectionView(
        name: '本机',
        kind: 'local-launch',
        summary: '启动本机 kb-core 并连 /run/user/1000/llm_kb',
        kbCore: '/usr/local/bin/kb-core',
        runtimeDir: '/run/user/1000/llm_kb',
        storageDir: '',
        address: '',
      ),
    );
    final ConnectionController connection = ConnectionController(api);

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (BuildContext context) => TextButton(
            onPressed: () =>
                showConnectionDialog(context, connection, firstRun: true),
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    expect(find.textContaining('存储目录没有默认值'), findsOneWidget);
    await tester.tap(find.text('保存并连接'));
    await tester.pumpAndSettle();

    expect(find.textContaining('请填写存储目录'), findsOneWidget);
    expect(
      api.calls.where((String call) => call.startsWith('saveConfig')),
      isEmpty,
    );
    expect(
      api.calls.where((String call) => call.startsWith('connect:')),
      isEmpty,
    );
  });

  /// 测试启动页默认选中「最近一次使用」，并按 5 秒倒计时自动连接。
  ///
  /// - 手段：两条连接（本机 / 远程 TCP），`last_used = 实验室`、`default = 本机`；
  ///   先 initialize 再渲染应用，等倒计时走完。
  /// - 判断：启动页按「本机（Unix domain socket）」与「远程（TCP）」分组；
  ///   「实验室」带「上次使用」标签；文案出现「5 秒后自动连接「实验室」」；
  ///   5 秒后假接口收到 `connect:实验室`，启动页消失、主界面出现。
  testWidgets('启动页默认选最近一次并 5 秒后自动连接', (WidgetTester tester) async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[
        _profile('本机'),
        _profile('实验室', kind: 'tcp'),
      ],
      defaultName: '本机',
      lastUsedName: '实验室',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    // 第一帧完成后再等一帧：倒计时是在 post-frame 回调里启动的。
    await tester.pump();
    await tester.pump();

    expect(find.byType(StartupScreen), findsOneWidget);
    expect(find.text('本机（Unix domain socket）'), findsOneWidget);
    expect(find.text('远程（TCP）'), findsOneWidget);
    expect(find.text('上次使用'), findsOneWidget);
    expect(find.textContaining('秒后自动连接「实验室」'), findsOneWidget);

    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();

    expect(api.calls, contains('connect:实验室'));
    expect(connection.connected, isTrue);
    expect(connection.profileName, '实验室');
    expect(find.byType(StartupScreen), findsNothing);
    expect(find.textContaining('kb_core 9.9.9'), findsOneWidget);
  });

  /// 测试用户改选连接方式会取消倒计时，并且只把选择写回配置、不立刻连接。
  ///
  /// - 手段：启动页出现后点「实验室」（默认选的是「本机」），再等 6 秒。
  /// - 判断：界面显示「已取消自动连接」；假接口收到 `rememberLastUsed:实验室`
  ///   但没有 `connect:`；启动页仍在（用户还没点「连接」）。
  testWidgets('用户改选连接方式会取消倒计时', (WidgetTester tester) async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[
        _profile('本机'),
        _profile('实验室', kind: 'tcp'),
      ],
      lastUsedName: '本机',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pump();

    await tester.tap(find.text('实验室'));
    await tester.pump();

    expect(find.textContaining('已取消自动连接'), findsOneWidget);
    expect(api.calls, contains('rememberLastUsed:实验室'));

    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();

    expect(
      api.calls.where((String call) => call.startsWith('connect:')),
      isEmpty,
      reason: '倒计时已被用户操作取消，不应当自动连接',
    );
    expect(find.byType(StartupScreen), findsOneWidget);
    expect(connection.connected, isFalse);
  });

  /// 测试连接失败会停在启动页报错，不会自动重试。
  ///
  /// - 手段：假接口的 `connect` 返回失败；等倒计时走完，再等 6 秒。
  /// - 判断：启动页仍在并显示错误说明；`connect:` 只被调用一次（没有自动重试）。
  testWidgets('连接失败停在启动页且不自动重试', (WidgetTester tester) async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机')],
      lastUsedName: '本机',
      connectOk: false,
      connectError: '本机 IPC 传输失败: 连不上',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();

    expect(find.byType(StartupScreen), findsOneWidget);
    expect(find.textContaining('连不上'), findsOneWidget);
    final int attempts = api.calls
        .where((String call) => call.startsWith('connect:'))
        .length;
    expect(attempts, 1);

    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();

    expect(
      api.calls.where((String call) => call.startsWith('connect:')).length,
      attempts,
      reason: '失败后不应当自动重试',
    );
  });

  /// 测试启动页可以「离线进入」主界面（不连接，也不清掉配置）。
  ///
  /// - 手段：启动页出现后点「离线进入」。
  /// - 判断：`startupFinished` 为真、启动页消失、主界面的设置按钮出现；
  ///   假接口没有收到任何 `connect:`。
  testWidgets('启动页可以离线进入主界面', (WidgetTester tester) async {
    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机')],
      lastUsedName: '本机',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pump();

    await tester.tap(find.text('离线进入'));
    await tester.pumpAndSettle();

    expect(connection.startupFinished, isTrue);
    expect(find.byType(StartupScreen), findsNothing);
    expect(find.text('设置'), findsWidgets);
    expect(
      api.calls.where((String call) => call.startsWith('connect:')),
      isEmpty,
    );
  });

  /// 测试坏配置不会被当成“首次运行”去打开配置向导（避免静默覆盖手改内容）。
  ///
  /// - 手段：假接口返回 `error` 非空且没有连接项，渲染启动页。
  /// - 判断：启动页显示配置错误，不出现「配置向导」；用户应当去修文件。
  testWidgets('配置坏了不自动打开配置向导', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      profiles: const <ConnectionView>[],
      configError: '配置文件不是合法 TOML: 预期键值',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pumpAndSettle();

    expect(find.byType(StartupScreen), findsOneWidget);
    expect(find.textContaining('配置文件不是合法 TOML'), findsOneWidget);
    expect(find.textContaining('配置向导'), findsNothing);
  });

  /// 测试首次运行没有配置时，启动页会自动打开**配置向导**；取消后停在启动页。
  ///
  /// - 手段：假接口说 `exists == false`，渲染应用；等向导出现后点「取消」。
  /// - 判断：先看到向导的第一步与「下一步」；取消后看到启动页的
  ///   「还没有配置过连接方式。」与「打开配置向导」按钮。
  testWidgets('首次运行启动页自动打开配置向导', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(configExists: false);
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pumpAndSettle();

    expect(find.byType(StartupScreen), findsOneWidget);
    expect(find.text('配置向导 · 选择连接方式'), findsOneWidget);
    expect(find.text('下一步'), findsOneWidget);

    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();

    expect(find.text('下一步'), findsNothing);
    expect(find.text('还没有配置过连接方式。'), findsOneWidget);
    expect(find.text('打开配置向导'), findsOneWidget);
  });

  /// 测试配置向导能走完首次连接：选方式 → 填参数（含必填校验）→ 确认 → 连接。
  ///
  /// - 手段：假接口的预填建议里存储目录为空；依次点「下一步」，
  ///   先验证空存储被拦下，再填入存储目录继续，最后「保存并连接」。
  /// - 判断：向导三步标题依次出现；空存储时提示「请填写存储目录」；
  ///   填完后假接口收到 `saveConfig` / `connect`，向导关闭并进入主界面。
  testWidgets('配置向导可以完成首次连接', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      configExists: false,
      suggestionOverride: const ConnectionView(
        name: '本机',
        kind: 'local-launch',
        summary: '启动本机 kb-core 并连 /run/kb',
        kbCore: '/usr/bin/kb-core',
        runtimeDir: '/run/kb',
        storageDir: '',
        address: '',
      ),
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await _pumpApp(tester, connection);
    await tester.pump();
    await tester.pumpAndSettle();
    expect(find.text('配置向导 · 选择连接方式'), findsOneWidget);

    await tester.tap(find.text('下一步'));
    await tester.pumpAndSettle();
    expect(find.text('配置向导 · 填写参数'), findsOneWidget);
    expect(find.textContaining('存储目录没有默认值'), findsOneWidget);

    await tester.tap(find.text('下一步'));
    await tester.pumpAndSettle();
    expect(find.textContaining('请填写存储目录'), findsOneWidget);

    // 表单里的 4 个 TextField 依次是主机名 / kb-core / 运行时 / 存储。
    await tester.enterText(find.byType(TextField).last, '/data/kb');
    await tester.tap(find.text('下一步'));
    await tester.pumpAndSettle();
    expect(find.text('配置向导 · 确认并连接'), findsOneWidget);
    expect(find.text('/data/kb'), findsOneWidget);

    await tester.tap(find.text('保存并连接'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('saveConfig:本机'));
    expect(api.calls, contains('connect:本机'));
    expect(connection.connected, isTrue);
    expect(find.textContaining('kb_core 9.9.9'), findsOneWidget);
  });

  /// 测试连接方式行悬停出现的齿轮菜单可以「测试连接」。
  ///
  /// - 手段：打开连接对话框，用鼠标移到「本机」行上，点齿轮 →「测试连接」。
  /// - 判断：假接口收到 `testConnection:本机`；对话框显示「测试成功」；
  ///   当前连接没有被替换（`connection.connected` 仍为假）。
  testWidgets('连接方式悬停齿轮可以测试连接', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机')],
      lastUsedName: '本机',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (BuildContext context) => TextButton(
            onPressed: () => showConnectionDialog(context, connection),
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    final TestGesture gesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await gesture.addPointer(location: Offset.zero);
    addTearDown(gesture.removePointer);
    await gesture.moveTo(tester.getCenter(find.text('本机  ·  local-launch')));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('连接方式操作'));
    await tester.pumpAndSettle();
    expect(find.text('测试连接'), findsOneWidget);
    await tester.tap(find.text('测试连接'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('testConnection:本机'));
    expect(find.textContaining('测试成功'), findsOneWidget);
    expect(connection.connected, isFalse);
  });

  /// 测试连接方式行悬停齿轮菜单可以删除配置（并落到 saveConfig）。
  ///
  /// - 手段：两条连接，打开对话框，悬停「本机」→ 齿轮 →「删除配置」→ 确认删除。
  /// - 判断：假接口收到 `saveConfig:实验室`（新的 default 回退到剩下那条）；
  ///   配置里不再有「本机」；对话框提示已删除。
  testWidgets('连接方式悬停齿轮可以删除配置', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[
        _profile('本机'),
        _profile('实验室', kind: 'tcp'),
      ],
      lastUsedName: '本机',
    );
    final ConnectionController connection = ConnectionController(api);
    await connection.initialize();

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (BuildContext context) => TextButton(
            onPressed: () => showConnectionDialog(context, connection),
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    final TestGesture gesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await gesture.addPointer(location: Offset.zero);
    addTearDown(gesture.removePointer);
    await gesture.moveTo(tester.getCenter(find.text('本机  ·  local-launch')));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('连接方式操作').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('删除配置'));
    await tester.pumpAndSettle();
    expect(find.text('删除连接方式'), findsOneWidget);

    await tester.tap(find.widgetWithText(DswPrimaryButton, '删除'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('saveConfig:实验室'));
    expect(
      connection.profiles.map((ConnectionView item) => item.name),
      isNot(contains('本机')),
    );
    expect(find.text('已删除「本机」'), findsOneWidget);
  });

  /// 测试本机连接表单提供「选择文件…」与「选择目录…」入口。
  ///
  /// - 手段：打开配置不存在时的连接对话框（默认 local-launch）。
  /// - 判断：kb-core 一个文件选择按钮，运行时 / 存储两个目录选择按钮都在。
  testWidgets('连接表单提供文件与目录选择按钮', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(configExists: false);
    final ConnectionController connection = ConnectionController(api);

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (BuildContext context) => TextButton(
            onPressed: () => showConnectionDialog(context, connection),
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    expect(find.byTooltip('选择文件…'), findsOneWidget);
    expect(find.byTooltip('选择目录…'), findsNWidgets(2));
  });

  /// 测试连上之后，服务端的工作区与会话出现在侧边栏里。
  ///
  /// - 手段：用假接口连上，把 `KbAdminApp` 渲染出来（宽屏），等会话拉完。
  /// - 判断：状态条显示连接方式与服务端版本；两个工作区名都在；
  ///   第一个工作区展开着，所以它的会话标题也在。
  testWidgets('连上之后侧边栏显示服务端的工作区与会话', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final AppController app = await _appController();
    await tester.pumpWidget(
      KbAdminApp(controller: app, connection: connection),
    );
    await tester.pumpAndSettle();

    // 状态条：连接方式 + 服务端版本。
    expect(find.textContaining('kb_core 9.9.9'), findsOneWidget);
    // 服务端的工作区。名字在侧边栏与面包屑里各出现一次，所以限定在侧边栏里找。
    final Finder sidebar = find.byType(ServerWorkspaceList);
    expect(
      find.descendant(of: sidebar, matching: find.text('笔记')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: sidebar, matching: find.text('资料')),
      findsOneWidget,
    );
    // 展开着的第一个工作区，会话已经拉下来了。
    expect(
      find.descendant(of: sidebar, matching: find.text('会话@w-1')),
      findsOneWidget,
    );
    expect(sidebar, findsOneWidget);
  });

  /// 测试新建工作区：提交给服务端、刷新列表、并把新工作区选中。
  ///
  /// - 手段：连上假接口后调 `addWorkspace`，名字与目录都带过去。
  /// - 判断：返回空串；假接口收到 `addWorkspace:新库`；列表里出现「新库」且
  ///   路径原样；`selectedWorkspaceId` 指向服务端分配的那个标识——用户接着
  ///   建会话时不会建到别的工作区里。
  test('新建工作区后刷新列表并选中它', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final String error = await connection.addWorkspace(
      name: '新库',
      path: '/srv/new',
    );

    expect(error, isEmpty);
    expect(api.calls, contains('addWorkspace:新库'));
    expect(
      connection.workspaces.map((WorkspaceView item) => item.name),
      contains('新库'),
    );
    final WorkspaceView created = connection.workspaces.firstWhere(
      (WorkspaceView item) => item.name == '新库',
    );
    expect(created.path, '/srv/new');
    expect(connection.selectedWorkspaceId, created.id);
  });

  /// 测试删除工作区：服务端删掉之后，列表刷新并把它去掉。
  ///
  /// - 手段：连上之后删掉预置的 `w-1`。
  /// - 判断：假接口收到 `removeWorkspace:w-1`；列表里不再有「笔记」，
  ///   剩下的「资料」还在——删除是精确的，没有误伤。
  test('删除工作区后列表不再有它', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final String error = await connection.removeWorkspace('w-1');

    expect(error, isEmpty);
    expect(api.calls, contains('removeWorkspace:w-1'));
    expect(
      connection.workspaces.map((WorkspaceView item) => item.name),
      <String>['资料'],
    );
  });

  /// 测试草稿会话：点「新会话」只改客户端状态，首次提问才真的建会话。
  ///
  /// - 手段：连上之后 `startDraftSession('w-1')`，确认这时**没有任何请求**；
  ///   然后 `ask('你好世界')`。
  /// - 判断：草稿期间 `createSession` 没被调用、也没有新会话；提问之后假接口先收到
  ///   `createSession:w-1` 再收到 `ask:`，会话名字由假服务端从问题推导，
  ///   正文两回合（问题 + 逆序回答）——`turn_id` 去重生效，问题只记了一次。
  test('草稿会话首次提问才创建并落盘', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final String draftError = await connection.startDraftSession('w-1');
    expect(draftError, isEmpty);
    expect(connection.draftingSession, isTrue);
    expect(connection.hasDraftSession('w-1'), isTrue);
    expect(connection.isDraftSelected('w-1'), isTrue);
    expect(connection.selectedSessionId, isNull);
    expect(
      api.calls.where((String call) => call.startsWith('createSession')),
      isEmpty,
      reason: '草稿阶段不应当请求服务端',
    );

    final String error = await connection.ask('你好世界');

    expect(error, isEmpty);
    expect(connection.draftingSession, isFalse);
    expect(connection.hasDraftSession('w-1'), isFalse);
    expect(
      api.calls.indexOf('createSession:w-1'),
      lessThan(api.calls.indexWhere((String call) => call.startsWith('ask:'))),
    );

    final String? sessionId = connection.selectedSessionId;
    expect(sessionId, isNotNull);
    expect(connection.selectedServerSession?.title, '你好世界');
    final SessionDetailReport? detail = connection.sessionDetailOf(sessionId!);
    expect(detail, isNotNull);
    expect(detail!.turns.length, 2);
    expect(detail.turns[0].text, '你好世界');
    expect(detail.turns[1].text, '界世好你');
  });

  /// 测试放弃草稿会话：纯客户端操作，不碰服务端。
  ///
  /// - 手段：开一个草稿后调 `discardDraftSession`。
  /// - 判断：草稿从集合里消失、选中态也清掉；假接口没有收到任何删除 / 创建请求
  ///   ——`kb_core` 从来不知道它存在过。
  test('放弃草稿会话不发请求', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    await connection.startDraftSession('w-1');
    connection.discardDraftSession('w-1');

    expect(connection.hasDraftSession('w-1'), isFalse);
    expect(connection.draftingSession, isFalse);
    expect(connection.isDraftSelected('w-1'), isFalse);
    expect(
      api.calls.where(
        (String call) =>
            call.startsWith('createSession') ||
            call.startsWith('removeSession'),
      ),
      isEmpty,
    );
  });

  /// 测试工作区改名：提交给服务端并刷新列表。
  ///
  /// - 手段：连上之后对 `w-1` 调 `renameWorkspace('新名字')`。
  /// - 判断：返回空串；假接口收到 `renameWorkspace:w-1`；列表里那条工作区的名字
  ///   变成新的、标识不变。
  test('工作区改名后列表更新', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final String error = await connection.renameWorkspace('w-1', '新名字');

    expect(error, isEmpty);
    expect(api.calls, contains('renameWorkspace:w-1'));
    final WorkspaceView renamed = connection.workspaces.firstWhere(
      (WorkspaceView item) => item.id == 'w-1',
    );
    expect(renamed.name, '新名字');
  });

  /// 测试会话改名：提交给服务端并刷新该工作区的会话列表。
  ///
  /// - 手段：连上之后对 `s-w-1` 调 `renameSession('我的标题')`。
  /// - 判断：返回空串；假接口收到 `renameSession:s-w-1`；列表里那条会话的标题
  ///   变成新的；空白标题则回落到从首条用户消息推导。
  test('会话改名后列表更新且空白标题回落', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final String error = await connection.renameSession('w-1', 's-w-1', '我的标题');
    expect(error, isEmpty);
    expect(api.calls, contains('renameSession:s-w-1'));
    expect(
      connection
          .sessionsOf('w-1')
          .firstWhere((SessionView item) => item.id == 's-w-1')
          .title,
      '我的标题',
    );

    // 先问一句，再清空标题：服务端会从首条用户消息重新取名。
    await connection.selectSession('w-1', 's-w-1');
    await connection.ask('你好世界');
    await connection.renameSession('w-1', 's-w-1', '   ');
    expect(connection.selectedServerSession?.title, '你好世界');
  });

  /// 测试删除会话：删掉之后刷新该工作区的列表，别的会话不受影响。
  ///
  /// - 手段：连上之后先拉 `w-1`、`w-2` 的会话，再删掉 `w-1` 唯一的那条。
  /// - 判断：假接口收到 `removeSession:w-1:s-w-1`；`w-1` 的列表变空，
  ///   而 `w-2` 仍然有它自己那条。
  test('删除会话后它从列表消失', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);
    await connection.loadSessions('w-1');
    await connection.loadSessions('w-2');

    final String error = await connection.removeSession('w-1', 's-w-1');

    expect(error, isEmpty);
    expect(api.calls, contains('removeSession:w-1:s-w-1'));
    expect(connection.sessionsOf('w-1'), isEmpty);
    expect(connection.sessionsOf('w-2').length, 1);
  });

  /// 测试「新建工作区」对话框：填完名字与目录后提交给服务端，列表出现新工作区。
  ///
  /// - 手段：用假接口连上，渲染 `KbAdminApp`；点工具提示为「新建工作区」的按钮，
  ///   在**对话框内**的两个输入框里填名字与目录，点「创建」。
  /// - 判断：假接口收到 `addWorkspace:服务端新库`；界面上出现这个名字。
  ///   （输入框要限定在对话框里找：对话区的输入框也是一个 `TextField`。）
  testWidgets('新建工作区对话框会提交给服务端', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final AppController app = await _appController();
    await tester.pumpWidget(
      KbAdminApp(controller: app, connection: connection),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('新建工作区'));
    await tester.pumpAndSettle();

    final Finder fields = find.descendant(
      of: find.byType(Dialog),
      matching: find.byType(TextField),
    );
    expect(fields, findsNWidgets(2));
    await tester.enterText(fields.at(0), '服务端新库');
    await tester.enterText(fields.at(1), '/srv/notes');
    await tester.tap(find.text('创建'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('addWorkspace:服务端新库'));
    expect(find.text('服务端新库'), findsWidgets);
  });

  /// 测试选中会话会拉正文，并且同一个会话不会重复拉。
  ///
  /// - 手段：连上假接口后对 `s-w-1` 连续调两次 `selectSession`。
  /// - 判断：选中态与摘要都正确；假接口里 `getSession:s-w-1` **只被调用一次**
  ///   （第二次命中正文缓存）。
  test('选中会话时按需拉正文且不重复拉', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    await connection.selectSession('w-1', 's-w-1');
    await connection.selectSession('w-1', 's-w-1');

    expect(connection.selectedSessionId, 's-w-1');
    expect(connection.selectedServerSession?.title, '会话@w-1');
    expect(connection.sessionDetailOf('s-w-1'), isNotNull);
    expect(
      api.calls.where((String call) => call == 'getSession:s-w-1').length,
      1,
    );
  });

  /// 测试提问：正文里出现问题与逆序回答，会话列表的条数也刷新。
  ///
  /// - 手段：选中 `s-w-1` 后调 `ask('你好世界')`。
  /// - 判断：返回空串；假接口收到 `ask:s-w-1`；正文两回合——用户是原问题、
  ///  助手是它的逆序；列表里那条摘要的 `turnCount` 变成 2。
  test('提问后正文与列表都更新', () async {
    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);
    await connection.selectSession('w-1', 's-w-1');

    final String error = await connection.ask('你好世界');

    expect(error, isEmpty);
    expect(api.calls, contains('ask:s-w-1'));
    final SessionDetailReport? detail = connection.sessionDetailOf('s-w-1');
    expect(detail, isNotNull);
    expect(detail!.turns.length, 2);
    expect(detail.turns[0].role, 'user');
    expect(detail.turns[0].text, '你好世界');
    expect(detail.turns[1].text, '界世好你');
    expect(connection.selectedServerSession?.turnCount, 2);
  });

  /// 测试左上角的主机按钮：显示当前连接的花名，并能切换到另一条连接。
  ///
  /// - 手段：配好两条连接（本机 / 实验室），自动连上「本机」；点主机按钮打开
  ///   菜单，选「实验室」。
  /// - 判断：初始时花名与服务端版本都在界面上；切换后假接口收到
  ///   `connect:实验室`，控制器的 `profileName` 也变成「实验室」。
  testWidgets('左上角主机按钮显示当前花名并可切换连接', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(
      profiles: <ConnectionView>[_profile('本机'), _profile('实验室')],
    );
    final ConnectionController connection = await _connected(api);
    expect(connection.profileName, '本机');

    final AppController app = await _appController();
    await tester.pumpWidget(
      KbAdminApp(controller: app, connection: connection),
    );
    await tester.pumpAndSettle();

    expect(find.text('本机'), findsOneWidget);
    expect(find.textContaining('kb_core 9.9.9'), findsOneWidget);

    await tester.tap(find.text('本机'));
    await tester.pumpAndSettle();
    expect(find.text('实验室'), findsOneWidget);
    await tester.tap(find.text('实验室'));
    await tester.pumpAndSettle();

    expect(api.calls, contains('connect:实验室'));
    expect(connection.profileName, '实验室');
  });

  /// 测试"选中会话 → 提问"这条界面链路：服务端的逆序回答出现在对话区。
  ///
  /// - 手段：连上假接口、渲染应用；点侧边栏里那条会话，在输入框里输入「你好」
  ///   并点发送按钮。
  /// - 判断：假接口收到 `getSession:s-w-1` 与 `ask:s-w-1`；对话区同时出现用户
  ///   气泡「你好」与助手的逆序回答「好你」。
  testWidgets('提问之后对话区显示服务端的逆序回答', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final AppController app = await _appController();
    await tester.pumpWidget(
      KbAdminApp(controller: app, connection: connection),
    );
    await tester.pumpAndSettle();

    // 选中服务端的会话（第一个工作区默认展开，会话已经拉下来了）。
    await tester.tap(find.text('会话@w-1'));
    await tester.pumpAndSettle();
    expect(api.calls, contains('getSession:s-w-1'));

    // 对话区的输入框是全应用唯一的那个 TextField（连接对话框没打开）。
    await tester.enterText(find.byType(TextField), '你好');
    await tester.pumpAndSettle();
    await tester.tap(find.byType(SendArrowIcon));
    await tester.pumpAndSettle();

    expect(api.calls, contains('ask:s-w-1'));
    expect(find.text('你好'), findsWidgets);
    expect(find.text('好你'), findsOneWidget);
  });

  /// 测试界面上的草稿会话：点「新会话」→ 侧边栏立刻出现「新会话」行 → 输入第一个
  /// 问题后才在 `kb_core` 上建会话。
  ///
  /// - 手段：连上假接口、渲染应用；点侧边栏顶部的「新会话」按钮，直接在输入框里
  ///   输入问题并发送。
  /// - 判断：点完按钮后侧边栏多出一条「未同步」的「新会话」行（而且此时还没有任何
  ///   `createSession` 请求）；发送之后那条草稿行消失、换成以问题为标题的真会话，
  ///   对话区显示问题与逆序回答。
  testWidgets('点「新会话」后侧边栏出现草稿行，首个问题才创建会话', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    final _FakeApi api = _FakeApi(profiles: <ConnectionView>[_profile('本机')]);
    final ConnectionController connection = await _connected(api);

    final AppController app = await _appController();
    await tester.pumpWidget(
      KbAdminApp(controller: app, connection: connection),
    );
    await tester.pumpAndSettle();

    // 展开态与折叠轨道各有一个「新会话」按钮（都在树里，靠不透明度切换），
    // 这里点展开态的那个。
    await tester.tap(find.byTooltip('新会话').first);
    await tester.pumpAndSettle();
    expect(connection.draftingSession, isTrue);

    // 侧边栏立刻能看到这条草稿：标题「新会话」、状态「未同步」。
    final Finder sidebar = find.byType(ServerWorkspaceList);
    expect(
      find.descendant(of: sidebar, matching: find.text('新会话')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: sidebar, matching: find.text('未同步')),
      findsOneWidget,
    );
    expect(
      api.calls.where((String call) => call.startsWith('createSession')),
      isEmpty,
      reason: '草稿阶段不应当请求服务端',
    );

    await tester.enterText(find.byType(TextField), '你好世界');
    await tester.pumpAndSettle();
    await tester.tap(find.byType(SendArrowIcon));
    await tester.pumpAndSettle();

    expect(api.calls, contains('createSession:w-1'));
    expect(connection.draftingSession, isFalse);
    expect(connection.selectedServerSession?.title, '你好世界');
    expect(
      find.descendant(of: sidebar, matching: find.text('新会话')),
      findsNothing,
      reason: '草稿已经变成真会话',
    );
    expect(
      find.descendant(of: sidebar, matching: find.text('你好世界')),
      findsOneWidget,
    );
    expect(find.text('界世好你'), findsOneWidget);
  });
}
