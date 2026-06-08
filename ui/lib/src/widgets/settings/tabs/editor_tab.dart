import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../models/api_models.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

const _kBreakpoint = 900.0;

class EditorTab extends StatefulWidget {
  const EditorTab({super.key});

  @override
  State<EditorTab> createState() => _EditorTabState();
}

class _EditorTabState extends State<EditorTab> {
  late NexusApiService _api;
  List<LayoutPage> _pages = [];
  List<PluginCatalogEntry> _catalog = [];
  int _selectedPageIdx = 0;
  String? _selectedZoneId;
  bool _loading = true;
  String? _error;
  StreamSubscription? _wsSub;
  bool _initialized = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen((event) {
      if (!mounted) return;
      if (event is WsDraftStateEvent && !event.active) {
        _load(useDraft: false);
      }
    });
    if (!_initialized) {
      _initialized = true;
      _load();
    }
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  Future<void> _load({bool useDraft = true}) async {
    setState(() { _loading = true; _error = null; });
    try {
      final results = await Future.wait([
        useDraft ? _api.getDraft() : _api.getLayout(),
        _api.getPluginCatalog(),
      ]);
      if (!mounted) return;
      setState(() {
        _pages = results[0] as List<LayoutPage>;
        _catalog = _dedupCatalog(results[1] as List<PluginCatalogEntry>);
        if (_selectedPageIdx >= _pages.length) _selectedPageIdx = 0;
        _loading = false;
      });
    } catch (e) {
      if (mounted) setState(() { _loading = false; _error = e.toString(); });
    }
  }

  // Deduplicate catalog entries that describe the same plugin (same descriptor name).
  // Prefer exec over builtin when both exist.
  static List<PluginCatalogEntry> _dedupCatalog(List<PluginCatalogEntry> raw) {
    final seen = <String, PluginCatalogEntry>{};
    for (final e in raw) {
      final key = e.descriptor.name.toLowerCase();
      final existing = seen[key];
      if (existing == null || e.kind == 'exec') {
        seen[key] = e;
      }
    }
    return seen.values.toList();
  }

  LayoutPage? get _currentPage =>
      _pages.isNotEmpty ? _pages[_selectedPageIdx] : null;

  LayoutZone? get _selectedZone => _selectedZoneId == null
      ? null
      : _currentPage?.zones
          .cast<LayoutZone?>()
          .firstWhere((z) => z?.id == _selectedZoneId, orElse: () => null);

  PluginCatalogEntry? _catalogFor(String pluginId) {
    // Exact match on catalog id.
    for (final e in _catalog) {
      if (e.id == pluginId || e.descriptor.id == pluginId) return e;
    }
    // Fuzzy: match the bare binary name from paths like ./plugins/weather/weather
    // against catalog ids like exec:weather or builtin:clock.
    final needle = _pluginBaseName(pluginId).toLowerCase();
    for (final e in _catalog) {
      if (_pluginBaseName(e.id).toLowerCase() == needle) return e;
    }
    return null;
  }

  // Extracts the bare name from any plugin identifier format:
  //   exec:weather                     → weather
  //   builtin:clock                    → clock
  //   exec:./plugins/weather/weather   → weather
  //   ./plugins/weather/weather        → weather
  static String _pluginBaseName(String id) {
    // Strip scheme prefix (exec:, builtin:) if present.
    var s = id.contains(':') ? id.split(':').last : id;
    // Strip path — take the last non-empty segment.
    final parts = s.split('/').where((p) => p.isNotEmpty).toList();
    return parts.isNotEmpty ? parts.last : s;
  }

  // ── Mutations ───────────────────────────────────────────────────────────────

  Future<void> _addZone(String pluginId, {String? insertBeforeId}) async {
    final pageIdx = _selectedPageIdx;
    try {
      final newId = await _api.addDraftZone(
        pageIndex: pageIdx,
        plugin: pluginId,
        insertBeforeId: insertBeforeId,
      );
      final pages = await _api.getDraft();
      if (!mounted) return;
      setState(() {
        _pages = pages;
        _selectedZoneId = newId;
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Add zone failed: $e')));
      }
    }
  }

  Future<void> _reorderZones(List<String> orderedIds) async {
    final pageIdx = _selectedPageIdx;
    try {
      await _api.reorderDraftZones(pageIdx, orderedIds);
      final pages = await _api.getDraft();
      if (!mounted) return;
      setState(() { _pages = pages; });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Reorder failed: $e')));
      }
    }
  }

  Future<void> _navigatePage(int pageIndex) async {
    try {
      await _api.navigatePage(pageIndex);
    } catch (_) {
      // hardware navigation is best-effort
    }
  }

  Future<void> _deleteZone(String zoneId) async {
    try {
      await _api.deleteDraftZone(zoneId);
      final pages = await _api.getDraft();
      if (!mounted) return;
      setState(() {
        _pages = pages;
        if (_selectedZoneId == zoneId) _selectedZoneId = null;
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete zone failed: $e')));
      }
    }
  }

  Future<void> _patchZone(String zoneId, Map<String, dynamic> patch) async {
    try {
      await _api.patchDraftZone(zoneId, patch);
      final pages = await _api.getDraft();
      if (!mounted) return;
      setState(() { _pages = pages; });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Update failed: $e')));
      }
    }
  }

