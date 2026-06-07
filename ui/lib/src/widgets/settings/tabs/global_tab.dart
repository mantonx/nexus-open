// Global tab — display defaults, background images, and location.
// These write directly to POST /api/config — no draft needed.

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../models/settings_state.dart';
import '../../../theme/app_tokens.dart';
import 'display_tab.dart';
import 'images_tab.dart';
import 'location_tab.dart';

class GlobalTab extends StatefulWidget {
  const GlobalTab({super.key});

  @override
  State<GlobalTab> createState() => _GlobalTabState();
}

class _GlobalTabState extends State<GlobalTab> with SingleTickerProviderStateMixin {
  late final TabController _tabs;

  static const _labels = ['Display', 'Images', 'Location'];

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: _labels.length, vsync: this);
  }

  @override
  void dispose() {
    _tabs.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Column(
      children: [
        Container(
          decoration: BoxDecoration(
            border: Border(bottom: BorderSide(color: cs.outline, width: 1)),
          ),
          child: TabBar(
            controller: _tabs,
            labelColor: AppColors.accent,
            unselectedLabelColor: cs.onSurfaceVariant,
            indicatorColor: AppColors.accent,
            tabs: _labels.map((l) => Tab(text: l)).toList(),
          ),
        ),
        Expanded(
          child: TabBarView(
            controller: _tabs,
            children: [
              const DisplayTab(),
              const ImagesTab(),
              Builder(builder: (ctx) {
                final settings = ctx.watch<SettingsState>();
                return LocationTab(
                  onLocationSelected: (loc) => settings.updateConfig(location: loc),
                  initialLocation: settings.location,
                );
              }),
            ],
          ),
        ),
      ],
    );
  }
}
