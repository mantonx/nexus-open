import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../models/api_models.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';

const _displayWidth = 640;
const _zoneMinWidth = 80;

/// Visual drag-and-drop layout editor for hardware display pages and zones.
///
/// Structure:
///   • Live 640×48 hardware frame at the top (always current)
///   • Page tabs — click to switch, long-press to rename, + to add
///   • Zone list — width slider per zone, plugin dropdown, reorder handle
///   • Add/delete zone controls
class LayoutEditorTab extends StatefulWidget {
  const LayoutEditorTab({super.key});

  @override
  State<LayoutEditorTab> createState() => _LayoutEditorTabState();
}

class _LayoutEditorTabState extends State<LayoutEditorTab> {
  late NexusApiService _api;
  StreamSubscription? _wsSub;

  Uint8List? _frame;
  List<LayoutPage> _pages = [];
  int _selectedPageIdx = 0;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _loadLayout();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen((event) {
      if (event is WsFrameEvent && mounted) {
        setState(() => _frame = event.pngBytes);
      } else if (event is WsPageStateEvent && mounted) {
        // Keep selected page in sync with hardware when user swipes on device.
        if (event.currentPage < _pages.length) {
          setState(() => _selectedPageIdx = event.currentPage);
        }
      }
    });
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  Future<void> _loadLayout() async {
    setState(() { _loading = true; _error = null; });
    try {
      final pages = await _api.getLayout();
      if (mounted) setState(() { _pages = pages; _loading = false; });
    } catch (e) {
      if (mounted) setState(() { _error = e.toString(); _loading = false; });
    }
  }

  LayoutPage? get _currentPage =>
      _pages.isEmpty ? null : _pages[_selectedPageIdx.clamp(0, _pages.length - 1)];

  // ── Page operations ────────────────────────────────────────────────────────

  Future<void> _addPage() async {
    final name = await _promptText(context, 'New page name', 'Page ${_pages.length + 1}');
    if (name == null || name.isEmpty) return;
    try {
      await _api.createPage(name, _pages.length);
      await _loadLayout();
      setState(() => _selectedPageIdx = _pages.length - 1);
    } catch (e) {
      _showError('Failed to create page: $e');
    }
  }

  Future<void> _renamePage(LayoutPage page) async {
    final name = await _promptText(context, 'Rename page', page.name);
    if (name == null || name.isEmpty || name == page.name) return;
    try {
      await _api.updatePage(page.id, name, page.ord);
      await _loadLayout();
    } catch (e) {
      _showError('Failed to rename page: $e');
    }
  }

