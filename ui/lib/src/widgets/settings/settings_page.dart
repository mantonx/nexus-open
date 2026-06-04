import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../models/api_models.dart';
import '../../services/nexus_api_service.dart';
import '../../services/ws_service.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';
import 'tabs/location_tab.dart';
import 'tabs/display_tab.dart';
import 'tabs/preview_tab.dart';
import 'tabs/images_tab.dart';
import 'tabs/modules_tab.dart';

class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  int _selectedIndex = 0;
  StreamSubscription? _wsSub;
  String? _deviceModel;
  String? _deviceFirmware;
  NexusConfig? _savedConfig;

  // Navigation destinations — icon only, labels shown as tooltips
  static const _destinations = [
    (icon: Icons.display_settings_outlined, selected: Icons.display_settings, label: 'Display', tooltip: 'Display & Colors'),
    (icon: Icons.location_on_outlined,      selected: Icons.location_on,       label: 'Location', tooltip: 'Location'),
    (icon: Icons.tune_outlined,             selected: Icons.tune,              label: 'Modules',  tooltip: 'Modules'),
    (icon: Icons.image_outlined,            selected: Icons.image,             label: 'Images',   tooltip: 'Images'),
  ];

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await context.read<SettingsState>().loadFromBackend();
      if (mounted) {
        setState(() => _savedConfig = context.read<SettingsState>().config);
        _fetchDeviceInfo();
      }
    });
  }

  Future<void> _fetchDeviceInfo() async {
    try {
      final api = NexusApiService();
      final info = await api.getDeviceInfo();
      api.dispose();
      if (mounted) setState(() {
        _deviceModel = info.model;
        _deviceFirmware = info.firmware;
      });
    } catch (_) {}
  }

  bool get _hasUnsavedChanges {
    final current = context.read<SettingsState>().config;
    if (_savedConfig == null || current == null) return false;
    return current.toJson().toString() != _savedConfig!.toJson().toString();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen((event) {
      if (event is WsDisconnectedEvent && mounted) {
        context.read<SettingsState>().setConnected(false);
      } else if (event is WsConnectedEvent && mounted) {
        context.read<SettingsState>().loadFromBackend();
      }
    });
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  Future<void> _handleSave() async {
    final s = context.read<SettingsState>();
    await s.saveToBackend();
    if (!mounted) return;
    final ok = s.errorMessage == null;
    if (ok) setState(() => _savedConfig = s.config);
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
      content: Text(ok
          ? 'Settings saved'
          : 'Save failed: ${s.errorMessage}'),
    ));
  }

  Future<bool> _confirmDiscard() async =>
      await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Unsaved changes'),
          content: const Text('Discard changes and close?'),
          actions: [
            NexusButton.ghost(
                label: 'Keep editing',
                onPressed: () => Navigator.pop(ctx, false)),
            NexusButton.destructive(
                label: 'Discard',
                onPressed: () => Navigator.pop(ctx, true)),
          ],
        ),
      ) ?? false;

  Widget _buildPage(SettingsState s) {
    switch (_selectedIndex) {
      case 0: return const PreviewTab();
      case 1: return LocationTab(
        onLocationSelected: (loc) => s.updateConfig(location: loc),
        initialLocation: s.location,
      );
      case 2: return const ModulesTab();
      case 3: return const ImagesTab();
      default: return const SizedBox.shrink();
    }
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final ws = context.watch<WsService>();
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return PopScope(
      canPop: !_hasUnsavedChanges,
      onPopInvokedWithResult: (didPop, _) async {
        if (!didPop && _hasUnsavedChanges) {
          if (await _confirmDiscard() && mounted) Navigator.of(context).pop();
        }
      },
      child: Scaffold(
        body: Column(
          children: [
            // ── Connection loss banner ──────────────────────────────────────
            if (!ws.isConnected)
              Container(
                decoration: BoxDecoration(
                  color: AppColors.darkElevated,
                  border: Border(
                    left:   BorderSide(color: cs.warning, width: 3),
                    bottom: BorderSide(color: AppColors.darkBorder, width: 1),
                  ),
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.md,
                  vertical: AppSpacing.xs + 2,
                ),
                child: Row(
                  children: [
                    Icon(Icons.cloud_off_outlined,
                        size: AppIconSize.sm, color: cs.warning),
                    const SizedBox(width: AppSpacing.sm),
                    Expanded(
                      child: Text(
                        'Backend disconnected — changes cannot be saved',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                    NexusButton.ghost(
                      label: 'Retry',
                      onPressed: () => settings.loadFromBackend(),
                    ),
                  ],
                ),
              ),

            Expanded(
              child: Row(
                children: [
                  // ── NavigationRail ──────────────────────────────────────
                  _NexusRail(
                    selectedIndex: _selectedIndex,
                    onDestinationSelected: (i) =>
                        setState(() => _selectedIndex = i),
                    destinations: _destinations,
                    deviceModel: _deviceModel,
                    deviceFirmware: _deviceFirmware,
                    isConnected: ws.isConnected,
                    themeMode: settings.themeMode,
                    isLoading: settings.isLoading,
                    canSave: !settings.isLoading && ws.isConnected,
                    onSave: _handleSave,
                    onToggleTheme: () => settings.setThemeMode(
                      settings.themeMode == ThemeMode.dark
                          ? ThemeMode.light
                          : ThemeMode.dark,
                    ),
                  ),

                  // ── Content area ────────────────────────────────────────
                  Expanded(
                    child: Stack(
                      children: [
                        Positioned.fill(
                          child: CustomPaint(painter: _DotGridPainter()),
                        ),
                        AnimatedSwitcher(
                          duration: AppDuration.normal,
                          child: KeyedSubtree(
                            key: ValueKey(_selectedIndex),
                            child: _buildPage(settings),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
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
    required this.deviceModel,
    required this.deviceFirmware,
    required this.isConnected,
    required this.themeMode,
    required this.isLoading,
    required this.canSave,
    required this.onSave,
    required this.onToggleTheme,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<({IconData icon, IconData selected, String label, String tooltip})>
      destinations;
  final String? deviceModel;
  final String? deviceFirmware;
  final bool isConnected;
  final ThemeMode themeMode;
  final bool isLoading;
  final bool canSave;
  final VoidCallback onSave;
  final VoidCallback onToggleTheme;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final theme = Theme.of(context);

    return Container(
      width: 72,
      decoration: BoxDecoration(
        color: cs.railBackground,
        border: Border(
          right: BorderSide(color: AppColors.darkBorder, width: 1),
        ),
      ),
      child: Column(
        children: [
          // ── App header ─────────────────────────────────────────────────
          Padding(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.sm, AppSpacing.md, AppSpacing.sm, AppSpacing.sm),
            child: Column(
              children: [
                // App wordmark
                Text(
                  'NEXUS',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: AppColors.accent,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 3,
                    shadows: [
                      Shadow(
                        color: AppColors.accent.withOpacity(0.7),
                        blurRadius: 10,
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: AppSpacing.xs),
                // ── 640×48 live display strip ─────────────────────────
                _DisplayStrip(),
                const SizedBox(height: AppSpacing.xs),
                // Connection status
                Semantics(
                  label: isConnected
                      ? (deviceModel != null
                          ? 'Connected: $deviceModel'
                          : 'Connected')
                      : 'Disconnected',
                  child: NexusStatusBadge.dot(
                    status: isConnected ? NexusStatus.ok : NexusStatus.warning,
                  ),
                ),
              ],
            ),
          ),

          const Divider(height: 1, thickness: 1),
          const SizedBox(height: AppSpacing.xs),

          // ── Destinations ───────────────────────────────────────────────
          ...destinations.asMap().entries.map((e) {
            final i = e.key;
            final d = e.value;
            final isSelected = i == selectedIndex;

            return Padding(
              padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.xs, vertical: 2),
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
                      color: isSelected
                          ? AppColors.accent.withOpacity(0.08)
                          : Colors.transparent,
                      borderRadius: AppRadius.smBr,
                      border: Border(
                        left: BorderSide(
                          color: isSelected
                              ? AppColors.accent
                              : Colors.transparent,
                          width: 2,
                        ),
                      ),
                    ),
                    child: Column(
                      children: [
                        Icon(
                          isSelected ? d.selected : d.icon,
                          size: AppIconSize.md,
                          color: isSelected
                              ? AppColors.accent
                              : Colors.white.withOpacity(0.45),
                        ),
                        const SizedBox(height: 3),
                        Text(
                          d.label,
                          style: theme.textTheme.labelSmall?.copyWith(
                            fontSize: 9,
                            color: isSelected
                                ? AppColors.accent
                                : Colors.white.withOpacity(0.45),
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

          // ── Footer actions ─────────────────────────────────────────────
          Padding(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
            child: Column(
              children: [
                // Theme toggle
                Semantics(
                  label: themeMode == ThemeMode.dark
                      ? 'Switch to light mode'
                      : 'Switch to dark mode',
                  button: true,
                  child: Tooltip(
                    message: themeMode == ThemeMode.dark
                        ? 'Light mode'
                        : 'Dark mode',
                    child: IconButton(
                      icon: Icon(
                        themeMode == ThemeMode.dark
                            ? Icons.light_mode_outlined
                            : Icons.dark_mode_outlined,
                        size: AppIconSize.md,
                        color: Colors.white.withOpacity(0.55),
                      ),
                      onPressed: onToggleTheme,
                    ),
                  ),
                ),
                // Save
                Semantics(
                  label: isLoading ? 'Saving…' : 'Save settings',
                  button: true,
                  enabled: canSave,
                  child: Tooltip(
                    message: canSave ? 'Save settings' : 'Not connected',
                    child: IconButton(
                      icon: isLoading
                          ? SizedBox(
                              width: AppIconSize.md,
                              height: AppIconSize.md,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: AppColors.accent,
                              ),
                            )
                          : Icon(
                              Icons.save_outlined,
                              size: AppIconSize.md,
                              color: canSave
                                  ? AppColors.accent
                                  : Colors.white.withOpacity(0.25),
                            ),
                      onPressed: canSave ? onSave : null,
                    ),
                  ),
                ),
              ],
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
      ..color = Colors.white.withOpacity(0.032)
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

// ── Persistent 640×48 display strip ──────────────────────────────────────────
// Sits in the rail header, visible on every section.

class _DisplayStrip extends StatefulWidget {
  const _DisplayStrip();

  @override
  State<_DisplayStrip> createState() => _DisplayStripState();
}

class _DisplayStripState extends State<_DisplayStrip> {
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
    final cs = Theme.of(context).colorScheme;

    return Container(
      // Scale to fit the 72px rail. Aspect ratio is 640:48 ≈ 13.3:1
      // At 64px wide that gives ~4.8px height — too thin. Use 56px wide, 8px tall.
      width: 56,
      height: 8,
      decoration: BoxDecoration(
        color: AppColors.darkBg,
        borderRadius: AppRadius.xsBr,
        border: Border.all(
          color: _lastFrame != null
              ? AppColors.accent.withOpacity(0.5)
              : cs.outline,
          width: 1,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: _lastFrame != null
          ? Image.memory(
              _lastFrame!,
              fit: BoxFit.fill,
              gaplessPlayback: true,
            )
          : const SizedBox.shrink(),
    );
  }
}