  Future<void> _addPage() async {
    final name = await _promptText(context, 'New Page', 'Page name', 'Page ${_pages.length + 1}');
    if (name == null) return;
    try {
      await _api.createPage(name, _pages.length);
      await _load();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Create page failed: $e')));
    }
  }

  // pageIdx is the position in _pages — we resolve the real DB id from the
  // committed layout because draft pages carry id=0.
  Future<void> _deletePage(int pageIdx) async {
    try {
      final committed = await _api.getLayout();
      if (pageIdx >= committed.length) return;
      final realId = committed[pageIdx].id;
      await _api.deletePage(realId);
      setState(() {
        _selectedPageIdx = _selectedPageIdx.clamp(0, (_pages.length - 2).clamp(0, _pages.length));
        _selectedZoneId = null;
      });
      await _load();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete page failed: $e')));
    }
  }

  // ── Build ───────��────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Center(child: CircularProgressIndicator(color: AppColors.accent));
    }
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(_error!, style: const TextStyle(color: AppColors.hardwareAccent)),
            const SizedBox(height: 12),
            NexusButton.ghost(label: 'Retry', onPressed: _load),
          ],
        ),
      );
    }

    final width = MediaQuery.sizeOf(context).width;
    final isWide = width >= _kBreakpoint;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _PageStrip(
          pages: _pages,
          selectedIdx: _selectedPageIdx,
          onSelect: (i) => setState(() { _selectedPageIdx = i; _selectedZoneId = null; }),
          onAdd: _addPage,
          onDelete: _deletePage,
        ),
        Expanded(
          child: isWide
              ? Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 220,
                      child: _PluginLibrary(
                        catalog: _catalog,
                        onAdd: (id) => _addZone(id),
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    Expanded(
                      child: _ZonePage(
                        page: _currentPage,
                        pageIndex: _selectedPageIdx,
                        pageCount: _pages.length,
                        selectedZoneId: _selectedZoneId,
                        onSelect: (id) => setState(() => _selectedZoneId = id),
                        onDelete: _deleteZone,
                        onDropPlugin: _addZone,
                        onReorder: _reorderZones,
                        onNavigate: _navigatePage,
                        catalogFor: _catalogFor,
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    SizedBox(
                      width: 260,
                      child: _Configuration(
                        zone: _selectedZone,
                        catalog: _catalogFor(_selectedZone?.plugin ?? ''),
                        onPatch: _patchZone,
                      ),
                    ),
                  ],
                )
              : _NarrowLayout(
                  page: _currentPage,
                  pageIndex: _selectedPageIdx,
                  pageCount: _pages.length,
                  catalog: _catalog,
                  selectedZoneId: _selectedZoneId,
                  selectedZone: _selectedZone,
                  catalogFor: _catalogFor,
                  onSelectZone: (id) => setState(() => _selectedZoneId = id),
                  onAddZone: _addZone,
                  onDeleteZone: _deleteZone,
                  onPatchZone: _patchZone,
                  onReorderZones: _reorderZones,
                  onNavigate: _navigatePage,
                ),
        ),
      ],
    );
  }
}

// ── Page strip ────────────────────────────────────────────────────────────────

class _PageStrip extends StatelessWidget {
  const _PageStrip({
    required this.pages,
    required this.selectedIdx,
    required this.onSelect,
    required this.onAdd,
    required this.onDelete,
  });