  Future<void> _deletePage(LayoutPage page) async {
    if (_pages.length == 1) {
      _showError('Cannot delete the last page.');
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Delete page?'),
        content: Text('Delete "${page.name}" and all its zones?'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            style: TextButton.styleFrom(foregroundColor: AppColors.error),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await _api.deletePage(page.id);
      await _loadLayout();
      setState(() => _selectedPageIdx = 0);
    } catch (e) {
      _showError('Failed to delete page: $e');
    }
  }

  // ── Zone operations ────────────────────────────────────────────────────────

  Future<void> _addZone() async {
    final page = _currentPage;
    if (page == null) return;

    final remaining = _displayWidth - page.totalWidth;
    if (remaining < _zoneMinWidth) {
      _showError('No space left on this page (zones must sum to 640px).');
      return;
    }

    final id = '${page.name.toLowerCase().replaceAll(' ', '_')}.zone${page.zones.length + 1}';
    final newWidth = remaining.clamp(_zoneMinWidth, remaining);

    // Shrink the last zone to make room.
    if (page.zones.isNotEmpty) {
      final last = page.zones.last;
      final shrunk = last.widthPx - newWidth;
      if (shrunk < _zoneMinWidth) {
        _showError('Not enough space to add a zone (min ${ _zoneMinWidth}px each).');
        return;
      }
      try {
        await _api.updateZone(last.copyWith(widthPx: shrunk));
      } catch (e) {
        _showError('Failed to resize adjacent zone: $e');
        return;
      }
    }

    try {
      await _api.createZone(LayoutZone(
        id: id,
        pageId: page.id,
        ord: page.zones.length,
        widthPx: newWidth,
      ));
      await _loadLayout();
    } catch (e) {
      _showError('Failed to create zone: $e');
    }
  }

  Future<void> _deleteZone(LayoutZone zone) async {
    final page = _currentPage;
    if (page == null) return;
    if (page.zones.length == 1) {
      _showError('A page must have at least one zone.');
      return;
    }

    // Give the deleted zone's width to its neighbour.
    final idx = page.zones.indexOf(zone);
    final neighbour = idx > 0 ? page.zones[idx - 1] : page.zones[idx + 1];

    try {
      await _api.updateZone(neighbour.copyWith(widthPx: neighbour.widthPx + zone.widthPx));
      await _api.deleteZone(zone.id);
      await _loadLayout();
    } catch (e) {
      _showError('Failed to delete zone: $e');
    }
  }

  // Committed when the slider is released — avoids flooding the API on drag.
  Future<void> _commitWidthChange(LayoutZone zone, LayoutZone neighbour) async {
    try {
      await Future.wait([
        _api.updateZone(zone),
        _api.updateZone(neighbour),
      ]);
      // No reload needed — local state already reflects the change.
    } catch (e) {
      _showError('Failed to save width: $e');
      await _loadLayout(); // resync from server on error
    }
  }

  Future<void> _changePlugin(LayoutZone zone, String plugin) async {
    zone.plugin = plugin;
    try {
      await _api.updateZone(zone);
    } catch (e) {
      _showError('Failed to update plugin: $e');
      await _loadLayout();
    }
  }

  Future<void> _changeAlign(LayoutZone zone, String align) async {
    zone.align = align;
    try {
      await _api.updateZone(zone);
    } catch (e) {
      _showError('Failed to update align: $e');
    }
  }

  // ── Reorder zones ──────────────────────────────────────────────────────────

  Future<void> _reorderZones(int oldIndex, int newIndex) async {
    final page = _currentPage;
    if (page == null) return;
    if (newIndex > oldIndex) newIndex -= 1;

    setState(() {
      final z = page.zones.removeAt(oldIndex);
      page.zones.insert(newIndex, z);
    });

    try {
      await _api.reorderZones(page.id, page.zones.map((z) => z.id).toList());
    } catch (e) {
      _showError('Failed to reorder zones: $e');
      await _loadLayout();
    }
  }

  // ── Build ──────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final connected = context.watch<WsService>().isConnected;
    final theme = Theme.of(context);

    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(_error!, style: const TextStyle(color: AppColors.error)),
            const SizedBox(height: AppSpacing.md),
            FilledButton(onPressed: _loadLayout, child: const Text('Retry')),
          ],
        ),
      );
    }

    final page = _currentPage;

    return Column(
      children: [
        // ── Live frame ───────────────────────────────────────────────────────
        Padding(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.md, AppSpacing.md, AppSpacing.md, 0),
          child: _LiveFrame(frame: _frame, connected: connected),
        ),

        // ── Page tabs ────────────────────────────────────────────────────────
        Padding(
          padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.md, vertical: AppSpacing.sm),
          child: _PageTabBar(
            pages: _pages,
            selectedIdx: _selectedPageIdx,
            onSelect: (i) {
              setState(() => _selectedPageIdx = i);
              _api.navigatePage(i).catchError((_) {});
            },
            onAdd: _addPage,
            onRename: _renamePage,
            onDelete: _deletePage,
          ),
        ),

        if (page == null)
          const Expanded(child: Center(child: Text('No pages — add one above')))
        else ...[
          // ── Zone width visualiser ──────────────────────────────────────────
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: AppSpacing.md),
            child: _ZoneWidthBar(zones: page.zones),
          ),
          const SizedBox(height: AppSpacing.xs),

          // ── Zone list ──────────────────────────────────────────────────────
          Expanded(
            child: ReorderableListView.builder(
              padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.md, vertical: AppSpacing.xs),
              onReorder: _reorderZones,
              itemCount: page.zones.length,
              itemBuilder: (ctx, i) {
                final z = page.zones[i];
                final hasNext = i < page.zones.length - 1;
                final next = hasNext ? page.zones[i + 1] : null;
                return _ZoneCard(
                  key: ValueKey(z.id),
                  zone: z,
                  nextZone: next,
                  canDelete: page.zones.length > 1,
                  onWidthChanged: (newWidth, nextWidth) {
                    setState(() {
                      z.widthPx = newWidth;
                      if (next != null) next.widthPx = nextWidth;
                    });
                  },
                  onWidthCommitted: next != null
                      ? () => _commitWidthChange(z, next)
                      : null,
                  onPluginChanged: (m) => _changePlugin(z, m),
                  onAlignChanged: (a) => _changeAlign(z, a),
                  onDelete: () => _deleteZone(z),
                );
              },
            ),
          ),

          // ── Add zone button ─────────────────────────────────────────────
          Padding(
            padding: const EdgeInsets.all(AppSpacing.md),
            child: Row(
              children: [
                OutlinedButton.icon(
                  onPressed: _addZone,
                  icon: const Icon(Icons.add, size: AppIconSize.sm),
                  label: const Text('Add zone'),
                ),
                const SizedBox(width: AppSpacing.sm),
                Text(
                  page.isValid
                      ? '640px ✓'
                      : '${page.totalWidth}px — must equal 640',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: page.isValid ? AppColors.success : AppColors.error,
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  void _showError(String msg) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(msg), backgroundColor: AppColors.error),
    );
  }

  Future<String?> _promptText(
      BuildContext context, String title, String initial) async {
    final ctrl = TextEditingController(text: initial);
    return showDialog<String>(
      context: context,
      builder: (_) => AlertDialog(
        title: Text(title),
        content: TextField(
          controller: ctrl,
          autofocus: true,
          onSubmitted: (v) => Navigator.pop(context, v),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.pop(context, ctrl.text),
              child: const Text('OK')),
        ],
      ),
    );
  }
}

