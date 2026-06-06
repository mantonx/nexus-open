import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';

/// Full 1:1 hardware display preview with interactive swipe simulation.
///
/// Shows the live composited 640×48 frame inside a faithful hardware mockup,
/// page indicator dots, zone labels on long-press, and drag-to-swipe.
class HardwarePreviewTab extends StatefulWidget {
  const HardwarePreviewTab({super.key});

  @override
  State<HardwarePreviewTab> createState() => _HardwarePreviewTabState();
}

class _HardwarePreviewTabState extends State<HardwarePreviewTab> {
  StreamSubscription? _wsSub;
  final _api = NexusApiService();

  Uint8List? _frame;
  int _currentPage = 0;
  int _numPages = 0;
  List<WsPageInfo> _pages = [];

  // Drag state
  bool _dragging = false;
  double _dragStartX = 0;
  double _dragProgress = 0;
  bool _dragIsLeft = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen(_onEvent);
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    _api.dispose();
    super.dispose();
  }

  void _onEvent(WsEvent event) {
    if (!mounted) return;
    if (event is WsFrameEvent) {
      setState(() => _frame = event.pngBytes);
    } else if (event is WsPageStateEvent) {
      setState(() {
        _currentPage = event.currentPage;
        _numPages = event.numPages;
        _pages = event.pages;
      });
    }
  }

  // ── Drag handlers ────────────────────────────────────────────────────────

  void _onDragStart(DragStartDetails d) {
    _dragging = true;
    _dragStartX = d.localPosition.dx;
    _dragProgress = 0;
  }

  void _onDragUpdate(DragUpdateDetails d, double widgetWidth) {
    if (!_dragging) return;
    final delta = d.localPosition.dx - _dragStartX;
    _dragIsLeft = delta < 0;
    // Progress is absolute drag fraction of display width, clamped 0–1.
    final progress = (delta.abs() / widgetWidth).clamp(0.0, 1.0);
    _dragProgress = progress;
    _api.swipeUpdate(progress, isLeft: _dragIsLeft);
  }

  void _onDragEnd(DragEndDetails d, double widgetWidth) {
    if (!_dragging) return;
    _dragging = false;
    // Convert Flutter px/s velocity (screen coords) to display px/s (640px logical).
    final screenVelocity = d.velocity.pixelsPerSecond.dx.abs();
    // Scale: widgetWidth px on screen = 640 display px.
    final displayVelocity =
        widgetWidth > 0 ? screenVelocity * 640 / widgetWidth : screenVelocity;
    _api.swipeFinalize(_dragProgress, displayVelocity, isLeft: _dragIsLeft);
  }

  void _onDragCancel() {
    if (!_dragging) return;
    _dragging = false;
    _api.swipeCancel();
  }

  // ── Page dot tap ─────────────────────────────────────────────────────────

  Future<void> _goToPage(int index) async {
    try {
      await _api.navigatePage(index);
    } catch (_) {}
  }

  // ── Zone label overlay ───────────────────────────────────────────────────

  void _showZoneOverlay(BuildContext context) {
    if (_pages.isEmpty || _currentPage >= _pages.length) return;
    final page = _pages[_currentPage];
    if (page.zones.isEmpty) return;

    showDialog<void>(
      context: context,
      builder: (_) => AlertDialog(
        title: Text('Zones — ${page.name}'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final z in page.zones)
              Padding(
                padding:
                    const EdgeInsets.symmetric(vertical: AppSpacing.xs / 2),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(z.id,
                          style: const TextStyle(fontFamily: 'monospace')),
                    ),
                    Text('${z.width}px',
                        style: const TextStyle(color: AppColors.textMuted)),
                  ],
                ),
              ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context), child: const Text('OK'))
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.watch<WsService>();
    final connected = ws.isConnected;
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        // ── Header ──────────────────────────────────────────────────────────
        Padding(
          padding: const EdgeInsets.only(bottom: AppSpacing.sm),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Live Hardware Preview',
                        style: theme.textTheme.titleMedium),
                    const SizedBox(height: 2),
                    Text(
                      connected
                          ? 'Drag to swipe · Tap dots to jump · Long-press to see zones'
                          : 'Connect to backend to see live frames',
                      style: theme.textTheme.bodySmall
                          ?.copyWith(color: AppColors.textMuted),
                    ),
                  ],
                ),
              ),
              if (!connected)
                Icon(Icons.cable_outlined,
                    size: AppIconSize.lg, color: cs.error),
            ],
          ),
        ),

        // ── Hardware frame + live display ────────────────────────────────────
        LayoutBuilder(
          builder: (context, constraints) {
            final w = constraints.maxWidth;
            return GestureDetector(
              onLongPress: () => _showZoneOverlay(context),
              onHorizontalDragStart: connected ? _onDragStart : null,
              onHorizontalDragUpdate: connected
                  ? (d) => _onDragUpdate(d, w)
                  : null,
              onHorizontalDragEnd: connected
                  ? (d) => _onDragEnd(d, w)
                  : null,
              onHorizontalDragCancel:
                  connected ? _onDragCancel : null,
              child: _HardwareFrame(
                frame: _frame,
                connected: connected,
              ),
            );
          },
        ),

        const SizedBox(height: AppSpacing.md),

        // ── Page indicator dots ──────────────────────────────────────────────
        if (_numPages > 1) ...[
          _PageDots(
            numPages: _numPages,
            currentPage: _currentPage,
            pages: _pages,
            onTap: connected ? _goToPage : null,
          ),
          const SizedBox(height: AppSpacing.md),
        ],

        // ── Swipe buttons ────────────────────────────────────────────────────
        if (connected && _numPages > 1)
          _SwipeButtons(
            onLeft: () => _api.simulateSwipe(direction: 'left'),
            onRight: () => _api.simulateSwipe(direction: 'right'),
          ),

        const SizedBox(height: AppSpacing.md),

        // ── Zone breakdown for current page ──────────────────────────────────
        if (_pages.isNotEmpty && _currentPage < _pages.length)
          _ZoneBreakdown(page: _pages[_currentPage]),
      ],
    );
  }
}

