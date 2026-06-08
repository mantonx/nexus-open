import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../services/nexus_api_service.dart';
import '../../services/ws_service.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';
import 'tabs/editor_tab.dart';
import 'tabs/global_tab.dart';
import 'tabs/device_tab.dart';

class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  int _selectedIndex = 0;
  StreamSubscription? _wsSub;
  bool _hasDraftChanges = false;
  bool _draftBusy = false;
  late NexusApiService _api;

  static const _destinations = [
    (icon: Icons.dashboard_customize_outlined, selected: Icons.dashboard_customize, label: 'Editor',  tooltip: 'Layout Editor'),
    (icon: Icons.tune_outlined,               selected: Icons.tune,                label: 'Global',   tooltip: 'Global Settings'),
    (icon: Icons.developer_board_outlined,    selected: Icons.developer_board,     label: 'Device',   tooltip: 'Device & Health'),
  ];

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await context.read<SettingsState>().loadFromBackend();
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen((event) {
      if (!mounted) return;
      if (event is WsDisconnectedEvent) {
        context.read<SettingsState>().setConnected(false);
        ScaffoldMessenger.of(context)
          ..hideCurrentSnackBar()
          ..showSnackBar(const SnackBar(
            content: Text('Device disconnected'),
            duration: Duration(seconds: 3),
          ));
      } else if (event is WsConnectedEvent) {
        context.read<SettingsState>().loadFromBackend();
        ScaffoldMessenger.of(context)
          ..hideCurrentSnackBar()
          ..showSnackBar(const SnackBar(
            content: Text('Device connected'),
            duration: Duration(seconds: 3),
          ));
      } else if (event is WsDraftStateEvent) {
        setState(() => _hasDraftChanges = event.active);
      }
    });
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  Future<void> _commitDraft() async {
    setState(() => _draftBusy = true);
    try {
      await _api.commitDraft();
      if (mounted) setState(() => _hasDraftChanges = false);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Commit failed: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _draftBusy = false);
    }
  }

  Future<void> _discardDraft() async {
    setState(() => _draftBusy = true);
    try {
      await _api.discardDraft();
      if (mounted) setState(() => _hasDraftChanges = false);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Discard failed: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _draftBusy = false);
    }
  }

  Widget _buildPage() {
    switch (_selectedIndex) {
      case 0: return const EditorTab();
      case 1: return const GlobalTab();
      case 2: return const DeviceTab();
      default: return const SizedBox.shrink();
    }
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.watch<WsService>();
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Scaffold(
      body: Column(
        children: [
          // ── Disconnection banner ──────────────────────────────────────────
          if (!ws.isConnected)
            Container(
              decoration: BoxDecoration(
                color: cs.surfaceContainerHigh,
                border: Border(
                  left:   BorderSide(color: cs.warning, width: 3),
                  bottom: BorderSide(color: cs.outline, width: 1),
                ),
              ),
              padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.md,
                vertical: AppSpacing.xs + 2,
              ),
              child: Row(
                children: [
                  Icon(Icons.cloud_off_outlined, size: AppIconSize.sm, color: cs.warning),
                  const SizedBox(width: AppSpacing.sm),
                  Expanded(
                    child: Text(
                      'Backend disconnected — changes cannot be saved',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: cs.onSurfaceVariant,
                      ),
                    ),
                  ),
                  NexusButton.ghost(
                    label: 'Retry',
                    onPressed: () => context.read<SettingsState>().loadFromBackend(),
                  ),
                ],
              ),
            ),

          // ── Preview strip ─────────────────────────────────────────────────
          _PreviewStrip(hasDraft: _hasDraftChanges),

          // ── Main body ────────────────────────────────────────────────────
          Expanded(
            child: Row(
              children: [
                // NavigationRail
                _NexusRail(
                  selectedIndex: _selectedIndex,
                  onDestinationSelected: (i) => setState(() => _selectedIndex = i),
                  destinations: _destinations,
                  isConnected: ws.isConnected,
                  themeMode: context.watch<SettingsState>().themeMode,
                  onToggleTheme: () {
                    final s = context.read<SettingsState>();
                    s.setThemeMode(s.themeMode == ThemeMode.dark
                        ? ThemeMode.light
                        : ThemeMode.dark);
                  },
                ),

                // Content
                Expanded(
                  child: Stack(
                    children: [
                      Positioned.fill(child: CustomPaint(painter: _DotGridPainter())),
                      AnimatedSwitcher(
                        duration: AppDuration.normal,
                        child: KeyedSubtree(
                          key: ValueKey(_selectedIndex),
                          child: _buildPage(),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),

          // ── Draft / confirm bar ──────────────────────────────────────────
          AnimatedSize(
            duration: AppDuration.normal,
            curve: Curves.easeInOut,
            child: _hasDraftChanges
                ? _DraftBar(busy: _draftBusy, onCommit: _commitDraft, onDiscard: _discardDraft)
                : const SizedBox.shrink(),
          ),
        ],
      ),
    );
  }
}

// ── Preview strip ─────────────────────────────────────────────────────────────

class _PreviewStrip extends StatefulWidget {
  const _PreviewStrip({required this.hasDraft});
  final bool hasDraft;

  @override
  State<_PreviewStrip> createState() => _PreviewStripState();
}

class _PreviewStripState extends State<_PreviewStrip> {
  Uint8List? _lastFrame;
  StreamSubscription? _sub;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sub?.cancel();
    _sub = context.read<WsService>().events.listen((event) {
      if (event is WsFrameEvent && mounted) {
        setState(() => _lastFrame = event.pngBytes);
      }
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final borderColor = widget.hasDraft
        ? AppColors.hardwareAccent
        : AppColors.hardwareAccent.withValues(alpha: 0.35);

    return Container(
      height: 48 + 8, // 48px display + 4px padding top/bottom
      padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 12),
      decoration: BoxDecoration(
        color: AppColors.darkBg,
        border: Border(
          bottom: BorderSide(color: borderColor, width: widget.hasDraft ? 2 : 1),
        ),
      ),
      child: Center(
        child: Container(
          width: 640,
          height: 48,
          decoration: BoxDecoration(
            color: Colors.black,
            borderRadius: AppRadius.xsBr,
            border: Border.all(color: borderColor, width: widget.hasDraft ? 2 : 1),
            boxShadow: widget.hasDraft
                ? [BoxShadow(color: AppColors.hardwareAccent.withValues(alpha: 0.25), blurRadius: 10, spreadRadius: 1)]
                : null,
          ),
          clipBehavior: Clip.antiAlias,
          child: _lastFrame != null
              ? Image.memory(_lastFrame!, fit: BoxFit.fill, gaplessPlayback: true)
              : const ColoredBox(color: Colors.black),
        ),
      ),
    );
  }
}

// ── Draft / confirm bar ───────────────────────────────────────────────────────

class _DraftBar extends StatelessWidget {
  const _DraftBar({
    required this.busy,
    required this.onCommit,
    required this.onDiscard,
  });

  final bool busy;
  final VoidCallback onCommit;
  final VoidCallback onDiscard;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.md,
        vertical: AppSpacing.sm,
      ),
      decoration: BoxDecoration(
        color: cs.surfaceContainer,
        border: Border(top: BorderSide(color: AppColors.hardwareAccent.withValues(alpha: 0.4), width: 1)),
      ),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: const BoxDecoration(
              shape: BoxShape.circle,
              color: AppColors.hardwareAccent,
            ),
          ),
          const SizedBox(width: AppSpacing.sm),
          Expanded(
            child: Text(
              'Unsaved changes — live on device',
              style: theme.textTheme.bodySmall?.copyWith(
                color: cs.onSurfaceVariant,
              ),
            ),
          ),
          if (busy)
            const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.accent),
            )
          else ...[
            NexusButton.ghost(label: 'Discard', onPressed: onDiscard),
            const SizedBox(width: AppSpacing.sm),
            NexusButton.primary(label: 'Confirm', onPressed: onCommit),
          ],
        ],
      ),
    );
  }
}