// ── Live frame ────────────────────────────────────────────────────────────────

class _LiveFrame extends StatelessWidget {
  const _LiveFrame({required this.frame, required this.connected});
  final Uint8List? frame;
  final bool connected;

  static const _housingHighlight = Color(0xFF242428);
  static const _glossySurround   = Color(0xFF0A0A0C);
  static const _mountingSlot     = Color(0xFF1A1A1E);
  static const _displayBorder    = Color(0xFF1C1C20);

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [Color(0xFF1A1A1C), Color(0xFF0E0E10), Color(0xFF121214)],
          stops: [0.0, 0.4, 1.0],
        ),
        borderRadius: const BorderRadius.all(Radius.circular(4)),
        border: Border.all(color: _housingHighlight, width: 0.5),
        boxShadow: [
          BoxShadow(color: Colors.black.withValues(alpha: 0.6), blurRadius: 16, offset: const Offset(0, 6)),
        ],
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 0, vertical: 8),
        child: Row(children: [
          const _MountingEnd(slot: _mountingSlot),
          Expanded(
            child: Container(
              decoration: BoxDecoration(
                color: _glossySurround,
                gradient: LinearGradient(
                  begin: Alignment.topLeft, end: Alignment.bottomRight,
                  colors: [Colors.white.withValues(alpha: 0.04), Colors.transparent],
                ),
              ),
              padding: const EdgeInsets.all(6),
              child: Container(
                decoration: BoxDecoration(
                  border: Border.all(color: _displayBorder, width: 1),
                  borderRadius: BorderRadius.circular(1),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(1),
                  child: AspectRatio(
                    aspectRatio: 640 / 48,
                    child: frame != null
                        ? Image.memory(frame!, fit: BoxFit.fill,
                            gaplessPlayback: true, filterQuality: FilterQuality.none)
                        : Container(color: const Color(0xFF101010),
                            alignment: Alignment.center,
                            child: Text(
                              connected ? 'Waiting for frame…' : 'Not connected',
                              style: TextStyle(
                                fontSize: 7,
                                color: connected ? Colors.white24 : Colors.red.withValues(alpha: 0.4),
                              ),
                            )),
                  ),
                ),
              ),
            ),
          ),
          const _MountingEnd(slot: _mountingSlot),
        ]),
      ),
    );
  }
}

// ── Page tab bar ──────────────────────────────────────────────────────────────

class _PageTabBar extends StatelessWidget {
  const _PageTabBar({
    required this.pages,
    required this.selectedIdx,
    required this.onSelect,
    required this.onAdd,
    required this.onRename,
    required this.onDelete,
  });