  final List<LayoutPage> pages;
  final int selectedIdx;
  final ValueChanged<int> onSelect;
  final VoidCallback onAdd;
  final ValueChanged<int> onDelete;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Container(
      height: 40,
      decoration: BoxDecoration(
        color: cs.surfaceContainerLow,
        border: Border(bottom: BorderSide(color: cs.outline, width: 1)),
      ),
      child: Row(
        children: [
          Expanded(
            child: ListView.builder(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.symmetric(horizontal: AppSpacing.sm, vertical: 6),
              itemCount: pages.length,
              itemBuilder: (ctx, i) {
                final selected = i == selectedIdx;
                return Padding(
                  padding: const EdgeInsets.only(right: 4),
                  child: GestureDetector(
                    onTap: () => onSelect(i),
                    child: AnimatedContainer(
                      duration: AppDuration.fast,
                      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 2),
                      decoration: BoxDecoration(
                        color: selected
                            ? AppColors.accent.withValues(alpha: 0.12)
                            : Colors.transparent,
                        borderRadius: AppRadius.smBr,
                        border: Border.all(
                          color: selected ? AppColors.accent : cs.outline,
                          width: selected ? 1.5 : 1,
                        ),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            pages[i].name,
                            style: theme.textTheme.labelSmall?.copyWith(
                              color: selected ? AppColors.accent : cs.onSurfaceVariant,
                              fontWeight: selected ? FontWeight.w600 : FontWeight.normal,
                            ),
                          ),
                          if (pages.length > 1) ...[
                            const SizedBox(width: 4),
                            InkWell(
                              onTap: () => onDelete(i),
                              borderRadius: BorderRadius.circular(10),
                              child: Icon(Icons.close, size: 12,
                                  color: cs.onSurfaceVariant.withValues(alpha: 0.6)),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
          Padding(
            padding: const EdgeInsets.only(right: AppSpacing.sm),
            child: Tooltip(
              message: 'Add page',
              child: InkWell(
                onTap: onAdd,
                borderRadius: AppRadius.smBr,
                child: const Padding(
                  padding: EdgeInsets.all(6),
                  child: Icon(Icons.add, size: AppIconSize.sm, color: AppColors.accent),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Zone page ─────────────────────────────────────────────────────────────────
//
// Layout: [ ← arrow ] [ zone row with drag-to-insert/reorder ] [ → arrow ]
//
// The zone row is a single _ZoneRow stateful widget. It wraps the chip Row
// in one DragTarget that spans the full width. onMove tracks the pointer X
// to compute which gap (0…n) the cursor is over and highlights it. onLeave
// clears the highlight. onAccept fires the insert/reorder callback with the
// computed gap index.
//
// Chips are also LongPressDraggable<_ZoneDrag> for reorder.
// Swiping horizontally on the zone row calls onNavigate (hardware swipe).

/// Drag payload for an existing zone being reordered.
class _ZoneDrag {
  const _ZoneDrag(this.zoneId);
  final String zoneId;
}

class _ZonePage extends StatelessWidget {
  const _ZonePage({
    required this.page,
    required this.pageIndex,
    required this.pageCount,
    required this.selectedZoneId,
    required this.onSelect,
    required this.onDelete,
    required this.onDropPlugin,
    required this.onReorder,
    required this.onNavigate,
    required this.catalogFor,
  });

  final LayoutPage? page;
  final int pageIndex;
  final int pageCount;
  final String? selectedZoneId;
  final ValueChanged<String> onSelect;
  final ValueChanged<String> onDelete;
  final void Function(String pluginId, {String? insertBeforeId}) onDropPlugin;
  final ValueChanged<List<String>> onReorder;
  final ValueChanged<int> onNavigate;
  final PluginCatalogEntry? Function(String) catalogFor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final zones = page?.zones ?? [];
    final canPrev = pageIndex > 0;
    final canNext = pageIndex < pageCount - 1;

    Widget navArrow(IconData icon, bool enabled, int target) {
      return Tooltip(
        message: enabled
            ? (icon == Icons.chevron_left ? 'Previous page' : 'Next page')
            : '',
        child: InkWell(
          onTap: enabled ? () => onNavigate(target) : null,
          borderRadius: AppRadius.smBr,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            child: Icon(icon, size: 22,
              color: enabled ? AppColors.accent : cs.onSurfaceVariant.withValues(alpha: 0.2)),
          ),
        ),
      );
    }

    final zoneRow = zones.isEmpty
        ? _emptyDropTarget(context, cs, theme)
        : _ZoneRow(
            zones: zones,
            selectedZoneId: selectedZoneId,
            onSelect: onSelect,
            onDelete: onDelete,
            onDropPlugin: onDropPlugin,
            onReorder: onReorder,
            catalogFor: catalogFor,
          );

    return Padding(
      padding: const EdgeInsets.fromLTRB(AppSpacing.sm, AppSpacing.md, AppSpacing.sm, AppSpacing.sm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: 4, bottom: AppSpacing.xs),
            child: Text(
              page?.name.toUpperCase() ?? '',
              style: theme.textTheme.labelSmall?.copyWith(
                color: cs.onSurfaceVariant, letterSpacing: 1.2, fontSize: 9),
            ),
          ),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              navArrow(Icons.chevron_left, canPrev, pageIndex - 1),
              Expanded(
                child: GestureDetector(
                  onHorizontalDragEnd: (details) {
                    final v = details.primaryVelocity ?? 0;
                    if (v < -300 && canNext) onNavigate(pageIndex + 1);
                    if (v > 300 && canPrev) onNavigate(pageIndex - 1);
                  },
                  child: zoneRow,
                ),
              ),
              navArrow(Icons.chevron_right, canNext, pageIndex + 1),
            ],
          ),
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Center(
              child: Text(
                '${pageIndex + 1} / $pageCount',
                style: theme.textTheme.labelSmall?.copyWith(
                  fontSize: 9,
                  color: cs.onSurfaceVariant.withValues(alpha: 0.4),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _emptyDropTarget(BuildContext context, ColorScheme cs, ThemeData theme) {
    return DragTarget<Object>(
      onWillAcceptWithDetails: (d) => d.data is String,
      onAcceptWithDetails: (d) {
        if (d.data is String) onDropPlugin(d.data as String);
      },
      builder: (ctx, candidates, _) => AnimatedContainer(
        duration: AppDuration.fast,
        height: 64,
        decoration: BoxDecoration(
          color: candidates.isNotEmpty
              ? AppColors.accent.withValues(alpha: 0.08)
              : cs.surfaceContainerHigh.withValues(alpha: 0.5),
          borderRadius: AppRadius.smBr,
          border: Border.all(
            color: candidates.isNotEmpty ? AppColors.accent : cs.outline.withValues(alpha: 0.4),
            width: candidates.isNotEmpty ? 1.5 : 1,
            style: BorderStyle.solid,
          ),
        ),
        child: Center(
          child: Text(
            candidates.isNotEmpty ? 'Drop to add' : 'Drag a plugin here',
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 10,
              color: candidates.isNotEmpty ? AppColors.accent : cs.onSurfaceVariant.withValues(alpha: 0.5),
            ),
          ),
        ),
      ),
    );
  }
}

// The zone strip itself — stateful so it can track the active gap index
// while a drag is in flight.
class _ZoneRow extends StatefulWidget {
  const _ZoneRow({
    required this.zones,
    required this.selectedZoneId,
    required this.onSelect,
    required this.onDelete,
    required this.onDropPlugin,
    required this.onReorder,
    required this.catalogFor,
  });

  final List<LayoutZone> zones;
  final String? selectedZoneId;
  final ValueChanged<String> onSelect;
  final ValueChanged<String> onDelete;
  final void Function(String pluginId, {String? insertBeforeId}) onDropPlugin;
  final ValueChanged<List<String>> onReorder;
  final PluginCatalogEntry? Function(String) catalogFor;

  @override
  State<_ZoneRow> createState() => _ZoneRowState();
}

class _ZoneRowState extends State<_ZoneRow> {
  // Index of the gap being hovered (0 = before first chip, n = after last).
  // null means no drag in flight.
  int? _activeGap;
  // Whether the incoming drag is a new plugin (needs capacity check) or reorder.
  bool _isDragNew = false;

  final _rowKey = GlobalKey();

  // Compute which gap (0..n) a pointer at globalPos falls into.
  // Boundary between gap i and gap i+1 is at the midpoint of chip i.
  int _gapAt(Offset globalPos) {
    final box = _rowKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return widget.zones.length;
    final local = box.globalToLocal(globalPos);
    final x = local.dx.clamp(0.0, box.size.width);
    final n = widget.zones.length;
    if (n == 0) return 0;
    // Each chip gets an equal share of the row width for hit-testing purposes.
    final chipW = box.size.width / n;
    // Pointer is in gap i if it's in the left half of chip i, gap i+1 if right half.
    final raw = x / chipW;
    return raw.round().clamp(0, n);
  }

  String? _beforeIdForGap(int gap) =>
      gap < widget.zones.length ? widget.zones[gap].id : null;

  void _acceptDrop(Object data, int gap) {
    if (data is _ZoneDrag) {
      final ids = widget.zones.map((z) => z.id).toList();
      final from = ids.indexOf(data.zoneId);
      if (from < 0) return;
      ids.removeAt(from);
      final insertIdx = gap > from ? (gap - 1).clamp(0, ids.length) : gap.clamp(0, ids.length);
      ids.insert(insertIdx, data.zoneId);
      widget.onReorder(ids);
    } else if (data is String) {
      widget.onDropPlugin(data, insertBeforeId: _beforeIdForGap(gap));
    }
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final zones = widget.zones;
    final hasCapacity = zones.length < 6;

    return DragTarget<Object>(
      onWillAcceptWithDetails: (details) {
        _isDragNew = details.data is String;
        if (_isDragNew && !hasCapacity) return false;
        return true;
      },
      onMove: (details) {
        final gap = _gapAt(details.offset);
        if (gap != _activeGap) setState(() => _activeGap = gap);
      },
      onLeave: (_) => setState(() => _activeGap = null),
      onAcceptWithDetails: (details) {
        final gap = _activeGap ?? zones.length;
        setState(() => _activeGap = null);
        _acceptDrop(details.data, gap);
      },
      builder: (ctx, candidates, _) {
        final dragging = candidates.isNotEmpty;
        return _buildStrip(context, cs, zones, dragging);
      },
    );
  }

  Widget _buildStrip(BuildContext context, ColorScheme cs, List<LayoutZone> zones, bool dragging) {
    // Each existing zone gets flex = its widthPx (or 1 if unknown).
    // The average flex weight of existing zones is used for the ghost slot so
    // it looks exactly like a real zone would at that position.
    final flexes = zones.map((z) => z.widthPx > 0 ? z.widthPx : 1).toList();
    final avgFlex = flexes.isEmpty ? 1 : (flexes.reduce((a, b) => a + b) / flexes.length).round();

    final children = <Widget>[];
    for (int i = 0; i < zones.length; i++) {
      // Insert ghost slot before this chip when active
      if (dragging && _activeGap == i) {
        children.add(Expanded(
          flex: avgFlex,
          child: _GhostSlot(zoneCount: zones.length + 1),
        ));
      }
      final zone = zones[i];
      children.add(Expanded(
        flex: flexes[i],
        child: _ZoneChip(
          zone: zone,
          entry: widget.catalogFor(zone.plugin),
          selected: zone.id == widget.selectedZoneId,
          onTap: () => widget.onSelect(zone.id),
          onDelete: () => widget.onDelete(zone.id),
        ),
      ));
    }
    // Ghost at end
    if (dragging && _activeGap == zones.length) {
      children.add(Expanded(
        flex: avgFlex,
        child: _GhostSlot(zoneCount: zones.length + 1),
      ));
    }

    return SizedBox(
      key: _rowKey,
      height: 64,
      child: Row(children: children),
    );
  }
}

// A ghost slot showing where the dragged item would land.
// Same height as a real chip, dashed accent border, no content.
class _GhostSlot extends StatelessWidget {
  const _GhostSlot({required this.zoneCount});
  final int zoneCount;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2),
      child: Container(
        decoration: BoxDecoration(
          color: AppColors.accent.withValues(alpha: 0.08),
          borderRadius: AppRadius.smBr,
          border: Border.all(color: AppColors.accent, width: 1.5),
        ),
        child: Center(
          child: Icon(Icons.add, size: 14, color: AppColors.accent.withValues(alpha: 0.6)),
        ),
      ),
    );
  }
}

/// A single zone chip — LongPressDraggable for reorder, tappable for selection.
class _ZoneChip extends StatelessWidget {
  const _ZoneChip({
    required this.zone,
    required this.entry,
    required this.selected,
    required this.onTap,
    required this.onDelete,
  });

  final LayoutZone zone;
  final PluginCatalogEntry? entry;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final label = entry?.descriptor.name ?? _readablePluginName(zone.plugin);

    final chip = GestureDetector(
      onTap: onTap,
      child: AnimatedContainer(
        duration: AppDuration.fast,
        margin: const EdgeInsets.symmetric(horizontal: 1),
        decoration: BoxDecoration(
          color: selected
              ? AppColors.accent.withValues(alpha: 0.15)
              : cs.surfaceContainerHigh,
          borderRadius: AppRadius.smBr,
          border: Border.all(
            color: selected ? AppColors.accent : cs.outline,
            width: selected ? 1.5 : 1,
          ),
        ),
        child: Stack(
          children: [
            Center(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(4, 4, 14, 4),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      _iconFor(zone.plugin),
                      size: 14,
                      color: AppColors.accent,
                    ),
                    const SizedBox(height: 3),
                    Text(
                      label,
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        fontSize: 9,
                        color: selected ? AppColors.accent : cs.onSurfaceVariant,
                      ),
                      overflow: TextOverflow.ellipsis,
                      maxLines: 1,
                      textAlign: TextAlign.center,
                    ),
                  ],
                ),
              ),
            ),
            Positioned(
              top: 2,
              right: 2,
              child: GestureDetector(
                onTap: onDelete,
                child: Container(
                  padding: const EdgeInsets.all(2),
                  decoration: BoxDecoration(
                    color: cs.surfaceContainerHighest,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(Icons.close, size: 9, color: cs.onSurfaceVariant),
                ),
              ),
            ),
          ],
        ),
      ),
    );

    return Draggable<_ZoneDrag>(
      data: _ZoneDrag(zone.id),
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          width: 72,
          height: 56,
          decoration: BoxDecoration(
            color: cs.surfaceContainerHigh,
            borderRadius: AppRadius.smBr,
            border: Border.all(color: AppColors.accent, width: 1.5),
            boxShadow: [BoxShadow(color: AppColors.accent.withValues(alpha: 0.25), blurRadius: 8)],
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(_iconFor(zone.plugin), size: 14, color: AppColors.accent),
              const SizedBox(height: 3),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Text(label,
                    style: const TextStyle(fontSize: 9, color: AppColors.accent),
                    overflow: TextOverflow.ellipsis,
                    maxLines: 1,
                    textAlign: TextAlign.center),
              ),
            ],
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.3, child: chip),
      child: chip,
    );
  }

  static String _readablePluginName(String plugin) {
    final bare = plugin.contains(':') ? plugin.split(':').last : plugin;
    final last = bare.split('/').where((s) => s.isNotEmpty).last;
    return last
        .split('-')
        .map((w) => w.isEmpty ? '' : '${w[0].toUpperCase()}${w.substring(1)}')
        .join(' ');
  }

  static IconData _iconFor(String id) {
    if (id.contains('cpu')) return Icons.memory_outlined;
    if (id.contains('gpu')) return Icons.videogame_asset_outlined;
    if (id.contains('weather')) return Icons.wb_sunny_outlined;
    if (id.contains('network')) return Icons.wifi_outlined;
    if (id.contains('clock')) return Icons.access_time_outlined;
    if (id.contains('temp')) return Icons.thermostat_outlined;
    return Icons.extension_outlined;
  }
}

// ── Plugin library ────────────────────────────────────────────────────────────

class _PluginLibrary extends StatelessWidget {
  const _PluginLibrary({required this.catalog, required this.onAdd});

  final List<PluginCatalogEntry> catalog;
  final ValueChanged<String> onAdd;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(AppSpacing.md, AppSpacing.md, AppSpacing.md, AppSpacing.sm),
          child: Text(
            'PLUGINS',
            style: theme.textTheme.labelSmall?.copyWith(
              color: cs.onSurfaceVariant,
              letterSpacing: 1.5,
              fontSize: 9,
            ),
          ),
        ),
        Expanded(
          child: GridView.builder(
            padding: const EdgeInsets.fromLTRB(AppSpacing.sm, 0, AppSpacing.sm, AppSpacing.md),
            gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
              crossAxisCount: 2,
              mainAxisSpacing: 6,
              crossAxisSpacing: 6,
              mainAxisExtent: 76,
            ),
            itemCount: catalog.length,
            itemBuilder: (ctx, i) {
              final entry = catalog[i];
              return _PluginCard(entry: entry, onAdd: onAdd);
            },
          ),
        ),
      ],
    );
  }
}

