import 'dart:async';
import 'dart:developer' as developer;
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'src/models/settings_state.dart';
import 'src/services/ws_service.dart';
import 'src/theme/app_theme.dart';
import 'src/widgets/onboarding/onboarding_overlay.dart';
import 'src/widgets/settings/settings_page.dart';

// Global key so VM service extensions can access the settings state
final _appKey = GlobalKey<_OpenNextAppState>();

void main() async {
  WidgetsFlutterBinding.ensureInitialized();


  await windowManager.ensureInitialized();

  final startMinimized = Platform.environment['NEXUS_START_MINIMIZED'] == '1';

  windowManager.waitUntilReadyToShow(
    const WindowOptions(
      size: Size(1400, 800),
      minimumSize: Size(1400, 800),
      center: true,
      backgroundColor: Colors.transparent,
      skipTaskbar: false,
      title: 'Nexus Open',
    ),
    () async {
      if (startMinimized) {
        await windowManager.minimize();
      } else {
        await windowManager.show();
        await windowManager.focus();
      }
    },
  );

  // Register VM service extension for screenshot tour — forces onboarding
  // to show regardless of backend firstRun state. Debug builds only.
  if (kDebugMode) {
    developer.registerExtension('ext.nexus.showOnboarding', (_, __) async {
      _appKey.currentState?._settings?.forceFirstRun();
      return developer.ServiceExtensionResponse.result('{"status":"ok"}');
    });
  }

  runApp(
    MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => SettingsState()),
        ChangeNotifierProvider(create: (_) => WsService()),
      ],
      child: OpenNextApp(key: _appKey),
    ),
  );
}

class OpenNextApp extends StatefulWidget {
  const OpenNextApp({super.key});

  @override
  State<OpenNextApp> createState() => _OpenNextAppState();
}

class _OpenNextAppState extends State<OpenNextApp> with WindowListener {
  StreamSubscription? _wsSub;
  SettingsState? _settings;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    windowManager.setPreventClose(true);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _settings = context.read<SettingsState>();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen(_onWsEvent);
  }

  @override
  void onWindowClose() async {
    try {
      final client = HttpClient();
      final req = await client.postUrl(
          Uri.parse('http://localhost:1985/api/window/closed'));
      req.headers.contentType = ContentType.json;
      await req.close();
      client.close();
    } catch (_) {}
    await windowManager.hide();
  }

  void _onWsEvent(WsEvent event) {
    if (event is WsWindowStateEvent) {
      if (event.state == 'shown') {
        windowManager.restore();
        windowManager.show();
        windowManager.focus();
      } else if (event.state == 'hidden') {
        windowManager.hide();
      }
    }
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    _wsSub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final ws = context.watch<WsService>();
    return MaterialApp(
      title: 'Open Next',
      theme: AppTheme.lightTheme,
      darkTheme: AppTheme.darkTheme,
      themeMode: settings.themeMode,
      home: !ws.isConnected
          ? const _LoadingScreen()
          : settings.isFirstRun
              ? const OnboardingOverlay()
              : const SettingsPage(),
    );
  }
}

class _LoadingScreen extends StatelessWidget {
  const _LoadingScreen();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      backgroundColor: Color(0xFF131316),
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            CircularProgressIndicator(
              color: Color(0xFF4F9EFF),
              strokeWidth: 2.5,
            ),
            SizedBox(height: 20),
            Text(
              'Connecting…',
              style: TextStyle(
                color: Color(0xFF6B6B7A),
                fontSize: 13,
                letterSpacing: 0.3,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