  final List<LayoutPage> pages;
  final int selectedIdx;
  final void Function(int) onSelect;
  final VoidCallback onAdd;
  final void Function(LayoutPage) onRename;
  final void Function(LayoutPage) onDelete;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          for (int i = 0; i < pages.length; i++)
            GestureDetector(
              onTap: () => onSelect(i),
              onLongPress: () => onRename(pages[i]),
              child: AnimatedContainer(
                duration: AppDuration.fast,
                margin: const EdgeInsets.only(right: AppSpacing.xs),
                padding: const EdgeInsets.symmetric(
                    horizontal: AppSpacing.md, vertical: AppSpacing.xs),
                decoration: BoxDecoration(
                  color: i == selectedIdx
                      ? AppColors.accent.withValues(alpha: 0.15)
                      : Colors.transparent,
                  borderRadius: AppRadius.smBr,
                  border: Border.all(
                    color: i == selectedIdx
                        ? AppColors.accent
                        : AppColors.darkBorder,
                    width: i == selectedIdx ? 1.5 : 1,
                  ),
                ),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  Text(
                    pages[i].name,
                    style: theme.textTheme.labelMedium?.copyWith(
                      color: i == selectedIdx
                          ? AppColors.accent
                          : AppColors.textSecondary,
                      fontWeight: i == selectedIdx
                          ? FontWeight.w600
                          : FontWeight.w400,
                    ),
                  ),
                  if (i == selectedIdx) ...[
                    const SizedBox(width: AppSpacing.xs),
                    GestureDetector(
                      onTap: () => onDelete(pages[i]),
                      child: const Icon(Icons.close,
                          size: AppIconSize.xs, color: AppColors.textMuted),
                    ),
                  ],
                ]),
              ),
            ),
          // Add page button
          InkWell(
            onTap: onAdd,
            borderRadius: AppRadius.smBr,
            child: Container(
              padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.sm, vertical: AppSpacing.xs),
              decoration: BoxDecoration(
                border: Border.all(color: AppColors.darkBorder),
                borderRadius: AppRadius.smBr,
              ),
              child: const Icon(Icons.add, size: AppIconSize.sm,
                  color: AppColors.textMuted),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Zone width bar ────────────────────────────────────────────────────────────

class _ZoneWidthBar extends StatelessWidget {
  const _ZoneWidthBar({required this.zones});
  final List<LayoutZone> zones;

  static const _palette = [
    Color(0xFF4F9EFF), Color(0xFFFF6B6B), Color(0xFF6BFF9E),
    Color(0xFFFFD76B), Color(0xFFD36BFF), Color(0xFFFF9E6B),
  ];

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return ClipRRect(
      borderRadius: AppRadius.smBr,
      child: SizedBox(
        height: 20,
        child: Row(
          children: [
            for (int i = 0; i < zones.length; i++) ...[
              Expanded(
                flex: zones[i].widthPx,
                child: Container(
                  color: _palette[i % _palette.length].withValues(alpha: 0.2),
                  alignment: Alignment.center,
                  child: Text(
                    '${zones[i].widthPx}px',
                    style: TextStyle(
                        fontSize: 8, color: _palette[i % _palette.length]),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              if (i < zones.length - 1)
                Container(width: 1, color: cs.outline.withValues(alpha: 0.4)),
            ],
          ],
        ),
      ),
    );
  }
}

// ── Zone card ─────────────────────────────────────────────────────────────────

class _ZoneCard extends StatelessWidget {
  const _ZoneCard({
    super.key,
    required this.zone,
    required this.nextZone,
    required this.canDelete,
    required this.onWidthChanged,
    required this.onWidthCommitted,
    required this.onPluginChanged,
    required this.onAlignChanged,
    required this.onDelete,
  });

  final LayoutZone zone;
  final LayoutZone? nextZone;
  final bool canDelete;
  final void Function(int zoneWidth, int nextWidth) onWidthChanged;
  final VoidCallback? onWidthCommitted;
  final void Function(String) onPluginChanged;
  final void Function(String) onAlignChanged;
  final VoidCallback onDelete;

  static const _alignOptions = ['left', 'center', 'right'];

  // Available plugins grouped for the dropdown.
  static const _pluginOptions = [
    'builtin:placeholder',
    'builtin:clock',
    'builtin:clock24',
    'exec:./plugins/weather/weather',
    'exec:./plugins/cpu-temp/cpu-temp',
    'exec:./plugins/gpu-temp/gpu-temp',
    'exec:./plugins/network/network',
  ];

  static String _pluginLabel(String m) {
    if (m == 'builtin:placeholder') return 'Placeholder';
    if (m == 'builtin:clock') return 'Clock (12h)';
    if (m == 'builtin:clock24') return 'Clock (24h)';
    if (m.startsWith('exec:./plugins/')) {
      final name = m.split('/')[2];
      return name.split('-').map((w) => w[0].toUpperCase() + w.substring(1)).join(' ');
    }
    return m;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    // Max width for the slider: zone can grow up to (current + next - minWidth)
    final maxWidth = nextZone != null
        ? (zone.widthPx + nextZone!.widthPx - _zoneMinWidth)
            .clamp(_zoneMinWidth, _displayWidth - _zoneMinWidth)
        : zone.widthPx;

    return Card(
      margin: const EdgeInsets.only(bottom: AppSpacing.xs),
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.sm),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Drag handle
            const Padding(
              padding: EdgeInsets.only(top: AppSpacing.xs, right: AppSpacing.sm),
              child: ReorderableDragStartListener(
                index: 0, // index is injected by ReorderableListView
                child: Icon(Icons.drag_handle,
                    size: AppIconSize.md, color: AppColors.textMuted),
              ),
            ),

            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Zone ID + delete
                  Row(children: [
                    Expanded(
                      child: Text(zone.id,
                          style: theme.textTheme.labelMedium
                              ?.copyWith(fontFamily: 'monospace')),
                    ),
                    if (canDelete)
                      IconButton(
                        icon: const Icon(Icons.delete_outline),
                        iconSize: AppIconSize.sm,
                        color: AppColors.textMuted,
                        onPressed: onDelete,
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(),
                      ),
                  ]),

                  const SizedBox(height: AppSpacing.xs),

                  // Width slider (only shown if there's a next zone to resize)
                  if (nextZone != null) ...[
                    Row(children: [
                      Text('Width', style: theme.textTheme.bodySmall),
                      const SizedBox(width: AppSpacing.xs),
                      Text('${zone.widthPx}px',
                          style: theme.textTheme.labelSmall
                              ?.copyWith(color: AppColors.accent)),
                    ]),
                    SliderTheme(
                      data: SliderTheme.of(context).copyWith(
                        trackHeight: 2,
                        thumbShape: const RoundSliderThumbShape(
                            enabledThumbRadius: 6),
                        overlayShape: const RoundSliderOverlayShape(
                            overlayRadius: 12),
                      ),
                      child: Slider(
                        value: zone.widthPx.toDouble(),
                        min: _zoneMinWidth.toDouble(),
                        max: maxWidth.toDouble(),
                        divisions: (maxWidth - _zoneMinWidth) ~/ 10,
                        onChanged: (v) {
                          final newW = v.round();
                          final delta = newW - zone.widthPx;
                          final nextW = nextZone!.widthPx - delta;
                          if (nextW >= _zoneMinWidth) {
                            onWidthChanged(newW, nextW);
                          }
                        },
                        onChangeEnd: (_) => onWidthCommitted?.call(),
                      ),
                    ),
                  ] else
                    Text('Width: ${zone.widthPx}px',
                        style: theme.textTheme.bodySmall),

                  const SizedBox(height: AppSpacing.xs),

                  // Plugin dropdown + align dropdown
                  Row(children: [
                    Expanded(
                      flex: 3,
                      child: _compact(
                        context,
                        DropdownButton<String>(
                          value: _pluginOptions.contains(zone.plugin)
                              ? zone.plugin
                              : null,
                          hint: Text(zone.plugin,
                              style: theme.textTheme.bodySmall
                                  ?.copyWith(fontFamily: 'monospace'),
                              overflow: TextOverflow.ellipsis),
                          isExpanded: true,
                          underline: const SizedBox(),
                          items: _pluginOptions.map((m) => DropdownMenuItem(
                                value: m,
                                child: Text(_pluginLabel(m),
                                    style: theme.textTheme.bodySmall),
                              )).toList(),
                          onChanged: (m) { if (m != null) onPluginChanged(m); },
                        ),
                      ),
                    ),
                    const SizedBox(width: AppSpacing.xs),
                    Expanded(
                      flex: 1,
                      child: _compact(
                        context,
                        DropdownButton<String>(
                          value: zone.align,
                          isExpanded: true,
                          underline: const SizedBox(),
                          items: _alignOptions.map((a) => DropdownMenuItem(
                                value: a,
                                child: Text(a, style: theme.textTheme.bodySmall),
                              )).toList(),
                          onChanged: (a) { if (a != null) onAlignChanged(a); },
                        ),
                      ),
                    ),
                  ]),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  // Wrap dropdown in a tight container so it doesn't overflow card padding.
  Widget _compact(BuildContext context, Widget child) {
    final cs = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.sm),
      decoration: BoxDecoration(
        color: cs.surfaceContainerHigh,
        borderRadius: AppRadius.smBr,
        border: Border.all(color: AppColors.darkBorder),
      ),
      child: child,
    );
  }
}

// ── Mounting bracket ──────────────────────────────────────────────────────────

class _MountingEnd extends StatelessWidget {
  const _MountingEnd({required this.slot});
  final Color slot;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 12,
      child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
        _slotWidget(slot),
        const SizedBox(height: 8),
        _slotWidget(slot),
      ]),
    );
  }

  Widget _slotWidget(Color color) => Container(
        width: 3, height: 12,
        decoration: BoxDecoration(
          color: color, borderRadius: BorderRadius.circular(2),
          boxShadow: [BoxShadow(color: Colors.black.withValues(alpha: 0.5),
              blurRadius: 2, offset: const Offset(1, 1))],
        ),
      );
}