class _PluginCard extends StatelessWidget {
  const _PluginCard({required this.entry, required this.onAdd});

  final PluginCatalogEntry entry;
  final ValueChanged<String> onAdd;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    final card = Tooltip(
      message: entry.descriptor.description,
      child: InkWell(
        onTap: () => onAdd(entry.id),
        borderRadius: AppRadius.smBr,
        child: Container(
          decoration: BoxDecoration(
            color: cs.surfaceContainerLow,
            borderRadius: AppRadius.smBr,
            border: Border.all(color: cs.outline.withValues(alpha: 0.5)),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: AppColors.accent.withValues(alpha: 0.1),
                  borderRadius: AppRadius.smBr,
                ),
                child: Icon(
                  _iconFor(entry.id),
                  size: 14,
                  color: AppColors.accent,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                entry.descriptor.name.isNotEmpty ? entry.descriptor.name : entry.id,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: cs.onSurface,
                  fontSize: 10,
                  fontWeight: FontWeight.w500,
                ),
                textAlign: TextAlign.center,
                overflow: TextOverflow.ellipsis,
                maxLines: 2,
              ),
            ],
          ),
        ),
      ),
    );

    return Draggable<String>(
      data: entry.id,
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          width: 80,
          height: 72,
          decoration: BoxDecoration(
            color: cs.surfaceContainerHigh,
            borderRadius: AppRadius.smBr,
            border: Border.all(color: AppColors.accent, width: 1.5),
            boxShadow: [
              BoxShadow(
                color: AppColors.accent.withValues(alpha: 0.2),
                blurRadius: 8,
                spreadRadius: 1,
              ),
            ],
          ),
          padding: const EdgeInsets.all(AppSpacing.sm),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(_iconFor(entry.id), size: 16, color: AppColors.accent),
              const SizedBox(height: 4),
              Text(
                entry.descriptor.name.isNotEmpty ? entry.descriptor.name : entry.id,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: AppColors.accent,
                  fontSize: 9,
                ),
                textAlign: TextAlign.center,
                overflow: TextOverflow.ellipsis,
                maxLines: 2,
              ),
            ],
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.35, child: card),
      child: card,
    );
  }

  static IconData _iconFor(String id) {
    if (id.contains('cpu')) return Icons.memory_outlined;
    if (id.contains('gpu')) return Icons.videogame_asset_outlined;
    if (id.contains('weather')) return Icons.wb_sunny_outlined;
    if (id.contains('network')) return Icons.wifi_outlined;
    if (id.contains('clock')) return Icons.access_time_outlined;
    if (id.contains('temp')) return Icons.thermostat_outlined;
    return Icons.extension_outlined;
  }
}