// ── Hardware frame widget ─────────────────────────────────────────────────────
// Reuses the exact same housing constants as _DevicePreview in preview_tab.dart
// but renders Image.memory instead of static text.

class _HardwareFrame extends StatelessWidget {
  const _HardwareFrame({required this.frame, required this.connected});

  final Uint8List? frame;
  final bool connected;

  static const _housingHighlight = Color(0xFF242428);
  static const _glossySurround = Color(0xFF0A0A0C);
  static const _mountingSlot = Color(0xFF1A1A1E);
  static const _displayBorder = Color(0xFF1C1C20);

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Center(
      child: Column(
        children: [
          // In light mode wrap the housing in a dark recessed panel so the
          // near-black hardware reads as a physical object, not a dark widget
          // floating on a white canvas.
          Container(
            decoration: isDark
                ? null
                : BoxDecoration(
                    color: const Color(0xFF1A1A1F),
                    borderRadius: BorderRadius.circular(8),
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withValues(alpha: 0.5),
                        blurRadius: 24,
                        offset: const Offset(0, 8),
                      ),
                    ],
                  ),
            padding: isDark ? null : const EdgeInsets.all(16),
            child: Container(
            decoration: BoxDecoration(
              gradient: const LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  Color(0xFF1A1A1C),
                  Color(0xFF0E0E10),
                  Color(0xFF121214),
                ],
                stops: [0.0, 0.4, 1.0],
              ),
              borderRadius: const BorderRadius.all(Radius.circular(4)),
              border: Border.all(color: _housingHighlight, width: 0.5),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.75),
                  blurRadius: 20,
                  offset: const Offset(0, 8),
                ),
              ],
            ),
            child: Padding(
              padding:
                  const EdgeInsets.symmetric(horizontal: 0, vertical: 10),
              child: Row(
                children: [
                  const _MountingEnd(slot: _mountingSlot),
                  Expanded(
                    child: Container(
                      decoration: BoxDecoration(
                        color: _glossySurround,
                        gradient: LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: [
                            Colors.white.withValues(alpha: 0.05),
                            Colors.transparent,
                          ],
                        ),
                      ),
                      padding: const EdgeInsets.symmetric(
                          horizontal: 8, vertical: 8),
                      child: Container(
                        decoration: BoxDecoration(
                          border:
                              Border.all(color: _displayBorder, width: 1),
                          borderRadius: BorderRadius.circular(1),
                          boxShadow: [
                            BoxShadow(
                              color: Colors.black.withValues(alpha: 0.9),
                              blurRadius: 6,
                              spreadRadius: 1,
                            ),
                          ],
                        ),
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(1),
                          child: AspectRatio(
                            // 640:48 = 13.333...
                            aspectRatio: 640 / 48,
                            child: Stack(
                              fit: StackFit.expand,
                              children: [
                                // Live frame or placeholder
                                frame != null
                                    ? Image.memory(
                                        frame!,
                                        fit: BoxFit.fill,
                                        gaplessPlayback: true,
                                        filterQuality: FilterQuality.none,
                                      )
                                    : Container(
                                        color: const Color(0xFF101010),
                                        alignment: Alignment.center,
                                        child: Text(
                                          connected
                                              ? 'Waiting for frame…'
                                              : 'Not connected',
                                          style: TextStyle(
                                            color: connected
                                                ? Colors.white38
                                                : Colors.red.withValues(alpha: 0.5),
                                            fontSize: 8,
                                            fontFamily: 'monospace',
                                          ),
                                        ),
                                      ),
                                // Scanline texture
                                Positioned.fill(
                                  child: CustomPaint(
                                      painter: _ScanlinePainter()),
                                ),
                                // Subtle gloss reflection
                                Positioned.fill(
                                  child: DecoratedBox(
                                    decoration: BoxDecoration(
                                      gradient: LinearGradient(
                                        begin: Alignment.topLeft,
                                        end: Alignment.centerRight,
                                        colors: [
                                          Colors.white.withValues(alpha: 0.06),
                                          Colors.transparent,
                                        ],
                                        stops: const [0.0, 0.4],
                                      ),
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
                  const _MountingEnd(slot: _mountingSlot),
                ],
              ),
            ),
          ),
          ), // closes light-mode panel Container
          const SizedBox(height: AppSpacing.xs),
          Text(
            'Corsair iCUE Nexus  ·  640 × 48',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: AppColors.textMuted,
                  letterSpacing: 0.6,
                ),
          ),
        ],
      ),
    );
  }
}

