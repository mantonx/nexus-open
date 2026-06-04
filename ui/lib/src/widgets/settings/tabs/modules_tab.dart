import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

class ModulesTab extends StatefulWidget {
  const ModulesTab({super.key});

  @override
  State<ModulesTab> createState() => _ModulesTabState();
}

class _ModulesTabState extends State<ModulesTab> {
  final _api = NexusApiService();
  final Map<String, Map<String, dynamic>> _configs = {};
  final Map<String, String?> _errors = {};
  final Map<String, Map<String, String>> _statuses = {};
  bool _loading = true;

  static const _knownModules = [
    _ModuleDef(
      zoneId: 'system.cpu',
      label: 'CPU',
      sublabel: 'Temperature',
      icon: Icons.memory,
      keys: [
        _ConfigKey('unit', 'Unit', _ControlType.dropdown,
            options: ['metric', 'imperial']),
        _ConfigKey('graph_type', 'Graph', _ControlType.dropdown,
            options: ['sparkline', 'bar', 'area']),
      ],
    ),
    _ModuleDef(
      zoneId: 'system.gpu',
      label: 'GPU',
      sublabel: 'Temperature',
      icon: Icons.videogame_asset,
      keys: [
        _ConfigKey('unit', 'Unit', _ControlType.dropdown,
            options: ['metric', 'imperial']),
        _ConfigKey('graph_type', 'Graph', _ControlType.dropdown,
            options: ['sparkline', 'bar', 'area', 'line']),
      ],
    ),
    _ModuleDef(
      zoneId: 'system.weather',
      label: 'Weather',
      sublabel: 'Conditions',
      icon: Icons.cloud,
      keys: [
        _ConfigKey('location', 'Location', _ControlType.text),
        _ConfigKey('unit', 'Unit', _ControlType.dropdown,
            options: ['metric', 'imperial']),
      ],
    ),
    _ModuleDef(
      zoneId: 'system.network',
      label: 'Network',
      sublabel: 'Throughput',
      icon: Icons.network_check,
      keys: [
        _ConfigKey('network_format', 'Format', _ControlType.dropdown,
            options: ['bytes', 'bits']),
        _ConfigKey('graph_type', 'Graph', _ControlType.dropdown,
            options: ['sparkline', 'bar', 'area']),
      ],
    ),
  ];

  @override
  void initState() {
    super.initState();
    _loadAll();
  }

  Future<void> _loadAll() async {
    setState(() => _loading = true);
    for (final mod in _knownModules) {
      try {
        final cfg = await _api.getModuleConfig(mod.zoneId);
        _configs[mod.zoneId] = cfg;
        _errors[mod.zoneId] = null;
      } catch (e) {
        _errors[mod.zoneId] = e.toString();
      }
      await _refreshStatus(mod.zoneId);
    }
    if (mounted) setState(() => _loading = false);
  }

  Future<void> _refreshStatus(String zoneId) async {
    try {
      final st = await _api.getZoneStatus(zoneId);
      if (mounted) setState(() => _statuses[zoneId] = st);
    } catch (_) {}
  }

  Future<void> _updateKey(String zoneId, String key, String value) async {
    final current = Map<String, dynamic>.from(_configs[zoneId] ?? {});
    current[key] = value;
    setState(() {
      _configs[zoneId] = current;
      _errors[zoneId] = null;
    });
    try {
      await _api.updateModuleConfig(zoneId, current);
    } catch (e) {
      if (mounted) setState(() => _errors[zoneId] = e.toString());
    }
  }

  @override
  void dispose() {
    _api.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.watch<WsService>();
    final theme = Theme.of(context);

    if (_loading) {
      return Center(
        child: CircularProgressIndicator(color: AppColors.accent),
      );
    }

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Modules', style: theme.textTheme.headlineSmall),
                  Text('Changes apply live — no save needed.',
                      style: theme.textTheme.bodySmall),
                ],
              ),
            ),
            if (!ws.isConnected)
              NexusStatusBadge(
                  status: NexusStatus.warning, label: 'Not connected'),
          ],
        ),
        const SizedBox(height: AppSpacing.md),
        // 2×2 grid of module cards
        GridView.count(
          crossAxisCount: 2,
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          crossAxisSpacing: AppSpacing.sm,
          mainAxisSpacing: AppSpacing.sm,
          childAspectRatio: 1.6,
          children: [
            for (final mod in _knownModules)
              _ModuleCard(
                mod: mod,
                config: _configs[mod.zoneId] ?? {},
                error: _errors[mod.zoneId],
                zoneStatus: _statuses[mod.zoneId],
                enabled: ws.isConnected,
                onChanged: (k, v) => _updateKey(mod.zoneId, k, v),
              ),
          ],
        ),
      ],
    );
  }
}

