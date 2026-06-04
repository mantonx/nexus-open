import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'src/models/settings_state.dart';
import 'src/services/ws_service.dart';
import 'src/theme/app_theme.dart';
import 'src/widgets/onboarding/onboarding_overlay.dart';
import 'src/widgets/settings/settings_page.dart';

// Set --dart-define=FORCE_ONBOARDING=true to show onboarding unconditionally.
// Used by the screenshot tour to capture onboarding screens without resetting
// the backend's firstRun flag.
const bool _forceOnboarding =
    bool.fromEnvironment('FORCE_ONBOARDING', defaultValue: false);

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Initialize window manager
  await windowManager.ensureInitialized();

  // Set initial window options
  WindowOptions windowOptions = const WindowOptions(
    size: Size(800, 600),
    center: true,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    title: 'Nexus Open',
  );

  windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  runApp(
    MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => SettingsState()),
        ChangeNotifierProvider(create: (_) => WsService()),
      ],
      child: const OpenNextApp(),
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

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
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
