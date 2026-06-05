import 'dart:async';
import 'dart:developer' as developer;

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

  windowManager.waitUntilReadyToShow(
    const WindowOptions(
      size: Size(800, 600),
      center: true,
      backgroundColor: Colors.transparent,
      skipTaskbar: false,
      title: 'Nexus Open',
    ),
    () async {
      await windowManager.show();
      await windowManager.focus();
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

class _OpenNextAppState extends State<OpenNextApp> {
  StreamSubscription? _wsSub;
  SettingsState? _settings;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _settings = context.read<SettingsState>();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen(_onWsEvent);
  }

  void _onWsEvent(WsEvent event) {
    if (event is WsWindowStateEvent) {
      if (event.state == 'shown') {
        windowManager.show();
        windowManager.focus();
      } else if (event.state == 'hidden') {
        windowManager.hide();
      }
    }
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    return MaterialApp(
      title: 'Open Next',
      theme: AppTheme.lightTheme,
      darkTheme: AppTheme.darkTheme,
      themeMode: settings.themeMode,
      home: settings.isFirstRun
          ? const OnboardingOverlay()
          : const SettingsPage(),
    );
  }
}