// ── Module card ───────────────────────────────────────────────────────────────

class _ModuleCard extends StatefulWidget {
  const _ModuleCard({
    required this.mod,
    required this.config,
    required this.error,
    required this.enabled,
    required this.onChanged,
    this.zoneStatus,
  });

  final _ModuleDef mod;
  final Map<String, dynamic> config;
  final String? error;
  final Map<String, String>? zoneStatus;
  final bool enabled;
  final void Function(String key, String value) onChanged;

  @override
  State<_ModuleCard> createState() => _ModuleCardState();
}

class _ModuleCardState extends State<_ModuleCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final statusStr = widget.zoneStatus?['status'] ?? 'loading';
    final statusErrMsg = widget.zoneStatus?['error'] ?? '';
    final nexusStatus = statusStr.toNexusStatus();
    final hasError =
        nexusStatus == NexusStatus.error || nexusStatus == NexusStatus.warning;

    return NexusCard(
      padding: AppSpacing.paddingSm,
      accentBorder: hasError || widget.error != null,
      accentColor: hasError
          ? theme.colorScheme.warning
          : widget.error != null
              ? theme.colorScheme.critical
              : null,
      onTap: widget.mod.keys.isNotEmpty
          ? () => setState(() => _expanded = !_expanded)
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header row
          Row(
            children: [
              Icon(widget.mod.icon,
                  size: AppIconSize.md, color: AppColors.accent),
              const SizedBox(width: AppSpacing.xs),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(widget.mod.label,
                        style: theme.textTheme.titleSmall),
                    Text(widget.mod.sublabel,
                        style: theme.textTheme.labelSmall),
                  ],
                ),
              ),
              NexusStatusBadge.dot(status: nexusStatus),
            ],
          ),

          // Error message
          if (widget.error != null) ...[
            const SizedBox(height: AppSpacing.xs),
            Text(widget.error!,
                style: theme.textTheme.labelSmall
                    ?.copyWith(color: theme.colorScheme.critical),
                maxLines: 1,
                overflow: TextOverflow.ellipsis),
          ] else if (hasError && statusErrMsg.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.xs),
            Text(statusErrMsg,
                style: theme.textTheme.labelSmall
                    ?.copyWith(color: theme.colorScheme.warning),
                maxLines: 1,
                overflow: TextOverflow.ellipsis),
          ],

          // Expand indicator
          if (widget.mod.keys.isNotEmpty) ...[
            const Spacer(),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Text(
                  _expanded ? 'Collapse' : 'Configure',
                  style: theme.textTheme.labelSmall
                      ?.copyWith(color: AppColors.accent),
                ),
                Icon(
                  _expanded
                      ? Icons.keyboard_arrow_up
                      : Icons.keyboard_arrow_down,
                  size: AppIconSize.sm,
                  color: AppColors.accent,
                ),
              ],
            ),
          ],

          // Controls (shown when expanded)
          if (_expanded && widget.mod.keys.isNotEmpty) ...[
            const Divider(height: AppSpacing.md),
            for (final key in widget.mod.keys) ...[
              _buildControl(context, key),
              if (key != widget.mod.keys.last)
                const SizedBox(height: AppSpacing.sm),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildControl(BuildContext context, _ConfigKey key) {
    final theme = Theme.of(context);
    final current = widget.config[key.id]?.toString() ?? '';

    if (key.type == _ControlType.dropdown && key.options != null) {
      final value =
          key.options!.contains(current) ? current : key.options!.first;
      return NexusDropdownField<String>(
        label: key.label,
        value: value,
        enabled: widget.enabled,
        items: key.options!
            .map((o) => DropdownMenuItem(value: o, child: Text(o)))
            .toList(),
        onChanged: (v) {
          if (v != null) widget.onChanged(key.id, v);
        },
      );
    }

    return NexusFormField(
      label: key.label,
      initialValue: current,
      enabled: widget.enabled,
      onSubmitted: (v) => widget.onChanged(key.id, v),
    );
  }
}

// ── Data model helpers ────────────────────────────────────────────────────────

enum _ControlType { dropdown, text }

class _ConfigKey {
  final String id;
  final String label;
  final _ControlType type;
  final List<String>? options;
  const _ConfigKey(this.id, this.label, this.type, {this.options});
}

class _ModuleDef {
  final String zoneId;
  final String label;
  final String sublabel;
  final IconData icon;
  final List<_ConfigKey> keys;
  const _ModuleDef({
    required this.zoneId,
    required this.label,
    required this.sublabel,
    required this.icon,
    required this.keys,
  });
}