// ── Configuration panel ───────────────────────────────────────────────────────

class _Configuration extends StatelessWidget {
  const _Configuration({
    required this.zone,
    required this.catalog,
    required this.onPatch,
  });

  final LayoutZone? zone;
  final PluginCatalogEntry? catalog;
  final void Function(String zoneId, Map<String, dynamic> patch) onPatch;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    if (zone == null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(AppSpacing.md),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.touch_app_outlined,
                  size: 28, color: cs.onSurfaceVariant.withValues(alpha: 0.3)),
              const SizedBox(height: 8),
              Text(
                'Select a zone to configure',
                style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurfaceVariant),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    final z = zone!;
    final fields = catalog?.descriptor.schemaFields ?? [];

    return SingleChildScrollView(
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'CONFIGURATION',
            style: theme.textTheme.labelSmall?.copyWith(
              color: cs.onSurfaceVariant,
              letterSpacing: 1.5,
              fontSize: 9,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Row(
            children: [
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: AppColors.accent.withValues(alpha: 0.1),
                  borderRadius: AppRadius.smBr,
                ),
                child: Icon(
                  _iconFor(z.plugin),
                  size: 16,
                  color: AppColors.accent,
                ),
              ),
              const SizedBox(width: AppSpacing.sm),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      catalog?.descriptor.name ?? _shortPlugin(z.plugin),
                      style: theme.textTheme.titleSmall?.copyWith(color: cs.onSurface),
                    ),
                    if (catalog?.descriptor.description.isNotEmpty == true)
                      Text(
                        catalog!.descriptor.description,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: cs.onSurfaceVariant,
                          fontSize: 10,
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                  ],
                ),
              ),
            ],
          ),

          if (fields.isEmpty) ...[
            const SizedBox(height: AppSpacing.lg),
            Text(
              'No configuration options for this plugin.',
              style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurfaceVariant),
            ),
          ] else ...[
            const SizedBox(height: AppSpacing.md),
            Divider(height: 1, color: cs.outline),
            const SizedBox(height: AppSpacing.md),
            ...fields
              .where((field) => field.showIf?.isVisible(z.config) ?? true)
              .map((field) => _SchemaField(
                field: field,
                currentValue: z.config[field.key],
                onChanged: (v) => onPatch(z.id, {
                  'plugin_config': {
                    ...z.config,
                    field.key: v,
                  },
                }),
              )),
          ],
        ],
      ),
    );
  }

  static String _shortPlugin(String plugin) {
    if (plugin.contains(':')) return plugin.split(':').last;
    return plugin.split('/').where((s) => s.isNotEmpty).last;
  }

  static IconData _iconFor(String id) {
    if (id.contains('cpu')) return Icons.memory_outlined;
    if (id.contains('gpu')) return Icons.videogame_asset_outlined;
    if (id.contains('weather')) return Icons.wb_sunny_outlined;
    if (id.contains('network')) return Icons.wifi_outlined;
    if (id.contains('clock')) return Icons.access_time_outlined;
    if (id.contains('temp')) return Icons.thermostat_outlined;
    return Icons.extension_outlined;
  }
}