// ── Page indicator dots ───────────────────────────────────────────────────────

class _PageDots extends StatelessWidget {
  const _PageDots({
    required this.numPages,
    required this.currentPage,
    required this.pages,
    required this.onTap,
  });

  final int numPages;
  final int currentPage;
  final List<WsPageInfo> pages;
  final void Function(int)? onTap;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        for (int i = 0; i < numPages; i++) ...[
          Tooltip(
            message: i < pages.length ? pages[i].name : 'Page ${i + 1}',
            child: GestureDetector(
              onTap: onTap != null ? () => onTap!(i) : null,
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 200),
                width: i == currentPage ? 20 : 8,
                height: 8,
                decoration: BoxDecoration(
                  color: i == currentPage
                      ? AppColors.accent
                      : AppColors.accent.withValues(alpha: 0.3),
                  borderRadius: BorderRadius.circular(4),
                ),
              ),
            ),
          ),
          if (i < numPages - 1) const SizedBox(width: 6),
        ],
      ],
    );
  }
}

// ── Swipe buttons ─────────────────────────────────────────────────────────────

class _SwipeButtons extends StatelessWidget {
  const _SwipeButtons({required this.onLeft, required this.onRight});
  final VoidCallback onLeft;
  final VoidCallback onRight;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        OutlinedButton.icon(
          onPressed: onRight,
          icon: const Icon(Icons.chevron_left, size: 18),
          label: const Text('Prev page'),
        ),
        const SizedBox(width: AppSpacing.sm),
        OutlinedButton.icon(
          onPressed: onLeft,
          icon: const Icon(Icons.chevron_right, size: 18),
          label: const Text('Next page'),
        ),
      ],
    );
  }
}

