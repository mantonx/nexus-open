import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'src/models/settings_state.dart';
import 'src/services/window_controller.dart';
import 'src/theme/app_theme.dart';
import 'src/widgets/settings/settings_page.dart';

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
    ChangeNotifierProvider(
      create: (_) => SettingsState(),
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
  late WindowController _windowController;

  @override
  void initState() {
    super.initState();
    _windowController = WindowController();
    _windowController.startListening();
  }

  @override
  void dispose() {
    _windowController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Open Next',
      theme: AppTheme.theme,
      home: const SettingsPage(),
    );
  }
}
