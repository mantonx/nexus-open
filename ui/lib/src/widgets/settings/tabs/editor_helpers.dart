part of 'editor_tab.dart';

// ── Plugin icon ───────────────────────────────────────────────────────────────

IconData _pluginIcon(String id) {
  if (id.contains('cpu')) return Icons.memory_outlined;
  if (id.contains('gpu')) return Icons.videogame_asset_outlined;
  if (id.contains('weather')) return Icons.wb_sunny_outlined;
  if (id.contains('network')) return Icons.wifi_outlined;
  if (id.contains('clock')) return Icons.access_time_outlined;
  if (id.contains('temp')) return Icons.thermostat_outlined;
  return Icons.extension_outlined;
}

// ── Prompt dialog ─────────────────────────────────────────────────────────────

Future<String?> _promptText(
    BuildContext context, String title, String hint, String initial) async {
  final ctrl = TextEditingController(text: initial);
  return showDialog<String>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(title),
      content: TextField(
          controller: ctrl,
          decoration: InputDecoration(hintText: hint),
          autofocus: true),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(ctx), child: const Text('Cancel')),
        TextButton(
          onPressed: () =>
              Navigator.pop(ctx, ctrl.text.trim().isEmpty ? null : ctrl.text.trim()),
          child: const Text('OK'),
        ),
      ],
    ),
  );
}

int? _toInt(dynamic v) {
  if (v == null) return null;
  if (v is num) return v.toInt();
  if (v is String) return int.tryParse(v);
  return null;
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
              padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.sm, vertical: 6),
              itemCount: pages.length,
              itemBuilder: (ctx, i) {
                final selected = i == selectedIdx;
                return Padding(
                  padding: const EdgeInsets.only(right: 4),
                  child: GestureDetector(
                    onTap: () => onSelect(i),
                    child: AnimatedContainer(
                      duration: AppDuration.fast,
                      padding: const EdgeInsets.symmetric(
                          horizontal: 10, vertical: 2),
                      decoration: BoxDecoration(
                        color: selected
                            ? AppColors.accent.withValues(alpha: 0.12)
                            : Colors.transparent,
                        borderRadius: AppRadius.smBr,
                        border: Border.all(
                          color:
                              selected ? AppColors.accent : cs.outline,
                          width: selected ? 1.5 : 1,
                        ),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            pages[i].name,
                            style: theme.textTheme.labelSmall?.copyWith(
                              color: selected
                                  ? AppColors.accent
                                  : cs.onSurfaceVariant,
                              fontWeight: selected
                                  ? FontWeight.w600
                                  : FontWeight.normal,
                            ),
                          ),
                          if (pages.length > 1) ...[
                            const SizedBox(width: 4),
                            InkWell(
                              onTap: () => onDelete(i),
                              borderRadius: BorderRadius.circular(10),
                              child: Icon(Icons.close,
                                  size: 12,
                                  color: cs.onSurfaceVariant
                                      .withValues(alpha: 0.6)),
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
                  child: Icon(Icons.add,
                      size: AppIconSize.sm, color: AppColors.accent),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
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

class _NarrowLayoutState extends State<_NarrowLayout>
    with SingleTickerProviderStateMixin {
  late final TabController _tabs;

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 2, vsync: this);
  }

  @override
  void didUpdateWidget(_NarrowLayout old) {
    super.didUpdateWidget(old);
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
          decoration:
              BoxDecoration(border: Border(top: BorderSide(color: cs.outline))),
          child: TabBar(
            controller: _tabs,
            tabs: const [
              Tab(text: 'Plugins', icon: Icon(Icons.extension_outlined, size: 14)),
              Tab(
                  text: 'Configuration',
                  icon: Icon(Icons.tune_outlined, size: 14)),
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
              _PluginLibrary(
                  catalog: widget.catalog,
                  onAdd: (id) => widget.onAddZone(id)),
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
