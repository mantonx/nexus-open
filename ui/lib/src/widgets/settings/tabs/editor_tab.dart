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
  List<LayoutPage> _pages = [];
  List<PluginCatalogEntry> _catalog = [];
  int _selectedPageIdx = 0;
  String? _selectedZoneId;
  bool _loading = true;
  String? _error;
  StreamSubscription? _wsSub;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _wsSub?.cancel();
    _wsSub = context.read<WsService>().events.listen((event) {
      if (!mounted) return;
      // On draft_state(active=false), reload from committed store.
      if (event is WsDraftStateEvent && !event.active) {
        _load(useDraft: false);
      }
    });
  }

  @override
  void dispose() {
    _wsSub?.cancel();
    super.dispose();
  }

  Future<void> _load({bool useDraft = true}) async {
    setState(() { _loading = true; _error = null; });
    final api = NexusApiService();
    try {
      final results = await Future.wait([
        useDraft ? api.getDraft() : api.getLayout(),
        api.getPluginCatalog(),
      ]);
      if (!mounted) return;
      setState(() {
        _pages = results[0] as List<LayoutPage>;
        _catalog = results[1] as List<PluginCatalogEntry>;
        if (_selectedPageIdx >= _pages.length) _selectedPageIdx = 0;
        _loading = false;
      });
    } catch (e) {
      if (mounted) setState(() { _loading = false; _error = e.toString(); });
    } finally {
      api.dispose();
    }
  }

  LayoutPage? get _currentPage =>
      _pages.isNotEmpty ? _pages[_selectedPageIdx] : null;

  LayoutZone? get _selectedZone => _selectedZoneId == null
      ? null
      : _currentPage?.zones
          .cast<LayoutZone?>()
          .firstWhere((z) => z?.id == _selectedZoneId, orElse: () => null);

  PluginCatalogEntry? _catalogFor(String pluginId) =>
      _catalog.cast<PluginCatalogEntry?>().firstWhere(
        (e) => e?.id == pluginId || e?.descriptor.id == pluginId,
        orElse: () => null,
      );

  // ── Mutations ───────────────────────────────────────────────────────────────

  Future<void> _addZone(String pluginId) async {
    final pageIdx = _selectedPageIdx;
    final api = NexusApiService();
    try {
      final newId = await api.addDraftZone(pageIndex: pageIdx, plugin: pluginId);
      final pages = await api.getDraft();
      if (!mounted) return;
      setState(() {
        _pages = pages;
        _selectedZoneId = newId;
        _markDraftActive();
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Add zone failed: $e')));
      }
    } finally {
      api.dispose();
    }
  }

  Future<void> _deleteZone(String zoneId) async {
    final api = NexusApiService();
    try {
      await api.deleteDraftZone(zoneId);
      final pages = await api.getDraft();
      if (!mounted) return;
      setState(() {
        _pages = pages;
        if (_selectedZoneId == zoneId) _selectedZoneId = null;
        _markDraftActive();
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete zone failed: $e')));
      }
    } finally {
      api.dispose();
    }
  }

  Future<void> _patchZone(String zoneId, Map<String, dynamic> patch) async {
    final api = NexusApiService();
    try {
      await api.patchDraftZone(zoneId, patch);
      final pages = await api.getDraft();
      if (!mounted) return;
      setState(() {
        _pages = pages;
        _markDraftActive();
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Update failed: $e')));
      }
    } finally {
      api.dispose();
    }
  }

  Future<void> _addPage() async {
    final name = await _promptText(context, 'New Page', 'Page name', 'Page ${_pages.length + 1}');
    if (name == null) return;
    final api = NexusApiService();
    try {
      await api.createPage(name, _pages.length);
      await _load();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Create page failed: $e')));
    } finally {
      api.dispose();
    }
  }

  Future<void> _deletePage(int pageId) async {
    final api = NexusApiService();
    try {
      await api.deletePage(pageId);
      await _load();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete page failed: $e')));
    } finally {
      api.dispose();
    }
  }

  void _markDraftActive() {
    // The backend WS will send draft_state{active:true} — settings_page picks it up.
    // No local flag needed; the bar appears via the WS event.
  }

  // ── Build ────────────────────────────────────────────────────────────────────

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
        // Page strip
        _PageStrip(
          pages: _pages,
          selectedIdx: _selectedPageIdx,
          onSelect: (i) => setState(() { _selectedPageIdx = i; _selectedZoneId = null; }),
          onAdd: _addPage,
          onDelete: _deletePage,
        ),

        // Main 3-column (or 1-column on narrow)
        Expanded(
          child: isWide
              ? Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Plugin library
                    SizedBox(
                      width: 200,
                      child: _PluginLibrary(
                        catalog: _catalog,
                        onAdd: _addZone,
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    // Zone canvas
                    Expanded(
                      child: _ZoneCanvas(
                        page: _currentPage,
                        selectedZoneId: _selectedZoneId,
                        onSelect: (id) => setState(() => _selectedZoneId = id),
                        onDelete: _deleteZone,
                        catalogFor: _catalogFor,
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    // Inspector
                    SizedBox(
                      width: 260,
                      child: _Inspector(
                        zone: _selectedZone,
                        catalog: _catalogFor(_selectedZone?.plugin ?? ''),
                        onPatch: _patchZone,
                      ),
                    ),
                  ],
                )
              : _NarrowLayout(
                  page: _currentPage,
                  catalog: _catalog,
                  selectedZoneId: _selectedZoneId,
                  selectedZone: _selectedZone,
                  catalogFor: _catalogFor,
                  onSelectZone: (id) => setState(() => _selectedZoneId = id),
                  onAddZone: _addZone,
                  onDeleteZone: _deleteZone,
                  onPatchZone: _patchZone,
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
                              onTap: () => onDelete(pages[i].id),
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

// ── Zone canvas ───────────────────────────────────────────────────────────────

class _ZoneCanvas extends StatelessWidget {
  const _ZoneCanvas({
    required this.page,
    required this.selectedZoneId,
    required this.onSelect,
    required this.onDelete,
    required this.catalogFor,
  });

  final LayoutPage? page;
  final String? selectedZoneId;
  final ValueChanged<String> onSelect;
  final ValueChanged<String> onDelete;
  final PluginCatalogEntry? Function(String) catalogFor;

  @override
  Widget build(BuildContext context) {
    if (page == null || page!.zones.isEmpty) {
      return Center(
        child: Text(
          'No zones — add a plugin from the library',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Canvas — ${page!.name}',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
              letterSpacing: 1,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          // Proportional zone row — fills width matching device proportions.
          LayoutBuilder(
            builder: (ctx, constraints) {
              final total = page!.totalWidth;
              if (total == 0) return const SizedBox.shrink();
              return SizedBox(
                height: 56,
                child: Row(
                  children: page!.zones.map((zone) {
                    final flex = zone.widthPx;
                    final selected = zone.id == selectedZoneId;
                    final entry = catalogFor(zone.plugin);
                    return Expanded(
                      flex: flex,
                      child: Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 1),
                        child: GestureDetector(
                          onTap: () => onSelect(zone.id),
                          child: AnimatedContainer(
                            duration: AppDuration.fast,
                            decoration: BoxDecoration(
                              color: selected
                                  ? AppColors.accent.withValues(alpha: 0.15)
                                  : Theme.of(ctx).colorScheme.surfaceContainerHigh,
                              borderRadius: AppRadius.smBr,
                              border: Border.all(
                                color: selected ? AppColors.accent : Theme.of(ctx).colorScheme.outline,
                                width: selected ? 1.5 : 1,
                              ),
                            ),
                            child: Stack(
                              children: [
                                Center(
                                  child: Padding(
                                    padding: const EdgeInsets.all(4),
                                    child: Column(
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        Text(
                                          entry?.descriptor.name ?? _shortPlugin(zone.plugin),
                                          style: Theme.of(ctx).textTheme.labelSmall?.copyWith(
                                            fontSize: 9,
                                            color: selected
                                                ? AppColors.accent
                                                : Theme.of(ctx).colorScheme.onSurfaceVariant,
                                          ),
                                          overflow: TextOverflow.ellipsis,
                                          maxLines: 1,
                                          textAlign: TextAlign.center,
                                        ),
                                        Text(
                                          '${zone.widthPx}px',
                                          style: Theme.of(ctx).textTheme.labelSmall?.copyWith(
                                            fontSize: 8,
                                            color: Theme.of(ctx).colorScheme.onSurfaceVariant.withValues(alpha: 0.6),
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                ),
                                // Delete button — top-right
                                Positioned(
                                  top: 2,
                                  right: 2,
                                  child: GestureDetector(
                                    onTap: () => onDelete(zone.id),
                                    child: Container(
                                      padding: const EdgeInsets.all(2),
                                      decoration: BoxDecoration(
                                        color: Theme.of(ctx).colorScheme.surfaceContainerHighest,
                                        shape: BoxShape.circle,
                                      ),
                                      child: Icon(Icons.close, size: 9,
                                          color: Theme.of(ctx).colorScheme.onSurfaceVariant),
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    );
                  }).toList(),
                ),
              );
            },
          ),
          const SizedBox(height: AppSpacing.sm),
          // Width breakdown label
          Wrap(
            spacing: 8,
            children: page!.zones.map((z) => Text(
              '${_shortPlugin(z.plugin)} ${z.widthPx}px',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                fontSize: 9,
                color: Theme.of(context).colorScheme.onSurfaceVariant.withValues(alpha: 0.5),
              ),
            )).toList(),
          ),
        ],
      ),
    );
  }

  static String _shortPlugin(String plugin) {
    final parts = plugin.split(':');
    return parts.last;
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
          child: ListView.builder(
            padding: const EdgeInsets.only(bottom: AppSpacing.md),
            itemCount: catalog.length,
            itemBuilder: (ctx, i) {
              final entry = catalog[i];
              return Tooltip(
                message: entry.descriptor.description,
                child: InkWell(
                  onTap: () => onAdd(entry.id),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: AppSpacing.md,
                      vertical: AppSpacing.xs + 2,
                    ),
                    child: Row(
                      children: [
                        Icon(
                          _iconFor(entry.id),
                          size: AppIconSize.sm,
                          color: AppColors.accent.withValues(alpha: 0.8),
                        ),
                        const SizedBox(width: AppSpacing.sm),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                entry.descriptor.name.isNotEmpty
                                    ? entry.descriptor.name
                                    : entry.id,
                                style: theme.textTheme.bodySmall?.copyWith(
                                  color: cs.onSurface,
                                ),
                                overflow: TextOverflow.ellipsis,
                              ),
                              Text(
                                entry.kind,
                                style: theme.textTheme.labelSmall?.copyWith(
                                  fontSize: 9,
                                  color: cs.onSurfaceVariant.withValues(alpha: 0.6),
                                ),
                              ),
                            ],
                          ),
                        ),
                        Icon(Icons.add, size: 14, color: cs.onSurfaceVariant.withValues(alpha: 0.4)),
                      ],
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ],
    );
  }

  static IconData _iconFor(String id) {
    if (id.contains('cpu')) return Icons.memory_outlined;
    if (id.contains('gpu')) return Icons.videogame_asset_outlined;
    if (id.contains('weather')) return Icons.wb_sunny_outlined;
    if (id.contains('network')) return Icons.wifi_outlined;
    if (id.contains('clock')) return Icons.access_time_outlined;
    return Icons.extension_outlined;
  }
}

// ── Inspector ─────────────────────────────────────────────────────────────────

class _Inspector extends StatelessWidget {
  const _Inspector({
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
        child: Text(
          'Select a zone to inspect',
          style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurfaceVariant),
          textAlign: TextAlign.center,
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
            'INSPECTOR',
            style: theme.textTheme.labelSmall?.copyWith(
              color: cs.onSurfaceVariant,
              letterSpacing: 1.5,
              fontSize: 9,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Text(
            catalog?.descriptor.name ?? z.plugin,
            style: theme.textTheme.titleSmall?.copyWith(color: cs.onSurface),
          ),
          if (catalog?.descriptor.description.isNotEmpty == true) ...[
            const SizedBox(height: 2),
            Text(
              catalog!.descriptor.description,
              style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurfaceVariant),
            ),
          ],
          const SizedBox(height: AppSpacing.md),

          // Refresh interval
          _InspectorRow(
            label: 'Refresh',
            child: _IntField(
              value: z.refreshMs,
              min: 100,
              max: 60000,
              suffix: 'ms',
              onChanged: (v) => onPatch(z.id, {'refresh_ms': v}),
            ),
          ),

          // Alignment
          _InspectorRow(
            label: 'Align',
            child: _EnumField(
              value: z.align,
              options: const ['left', 'center', 'right'],
              onChanged: (v) => onPatch(z.id, {'align': v}),
            ),
          ),

          if (fields.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.sm),
            Divider(height: 1, color: cs.outline),
            const SizedBox(height: AppSpacing.sm),
            Text(
              'Plugin config',
              style: theme.textTheme.labelSmall?.copyWith(
                color: cs.onSurfaceVariant,
                fontSize: 9,
                letterSpacing: 1,
              ),
            ),
            const SizedBox(height: AppSpacing.sm),
            ...fields.map((field) => _SchemaField(
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
}

// ── Inspector helpers ─────────────────────────────────────────────────────────

class _InspectorRow extends StatelessWidget {
  const _InspectorRow({required this.label, required this.child});
  final String label;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
      child: Row(
        children: [
          SizedBox(
            width: 72,
            child: Text(
              label,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          Expanded(child: child),
        ],
      ),
    );
  }
}

class _IntField extends StatefulWidget {
  const _IntField({required this.value, required this.onChanged, this.min, this.max, this.suffix});
  final int value;
  final ValueChanged<int> onChanged;
  final int? min;
  final int? max;
  final String? suffix;

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
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 28,
      child: TextField(
        controller: _ctrl,
        keyboardType: TextInputType.number,
        style: Theme.of(context).textTheme.bodySmall,
        decoration: InputDecoration(
          isDense: true,
          contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          border: const OutlineInputBorder(),
          suffix: widget.suffix != null ? Text(widget.suffix!, style: Theme.of(context).textTheme.labelSmall) : null,
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
  const _EnumField({required this.value, required this.options, required this.onChanged});
  final String value;
  final List<String> options;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final effective = options.contains(value) ? value : options.first;
    return SizedBox(
      height: 28,
      child: DropdownButtonFormField<String>(
        value: effective,
        isDense: true,
        decoration: const InputDecoration(
          isDense: true,
          contentPadding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          border: OutlineInputBorder(),
        ),
        style: Theme.of(context).textTheme.bodySmall,
        items: options
            .map((o) => DropdownMenuItem(value: o, child: Text(o)))
            .toList(),
        onChanged: (v) { if (v != null) onChanged(v); },
      ),
    );
  }
}

/// Renders a single schema-declared config field.
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
        final val = currentValue as String? ?? field.defaultValue as String? ?? (opts.isNotEmpty ? opts.first : '');
        return _EnumField(
          value: val,
          options: opts,
          onChanged: onChanged,
        );
      case 'bool':
        final val = currentValue as bool? ?? field.defaultValue as bool? ?? false;
        return SizedBox(
          height: 28,
          child: Align(
            alignment: Alignment.centerLeft,
            child: Switch(
              value: val,
              onChanged: onChanged,
              activeThumbColor: AppColors.accent,
            ),
          ),
        );
      case 'int':
        final val = _toInt(currentValue) ?? _toInt(field.defaultValue) ?? 0;
        return _IntField(
          value: val,
          min: field.min,
          max: field.max,
          onChanged: onChanged,
        );
      default: // string, color
        final val = currentValue as String? ?? field.defaultValue as String? ?? '';
        return SizedBox(
          height: 28,
          child: _StringFieldStateful(value: val, onChanged: (v) => onChanged(v)),
        );
    }
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

class _NarrowLayout extends StatelessWidget {
  const _NarrowLayout({
    required this.page,
    required this.catalog,
    required this.selectedZoneId,
    required this.selectedZone,
    required this.catalogFor,
    required this.onSelectZone,
    required this.onAddZone,
    required this.onDeleteZone,
    required this.onPatchZone,
  });

  final LayoutPage? page;
  final List<PluginCatalogEntry> catalog;
  final String? selectedZoneId;
  final LayoutZone? selectedZone;
  final PluginCatalogEntry? Function(String) catalogFor;
  final ValueChanged<String> onSelectZone;
  final ValueChanged<String> onAddZone;
  final ValueChanged<String> onDeleteZone;
  final void Function(String, Map<String, dynamic>) onPatchZone;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // Zone canvas fills available space
        Expanded(
          child: _ZoneCanvas(
            page: page,
            selectedZoneId: selectedZoneId,
            onSelect: onSelectZone,
            onDelete: onDeleteZone,
            catalogFor: catalogFor,
          ),
        ),
        // Inspector as bottom panel when zone selected, else plugin library
        if (selectedZone != null)
          SizedBox(
            height: 280,
            child: Container(
              decoration: BoxDecoration(
                border: Border(top: BorderSide(color: Theme.of(context).colorScheme.outline)),
              ),
              child: _Inspector(
                zone: selectedZone,
                catalog: catalogFor(selectedZone!.plugin),
                onPatch: onPatchZone,
              ),
            ),
          )
        else
          SizedBox(
            height: 180,
            child: Container(
              decoration: BoxDecoration(
                border: Border(top: BorderSide(color: Theme.of(context).colorScheme.outline)),
              ),
              child: _PluginLibrary(catalog: catalog, onAdd: onAddZone),
            ),
          ),
      ],
    );
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Coerces a JSON value to int — handles num, String, and null without casting.
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