// ── Zone breakdown ────────────────────────────────────────────────────────────

class _ZoneBreakdown extends StatelessWidget {
  const _ZoneBreakdown({required this.page});
  final WsPageInfo page;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final totalWidth =
        page.zones.fold<int>(0, (sum, z) => sum + z.width);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Page: ${page.name}',
          style: theme.textTheme.labelMedium
              ?.copyWith(color: AppColors.textMuted),
        ),
        const SizedBox(height: AppSpacing.xs),
        // Proportional zone strip
        ClipRRect(
          borderRadius: AppRadius.smBr,
          child: SizedBox(
            height: 24,
            child: Row(
              children: [
                for (int i = 0; i < page.zones.length; i++) ...[
                  Expanded(
                    flex: page.zones[i].width,
                    child: Container(
                      color: _zoneColor(i).withValues(alpha: 0.25),
                      alignment: Alignment.center,
                      child: Text(
                        page.zones[i].id.split('.').last,
                        style: TextStyle(
                          fontSize: 9,
                          color: _zoneColor(i),
                          fontFamily: 'monospace',
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                  if (i < page.zones.length - 1)
                    Container(width: 1, color: cs.outline.withValues(alpha: 0.4)),
                ],
              ],
            ),
          ),
        ),
        const SizedBox(height: AppSpacing.xs / 2),
        // Zone list with widths
        for (final z in page.zones)
          Padding(
            padding:
                const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                Container(
                  width: 6,
                  height: 6,
                  decoration: BoxDecoration(
                    color: _zoneColor(page.zones.indexOf(z)),
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: AppSpacing.xs),
                Expanded(
                  child: Text(z.id,
                      style: theme.textTheme.bodySmall
                          ?.copyWith(fontFamily: 'monospace')),
                ),
                Text(
                  '${z.width}px  '
                  '(${(z.width / (totalWidth > 0 ? totalWidth : 640) * 100).round()}%)',
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: AppColors.textMuted),
                ),
              ],
            ),
          ),
      ],
    );
  }

  static const _palette = [
    Color(0xFF00C8FF),
    Color(0xFFFF6B6B),
    Color(0xFF6BFF9E),
    Color(0xFFFFD76B),
    Color(0xFFD36BFF),
    Color(0xFFFF9E6B),
  ];

  Color _zoneColor(int i) => _palette[i % _palette.length];
}

// ── Mounting bracket ──────────────────────────────────────────────────────────

class _MountingEnd extends StatelessWidget {
  const _MountingEnd({required this.slot});
  final Color slot;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 14,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          _slot(slot),
          const SizedBox(height: 10),
          _slot(slot),
        ],
      ),
    );
  }

  Widget _slot(Color color) => Container(
        width: 4,
        height: 14,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(2),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.6),
              blurRadius: 2,
              offset: const Offset(1, 1),
            ),
          ],
        ),
      );
}

// ── Scanline painter ──────────────────────────────────────────────────────────

class _ScanlinePainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.black.withValues(alpha: 0.07)
      ..strokeWidth = 0.5;
    for (double y = 0; y < size.height; y += 2) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
    }
  }

  @override
  bool shouldRepaint(_ScanlinePainter _) => false;
}