// ── Schema field ──────────────────────────────────────────────────────────────

class _SchemaField extends StatelessWidget {
  const _SchemaField({required this.field, required this.currentValue, required this.onChanged});
  final PluginConfigField field;
  final dynamic currentValue;
  final ValueChanged<dynamic> onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            field.label,
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 3),
          _fieldWidget(context),
          if (field.help != null) ...[
            const SizedBox(height: 2),
            Text(
              field.help!,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                fontSize: 9,
                color: Theme.of(context).colorScheme.onSurfaceVariant.withValues(alpha: 0.55),
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _fieldWidget(BuildContext context) {
    switch (field.type) {
      case 'enum':
        final opts = field.options.map((o) => o.value).toList();
        final optLabels = {for (final o in field.options) o.value: o.label};
        final val = currentValue as String? ?? field.defaultValue as String? ?? (opts.isNotEmpty ? opts.first : '');
        return _EnumField(value: val, options: opts, labels: optLabels, onChanged: onChanged);
      case 'bool':
        final val = currentValue as bool? ?? field.defaultValue as bool? ?? false;
        return SizedBox(
          height: 28,
          child: Align(
            alignment: Alignment.centerLeft,
            child: Switch(value: val, onChanged: onChanged, activeThumbColor: AppColors.accent),
          ),
        );
      case 'int':
        final val = _toInt(currentValue) ?? _toInt(field.defaultValue) ?? 0;
        return _IntField(value: val, min: field.min, max: field.max, onChanged: onChanged);
      default:
        final val = currentValue as String? ?? field.defaultValue as String? ?? '';
        return SizedBox(height: 28, child: _StringFieldStateful(value: val, onChanged: (v) => onChanged(v)));
    }
  }
}

// ── Form field widgets ────────────────────────────────────────────────────────

class _IntField extends StatefulWidget {
  const _IntField({required this.value, required this.onChanged, this.min, this.max});
  final int value;
  final ValueChanged<int> onChanged;
  final int? min;
  final int? max;

  @override
  State<_IntField> createState() => _IntFieldState();
}

class _IntFieldState extends State<_IntField> {
  late final TextEditingController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = TextEditingController(text: widget.value.toString());
  }

  @override
  void didUpdateWidget(_IntField old) {
    super.didUpdateWidget(old);
    if (old.value != widget.value && _ctrl.text != widget.value.toString()) {
      _ctrl.text = widget.value.toString();
    }
  }

  @override
  void dispose() { _ctrl.dispose(); super.dispose(); }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 28,
      child: TextField(
        controller: _ctrl,
        keyboardType: TextInputType.number,
        style: Theme.of(context).textTheme.bodySmall,
        decoration: const InputDecoration(
          isDense: true,
          contentPadding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          border: OutlineInputBorder(),
        ),
        onSubmitted: (v) {
          final n = int.tryParse(v);
          if (n == null) return;
          final clamped = (widget.min != null && n < widget.min!) ? widget.min!
              : (widget.max != null && n > widget.max!) ? widget.max! : n;
          widget.onChanged(clamped);
        },
      ),
    );
  }
}

