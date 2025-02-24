import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'src/models/settings_state.dart';
import 'src/theme/app_theme.dart';
import 'src/widgets/settings/settings_page.dart';

void main() {
  runApp(
    ChangeNotifierProvider(
      create: (_) => SettingsState(),
      child: const OpenNextApp(),
    ),
  );
}

class OpenNextApp extends StatelessWidget {
  const OpenNextApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Open Next',
      theme: AppTheme.theme,
      home: const SettingsPage(),
    );
  }
}
