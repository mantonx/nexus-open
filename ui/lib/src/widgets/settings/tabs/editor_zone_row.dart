part of 'editor_tab.dart';

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
            child: Icon(icon,
                size: 22,
                color: enabled
                    ? AppColors.accent
                    : cs.onSurfaceVariant.withValues(alpha: 0.2)),
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
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.sm, AppSpacing.md, AppSpacing.sm, AppSpacing.sm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: 4, bottom: AppSpacing.xs),
            child: Text(
              page?.name.toUpperCase() ?? '',
              style: theme.textTheme.labelSmall?.copyWith(
                  color: cs.onSurfaceVariant,
                  letterSpacing: 1.2,
                  fontSize: 9),
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
          // Fill remaining height with a drop target so plugins can be
          // dropped anywhere in the middle column, not just on the zone strip.
          Expanded(
            child: DragTarget<String>(
              onWillAcceptWithDetails: (d) => zones.length < 6,
              onAcceptWithDetails: (d) => onDropPlugin(d.data),
              builder: (ctx, candidates, _) => candidates.isNotEmpty
                  ? Container(
                      margin: const EdgeInsets.only(top: AppSpacing.sm),
                      decoration: BoxDecoration(
                        color: AppColors.accent.withValues(alpha: 0.06),
                        borderRadius: AppRadius.smBr,
                        border: Border.all(
                          color: AppColors.accent.withValues(alpha: 0.4),
                          style: BorderStyle.solid,
                        ),
                      ),
                      child: Center(
                        child: Text(
                          'Drop to add',
                          style: theme.textTheme.labelSmall?.copyWith(
                            fontSize: 10,
                            color: AppColors.accent,
                          ),
                        ),
                      ),
                    )
                  : const SizedBox.expand(),
            ),
          ),
        ],
      ),
    );
  }

  Widget _emptyDropTarget(
      BuildContext context, ColorScheme cs, ThemeData theme) {
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
            color: candidates.isNotEmpty
                ? AppColors.accent
                : cs.outline.withValues(alpha: 0.4),
            width: candidates.isNotEmpty ? 1.5 : 1,
            style: BorderStyle.solid,
          ),
        ),
        child: Center(
          child: Text(
            candidates.isNotEmpty ? 'Drop to add' : 'Drag a plugin here',
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 10,
              color: candidates.isNotEmpty
                  ? AppColors.accent
                  : cs.onSurfaceVariant.withValues(alpha: 0.5),
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
      final insertIdx =
          gap > from ? (gap - 1).clamp(0, ids.length) : gap.clamp(0, ids.length);
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

  Widget _buildStrip(BuildContext context, ColorScheme cs,
      List<LayoutZone> zones, bool dragging) {
    final flexes = zones.map((z) => z.widthPx > 0 ? z.widthPx : 1).toList();
    final avgFlex = flexes.isEmpty
        ? 1
        : (flexes.reduce((a, b) => a + b) / flexes.length).round();

    final children = <Widget>[];
    for (int i = 0; i < zones.length; i++) {
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
          child: Icon(Icons.add,
              size: 14, color: AppColors.accent.withValues(alpha: 0.6)),
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

  static String _readablePluginName(String plugin) {
    final bare = plugin.contains(':') ? plugin.split(':').last : plugin;
    final last = bare.split('/').where((s) => s.isNotEmpty).last;
    return last
        .split('-')
        .map((w) => w.isEmpty ? '' : '${w[0].toUpperCase()}${w.substring(1)}')
        .join(' ');
  }

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
                    Icon(_pluginIcon(zone.plugin), size: 14, color: AppColors.accent),
                    const SizedBox(height: 3),
                    Text(
                      label,
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                            fontSize: 9,
                            color: selected
                                ? AppColors.accent
                                : cs.onSurfaceVariant,
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
            boxShadow: [
              BoxShadow(
                  color: AppColors.accent.withValues(alpha: 0.25),
                  blurRadius: 8)
            ],
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(_pluginIcon(zone.plugin), size: 14, color: AppColors.accent),
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
}