class _EnumField extends StatelessWidget {
  const _EnumField({
    required this.value,
    required this.options,
    required this.onChanged,
    this.labels = const {},
  });
  final String value;
  final List<String> options;
  final Map<String, String> labels;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final effective = options.contains(value) ? value : options.first;
    return SizedBox(
      height: 28,
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border.all(color: cs.outline),
          borderRadius: BorderRadius.circular(4),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              value: effective,
              isDense: true,
              style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurface),
              dropdownColor: cs.surface,
              items: options.map((o) => DropdownMenuItem(
                value: o,
                child: Text(labels[o] ?? o),
              )).toList(),
              onChanged: (v) { if (v != null) onChanged(v); },
            ),
          ),
        ),
      ),
    );
  }
}

class _StringFieldStateful extends StatefulWidget {
  const _StringFieldStateful({required this.value, required this.onChanged});
  final String value;
  final ValueChanged<String> onChanged;

  @override
  State<_StringFieldStateful> createState() => _StringFieldStatefulState();
}

class _StringFieldStatefulState extends State<_StringFieldStateful> {
  late final TextEditingController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = TextEditingController(text: widget.value);
  }

  @override
  void didUpdateWidget(_StringFieldStateful old) {
    super.didUpdateWidget(old);
    if (old.value != widget.value && _ctrl.text != widget.value) {
      _ctrl.text = widget.value;
    }
  }

  @override
  void dispose() { _ctrl.dispose(); super.dispose(); }

  @override
  Widget build(BuildContext context) => TextField(
    controller: _ctrl,
    style: Theme.of(context).textTheme.bodySmall,
    decoration: const InputDecoration(
      isDense: true,
      contentPadding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      border: OutlineInputBorder(),
    ),
    onSubmitted: widget.onChanged,
  );
}

