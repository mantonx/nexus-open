import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../services/nexus_api_service.dart';
import '../../services/ws_service.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';
import '../display/detail_overlay.dart';
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
  ui.Image? _uiImage;
  StreamSubscription? _sub;
  List<WsZoneInfo> _currentZones = [];
  int _numPages = 1;

  final _displayKey = GlobalKey();
  double? _dragStartX;
  double _lastDragX = 0;
  bool _isDragging = false;

  // Index into _currentZones of the zone under the cursor, or -1 if none/not tappable.
  int _hoveredZoneIdx = -1;
  Offset? _cursorPos; // local position within the display widget
  bool _detailActive = false;
  int _closeButtonHardwareX = 630; // hardware pixel center, updated from backend
  int _closeButtonHardwareY = 10;
  bool _ringVisible = false; // drives AnimatedOpacity

  NexusApiService get _api => context.read<NexusApiService>();

  void _applyPageState(WsPageStateEvent event) {
    if (!mounted) return;
    setState(() {
      _numPages = event.numPages;
      _currentZones = event.currentPage < event.pages.length
          ? event.pages[event.currentPage].zones
          : [];
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sub?.cancel();
    _sub = context.read<WsService>().events.listen((event) {
      if (!mounted) return;
      if (event is WsFrameEvent) {
        ui.decodeImageFromList(event.pngBytes, (img) {
          if (!mounted) { img.dispose(); return; }
          final old = _uiImage;
          setState(() => _uiImage = img);
          old?.dispose();
        });
      } else if (event is WsPageStateEvent) {
        _applyPageState(event);
      } else if (event is WsDetailStateEvent) {
        setState(() {
          _detailActive = event.active;
          if (event.active) {
            _closeButtonHardwareX = event.closeX;
            _closeButtonHardwareY = event.closeY;
          } else {
            _hoveredZoneIdx = -1;
            _cursorPos = null;
            _ringVisible = false;
          }
        });
      }
    });
    // Fetch current state immediately — WS stream doesn't replay past events.
    context.read<NexusApiService>().getNavigateState()
        .then(_applyPageState)
        .catchError((_) {});
  }

  @override
  void dispose() {
    _sub?.cancel();
    _uiImage?.dispose();
    super.dispose();
  }

  int _toHardwareX(double localX) {
    final box = _displayKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return 0;
    return (localX / box.size.width * 640).round().clamp(0, 639);
  }

  // Returns the zone index whose x-range contains hardwareX, or -1.
  int _zoneIndexAt(int hardwareX) {
    int offset = 0;
    for (var i = 0; i < _currentZones.length; i++) {
      final z = _currentZones[i];
      if (hardwareX >= offset && hardwareX < offset + z.width) return i;
      offset += z.width;
    }
    return -1;
  }

  // Maps hardware pixel coordinates to local widget coordinates.
  Offset? _closeButtonLocalPos() {
    final box = _displayKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return null;
    return Offset(
      _closeButtonHardwareX / 1280 * box.size.width,
      _closeButtonHardwareY / 96 * box.size.height,
    );
  }

  void _onHover(PointerHoverEvent e) {
    if (_detailActive) {
      // In detail mode: show ring near close button, not at cursor.
      final closePos = _closeButtonLocalPos();
      setState(() {
        _hoveredZoneIdx = -1;
        _cursorPos = closePos;
        _ringVisible = closePos != null;
      });
      return;
    }
    final hx = _toHardwareX(e.localPosition.dx);
    final idx = _zoneIndexAt(hx);
    final next = (idx != -1 && _currentZones[idx].isTappable) ? idx : -1;
    setState(() {
      _hoveredZoneIdx = next;
      _cursorPos = next != -1 ? e.localPosition : null;
      _ringVisible = next != -1;
    });
  }

  void _onExit(PointerExitEvent _) {
    if (_hoveredZoneIdx != -1 || _cursorPos != null || _ringVisible) {
      setState(() { _hoveredZoneIdx = -1; _cursorPos = null; _ringVisible = false; });
    }
  }

  void _onPanStart(DragStartDetails d) {
    _dragStartX = d.localPosition.dx;
    _isDragging = false;
  }

  void _onPanUpdate(DragUpdateDetails d) {
    final startX = _dragStartX;
    if (startX == null) return;
    final dx = d.localPosition.dx - startX;
    final box = _displayKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return;
    final progress = (dx.abs() / box.size.width).clamp(0.0, 1.0);
    if (progress > 0.01) _isDragging = true;
    if (_isDragging) {
      _lastDragX = d.localPosition.dx;
      _api.swipeUpdate(progress, isLeft: dx < 0).catchError((_) {});
    }
  }

  void _onPanEnd(DragEndDetails d) {
    if (!_isDragging) {
      _dragStartX = null;
      return;
    }
    final startX = _dragStartX;
    _dragStartX = null;
    _isDragging = false;
    if (startX == null) return;
    final box = _displayKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) { _api.swipeCancel().catchError((_) {}); return; }
    final velocity = d.velocity.pixelsPerSecond.dx;
    final progress = ((_lastDragX - startX).abs() / box.size.width).clamp(0.0, 1.0);
    _api.swipeFinalize(progress, velocity.abs(), isLeft: velocity < 0).catchError((_) {});
  }

  void _onTapUp(TapUpDetails d) {
    if (_isDragging) return;
    _api.tapZone(_toHardwareX(d.localPosition.dx)).catchError((_) {});
  }

  @override
  Widget build(BuildContext context) {
    final borderColor = widget.hasDraft
        ? AppColors.hardwareAccent
        : AppColors.hardwareAccent.withValues(alpha: 0.35);
    final canSwipeLeft  = _numPages > 1;
    final canSwipeRight = _numPages > 1;

    // Reserve 32px for the two arrows (20px icon + 6px gap each side).
    const arrowReserve = 32.0;

    return Container(
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 12),
      decoration: BoxDecoration(
        color: AppColors.darkBg,
        border: Border(
          bottom: BorderSide(color: borderColor, width: widget.hasDraft ? 2 : 1),
        ),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          // Fill available width; clamp so it never shrinks below a usable size.
          final displayW = (constraints.maxWidth - arrowReserve).clamp(200.0, 960.0);
          final displayH = (displayW / 640 * 48).roundToDouble();

          return Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              // ── Left swipe arrow ───────────────────────────────────────────
              _SwipeArrow(
                direction: _SwipeDirection.left,
                visible: canSwipeLeft,
                onTap: () => _api.simulateSwipe(direction: 'right').catchError((_) {}),
              ),
              const SizedBox(width: 6),

              // ── Display ───────────────────────────────────────────────────
              MouseRegion(
                cursor: _hoveredZoneIdx != -1
                    ? SystemMouseCursors.click
                    : SystemMouseCursors.basic,
                onExit: _onExit,
                child: Listener(
                  onPointerHover: _onHover,
                  child: GestureDetector(
                    key: _displayKey,
                    onPanStart: _onPanStart,
                    onPanUpdate: _onPanUpdate,
                    onPanEnd: _onPanEnd,
                    onTapUp: _onTapUp,
                    child: Container(
                      width: displayW,
                      height: displayH,
                      decoration: BoxDecoration(
                        color: Colors.black,
                        borderRadius: AppRadius.xsBr,
                        border: Border.all(color: borderColor, width: widget.hasDraft ? 2 : 1),
                        boxShadow: widget.hasDraft
                            ? [BoxShadow(color: AppColors.hardwareAccent.withValues(alpha: 0.25), blurRadius: 10, spreadRadius: 1)]
                            : null,
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: DetailOverlay(
                        active: _detailActive,
                        displaySize: Size(displayW, displayH),
                        onDismiss: () => _api.tapZone(_closeButtonHardwareX).catchError((_) {}),
                        child: Stack(
                          fit: StackFit.expand,
                          children: [
                            if (_uiImage != null)
                              RawImage(
                                image: _uiImage,
                                fit: BoxFit.fill,
                                filterQuality: FilterQuality.medium,
                              )
                            else
                              const ColoredBox(color: Colors.black),
                            if (_cursorPos != null)
                              AnimatedOpacity(
                                duration: const Duration(milliseconds: 180),
                                opacity: _ringVisible ? 1.0 : 0.0,
                                child: CustomPaint(
                                  painter: _CloseButtonHighlightPainter(
                                    position: _cursorPos!,
                                    accentColor: AppColors.hardwareAccent,
                                    widgetSize: Size(displayW, displayH),
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),

              const SizedBox(width: 6),
              // ── Right swipe arrow ──────────────────────────────────────────
              _SwipeArrow(
                direction: _SwipeDirection.right,
                visible: canSwipeRight,
                onTap: () => _api.simulateSwipe(direction: 'left').catchError((_) {}),
              ),
            ],
          );
        },
      ),
    );
  }
}

// ── Close-button hover highlight ──────────────────────────────────────────────
// Draws an accent-tinted circular glow centered on the close (✕) icon.
// Radius is proportional to the widget height so it looks right at any scale.

class _CloseButtonHighlightPainter extends CustomPainter {
  const _CloseButtonHighlightPainter({
    required this.position,
    required this.accentColor,
    required this.widgetSize,
  });
  final Offset position;
  final Color accentColor;
  final Size widgetSize;

  @override
  void paint(Canvas canvas, Size size) {
    // Scale radius to widget height so it looks consistent at any preview scale.
    final r = widgetSize.height * 0.30;

    // Outer diffuse glow
    canvas.drawCircle(
      position,
      r * 1.6,
      Paint()
        ..color = accentColor.withValues(alpha: 0.12)
        ..maskFilter = MaskFilter.blur(BlurStyle.normal, r * 0.8),
    );
    // Inner crisp ring
    canvas.drawCircle(
      position,
      r,
      Paint()
        ..color = accentColor.withValues(alpha: 0.50)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5,
    );
  }

  @override
  bool shouldRepaint(_CloseButtonHighlightPainter old) =>
      old.position != position ||
      old.accentColor != accentColor ||
      old.widgetSize != widgetSize;
}

// ── Swipe arrows ──────────────────────────────────────────────────────────────

enum _SwipeDirection { left, right }

class _SwipeArrow extends StatefulWidget {
  const _SwipeArrow({
    required this.direction,
    required this.visible,
    required this.onTap,
  });
  final _SwipeDirection direction;
  final bool visible;
  final VoidCallback onTap;

  @override
  State<_SwipeArrow> createState() => _SwipeArrowState();
}

class _SwipeArrowState extends State<_SwipeArrow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox(width: 20);
    final icon = widget.direction == _SwipeDirection.left
        ? Icons.chevron_left
        : Icons.chevron_right;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit:  (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedOpacity(
          duration: const Duration(milliseconds: 120),
          opacity: _hovered ? 0.7 : 0.25,
          child: Icon(icon, size: 20, color: Colors.white),
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
