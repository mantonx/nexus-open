// Global tab — display defaults and background images.
// These write directly to POST /api/config — no draft needed.

import 'package:flutter/material.dart';
import '../../../theme/app_tokens.dart';
import 'display_tab.dart';
import 'images_tab.dart';

class GlobalTab extends StatefulWidget {
  const GlobalTab({super.key});

  @override
  State<GlobalTab> createState() => _GlobalTabState();
}

class _GlobalTabState extends State<GlobalTab> with SingleTickerProviderStateMixin {
  late final TabController _tabs;

  static const _labels = ['Display', 'Images'];

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
            children: const [
              DisplayTab(),
              ImagesTab(),
            ],
          ),
        ),
      ],
    );
  }
}
