import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_typeahead/flutter_typeahead.dart';
import 'package:latlong2/latlong.dart';
import 'package:provider/provider.dart';
import '../../../models/api_models.dart';
import '../../../models/place.dart';
import '../../../services/location_service.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';
import '../../maps/world_map.dart';

part 'editor_helpers.dart';
part 'editor_zone_row.dart';
part 'editor_plugin_library.dart';
part 'editor_configuration.dart';

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
    var s = id.contains(':') ? id.split(':').last : id;
    final parts = s.split('/').where((p) => p.isNotEmpty).toList();
    return parts.isNotEmpty ? parts.last : s;
  }

  // ── Mutations ────────────────────────────────────────────────────────────────

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
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Add zone failed: $e')));
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
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Reorder failed: $e')));
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
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Delete zone failed: $e')));
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
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Update failed: $e')));
      }
    }
  }

  Future<void> _addPage() async {
    final name =
        await _promptText(context, 'New Page', 'Page name', 'Page ${_pages.length + 1}');
    if (name == null) return;
    try {
      await _api.createPage(name, _pages.length);
      await _load();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Create page failed: $e')));
      }
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
        _selectedPageIdx = _selectedPageIdx
            .clamp(0, (_pages.length - 2).clamp(0, _pages.length));
        _selectedZoneId = null;
      });
      await _load();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Delete page failed: $e')));
      }
    }
  }

  // ── Build ─────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Center(
          child: CircularProgressIndicator(color: AppColors.accent));
    }
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(_error!,
                style: const TextStyle(color: AppColors.hardwareAccent)),
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
          onSelect: (i) =>
              setState(() { _selectedPageIdx = i; _selectedZoneId = null; }),
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
                          catalog: _catalog, onAdd: (id) => _addZone(id)),
                    ),
                    const VerticalDivider(width: 1),
                    Expanded(
                      child: _ZonePage(
                        page: _currentPage,
                        pageIndex: _selectedPageIdx,
                        pageCount: _pages.length,
                        selectedZoneId: _selectedZoneId,
                        onSelect: (id) =>
                            setState(() => _selectedZoneId = id),
                        onDelete: _deleteZone,
                        onDropPlugin: _addZone,
                        onReorder: _reorderZones,
                        onNavigate: _navigatePage,
                        catalogFor: _catalogFor,
                        configuration: _Configuration(
                          zone: _selectedZone,
                          catalog: _catalogFor(_selectedZone?.plugin ?? ''),
                          onPatch: _patchZone,
                        ),
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