// ── NavigationRail ────────────────────────────────────────────────────────────

class _NexusRail extends StatelessWidget {
  const _NexusRail({
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.destinations,
    required this.isConnected,
    required this.themeMode,
    required this.onToggleTheme,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<({IconData icon, IconData selected, String label, String tooltip})> destinations;
  final bool isConnected;
  final ThemeMode themeMode;
  final VoidCallback onToggleTheme;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final theme = Theme.of(context);

    return Container(
      width: 72,
      decoration: BoxDecoration(
        color: cs.railBackground,
        border: Border(right: BorderSide(color: cs.outline, width: 1)),
      ),
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.sm, AppSpacing.md, AppSpacing.sm, AppSpacing.sm),
            child: Column(
              children: [
                Text(
                  'NEXUS',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: AppColors.hardwareAccent,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 3,
                    shadows: [Shadow(color: AppColors.hardwareAccent.withValues(alpha: 0.7), blurRadius: 10)],
                  ),
                ),
                const SizedBox(height: AppSpacing.sm),
                Semantics(
                  label: isConnected ? 'Connected' : 'Disconnected',
                  child: NexusStatusBadge.dot(
                    status: isConnected ? NexusStatus.ok : NexusStatus.warning,
                  ),
                ),
              ],
            ),
          ),