// ── Narrow layout ─────────────────────────────────────────────────────────────

class _NarrowLayout extends StatefulWidget {
  const _NarrowLayout({
    required this.page,
    required this.pageIndex,
    required this.pageCount,
    required this.catalog,
    required this.selectedZoneId,
    required this.selectedZone,
    required this.catalogFor,
    required this.onSelectZone,
    required this.onAddZone,
    required this.onDeleteZone,
    required this.onPatchZone,
    required this.onReorderZones,
    required this.onNavigate,
  });

  final LayoutPage? page;
  final int pageIndex;
  final int pageCount;
  final List<PluginCatalogEntry> catalog;
  final String? selectedZoneId;
  final LayoutZone? selectedZone;
  final PluginCatalogEntry? Function(String) catalogFor;
  final ValueChanged<String> onSelectZone;
  final void Function(String pluginId, {String? insertBeforeId}) onAddZone;
  final ValueChanged<String> onDeleteZone;
  final void Function(String, Map<String, dynamic>) onPatchZone;
  final ValueChanged<List<String>> onReorderZones;
  final ValueChanged<int> onNavigate;

  @override
  State<_NarrowLayout> createState() => _NarrowLayoutState();
}

class _NarrowLayoutState extends State<_NarrowLayout> with SingleTickerProviderStateMixin {
  late final TabController _tabs;

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 2, vsync: this);
  }

  @override
  void didUpdateWidget(_NarrowLayout old) {
    super.didUpdateWidget(old);
    // Switch to configuration tab when a zone is selected.
    if (widget.selectedZone != null && old.selectedZone == null) {
      _tabs.animateTo(1);
    }
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
        Expanded(
          child: _ZonePage(
            page: widget.page,
            pageIndex: widget.pageIndex,
            pageCount: widget.pageCount,
            selectedZoneId: widget.selectedZoneId,
            onSelect: widget.onSelectZone,
            onDelete: widget.onDeleteZone,
            onDropPlugin: widget.onAddZone,
            onReorder: widget.onReorderZones,
            onNavigate: widget.onNavigate,
            catalogFor: widget.catalogFor,
          ),
        ),
        Container(
          decoration: BoxDecoration(
            border: Border(top: BorderSide(color: cs.outline)),
          ),
          child: TabBar(
            controller: _tabs,
            tabs: const [
              Tab(text: 'Plugins', icon: Icon(Icons.extension_outlined, size: 14)),
              Tab(text: 'Configuration', icon: Icon(Icons.tune_outlined, size: 14)),
            ],
            labelStyle: const TextStyle(fontSize: 11),
            indicatorColor: AppColors.accent,
            labelColor: AppColors.accent,
            unselectedLabelColor: cs.onSurfaceVariant,
          ),
        ),
        SizedBox(
          height: 220,
          child: TabBarView(
            controller: _tabs,
            children: [
              _PluginLibrary(catalog: widget.catalog, onAdd: (id) => widget.onAddZone(id)),
              _Configuration(
                zone: widget.selectedZone,
                catalog: widget.catalogFor(widget.selectedZone?.plugin ?? ''),
                onPatch: widget.onPatchZone,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Helpers ────���──────────────────────────────────────────────────────────────

int? _toInt(dynamic v) {
  if (v == null) return null;
  if (v is num) return v.toInt();
  if (v is String) return int.tryParse(v);
  return null;
}

Future<String?> _promptText(BuildContext context, String title, String hint, String initial) async {
  final ctrl = TextEditingController(text: initial);
  return showDialog<String>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(title),
      content: TextField(controller: ctrl, decoration: InputDecoration(hintText: hint), autofocus: true),
      actions: [
        TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Cancel')),
        TextButton(
          onPressed: () => Navigator.pop(ctx, ctrl.text.trim().isEmpty ? null : ctrl.text.trim()),
          child: const Text('OK'),
        ),
      ],
    ),
  );
}