          const Divider(height: 1, thickness: 1),
          const SizedBox(height: AppSpacing.xs),

          ...destinations.asMap().entries.map((e) {
            final i = e.key;
            final d = e.value;
            final isSelected = i == selectedIndex;

            return Padding(
              padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs, vertical: 2),
              child: Tooltip(
                message: d.tooltip,
                preferBelow: false,
                child: InkWell(
                  onTap: () => onDestinationSelected(i),
                  borderRadius: AppRadius.smBr,
                  child: AnimatedContainer(
                    duration: AppDuration.fast,
                    width: double.infinity,
                    padding: const EdgeInsets.symmetric(vertical: AppSpacing.sm),
                    decoration: BoxDecoration(
                      color: isSelected ? AppColors.accent.withValues(alpha: 0.08) : Colors.transparent,
                      borderRadius: AppRadius.smBr,
                      border: Border(
                        left: BorderSide(
                          color: isSelected ? AppColors.accent : Colors.transparent,
                          width: 2,
                        ),
                      ),
                    ),
                    child: Column(
                      children: [
                        Icon(
                          isSelected ? d.selected : d.icon,
                          size: AppIconSize.md,
                          color: isSelected ? AppColors.accent : Colors.white.withValues(alpha: 0.45),
                        ),
                        const SizedBox(height: 3),
                        Text(
                          d.label,
                          style: theme.textTheme.labelSmall?.copyWith(
                            fontSize: 9,
                            color: isSelected ? AppColors.accent : Colors.white.withValues(alpha: 0.45),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            );
          }),

          const Spacer(),
          const Divider(height: 1, thickness: 1),

          Padding(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
            child: Tooltip(
              message: themeMode == ThemeMode.dark ? 'Light mode' : 'Dark mode',
              child: IconButton(
                icon: Icon(
                  themeMode == ThemeMode.dark ? Icons.light_mode_outlined : Icons.dark_mode_outlined,
                  size: AppIconSize.md,
                  color: Colors.white.withValues(alpha: 0.55),
                ),
                onPressed: onToggleTheme,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Dot-grid background painter ──────────────────────────────────────────────

class _DotGridPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.white.withValues(alpha: 0.045)
      ..strokeWidth = 1;
    const step = 24.0;
    for (double x = step; x < size.width; x += step) {
      for (double y = step; y < size.height; y += step) {
        canvas.drawCircle(Offset(x, y), 1, paint);
      }
    }
  }

  @override
  bool shouldRepaint(_DotGridPainter _) => false;
}
