import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

class PluginsTab extends StatefulWidget {
  const PluginsTab({super.key});

  @override
  State<PluginsTab> createState() => _PluginsTabState();
}

class _PluginsTabState extends State<PluginsTab> {
  late NexusApiService _api;
  final Map<String, Map<String, dynamic>> _configs = {};
  final Map<String, String?> _errors = {};
  final Map<String, Map<String, String>> _statuses = {};
  bool _loading = true;

  static const _knownPlugins = [
    _PluginDef(
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
    _PluginDef(
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
    _PluginDef(
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
    _PluginDef(
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

  bool _initialized = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
    if (!_initialized) {
      _initialized = true;
      _loadAll();
    }
  }

  Future<void> _loadAll() async {
    setState(() => _loading = true);
    for (final plugin in _knownPlugins) {
      try {
        final cfg = await _api.getPluginConfig(plugin.zoneId);
        _configs[plugin.zoneId] = cfg;
        _errors[plugin.zoneId] = null;
      } catch (e) {
        _errors[plugin.zoneId] = e.toString();
      }
      await _refreshStatus(plugin.zoneId);
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
      await _api.updatePluginConfig(zoneId, current);
    } catch (e) {
      if (mounted) setState(() => _errors[zoneId] = e.toString());
    }
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.watch<WsService>();

    if (_loading) {
      return const Center(
        child: CircularProgressIndicator(color: AppColors.accent),
      );
    }

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        _TabHeader(
          title: 'Plugins',
          description: 'Changes apply live — no save needed.',
          trailing: !ws.isConnected
              ? const NexusStatusBadge(status: NexusStatus.warning, label: 'Not connected')
              : null,
        ),
        const SizedBox(height: AppSpacing.md),
        // Two-column layout with intrinsic height — cards grow when expanded
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Column(
                children: [
                  for (final plugin in _knownPlugins.asMap().entries
                      .where((e) => e.key.isEven)
                      .map((e) => e.value))
                    _PluginCard(
                      plugin: plugin,
                      config: _configs[plugin.zoneId] ?? {},
                      error: _errors[plugin.zoneId],
                      zoneStatus: _statuses[plugin.zoneId],
                      enabled: ws.isConnected,
                      onChanged: (k, v) => _updateKey(plugin.zoneId, k, v),
                    ),
                ],
              ),
            ),
            const SizedBox(width: AppSpacing.sm),
            Expanded(
              child: Column(
                children: [
                  for (final plugin in _knownPlugins.asMap().entries
                      .where((e) => e.key.isOdd)
                      .map((e) => e.value))
                    _PluginCard(
                      plugin: plugin,
                      config: _configs[plugin.zoneId] ?? {},
                      error: _errors[plugin.zoneId],
                      zoneStatus: _statuses[plugin.zoneId],
                      enabled: ws.isConnected,
                      onChanged: (k, v) => _updateKey(plugin.zoneId, k, v),
                    ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }
}

// ── Plugin card ───────────────────────────────────────────────────────────────

class _PluginCard extends StatefulWidget {
  const _PluginCard({
    required this.plugin,
    required this.config,
    required this.error,
    required this.enabled,
    required this.onChanged,
    this.zoneStatus,
  });

  final _PluginDef plugin;
  final Map<String, dynamic> config;
  final String? error;
  final Map<String, String>? zoneStatus;
  final bool enabled;
  final void Function(String key, String value) onChanged;

  @override
  State<_PluginCard> createState() => _PluginCardState();
}

class _PluginCardState extends State<_PluginCard> {
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
      onTap: widget.plugin.keys.isNotEmpty
          ? () => setState(() => _expanded = !_expanded)
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header row
          Row(
            children: [
              Icon(widget.plugin.icon,
                  size: AppIconSize.md, color: AppColors.accent),
              const SizedBox(width: AppSpacing.xs),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(widget.plugin.label,
                        style: theme.textTheme.titleSmall),
                    Text(widget.plugin.sublabel,
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
          if (widget.plugin.keys.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.sm),
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
          if (_expanded && widget.plugin.keys.isNotEmpty) ...[
            const Divider(height: AppSpacing.md),
            for (final key in widget.plugin.keys) ...[
              _buildControl(context, key),
              if (key != widget.plugin.keys.last)
                const SizedBox(height: AppSpacing.sm),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildControl(BuildContext context, _ConfigKey key) {
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

// ── Tab page header — consistent with NexusSection label treatment ────────────

class _TabHeader extends StatelessWidget {
  const _TabHeader({
    required this.title,
    required this.description,
    this.trailing,
  });

  final String title;
  final String description;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Container(
          width: 2,
          height: 14,
          margin: const EdgeInsets.only(right: AppSpacing.sm),
          decoration: BoxDecoration(
            color: AppColors.accent,
            borderRadius: AppRadius.pillBr,
          ),
        ),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title.toUpperCase(),
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                  letterSpacing: 1.1,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 2),
              Text(description, style: theme.textTheme.bodySmall),
            ],
          ),
        ),
        if (trailing != null) ...[
          const SizedBox(width: AppSpacing.sm),
          trailing!,
        ],
      ],
    );
  }
}

class _PluginDef {
  final String zoneId;
  final String label;
  final String sublabel;
  final IconData icon;
  final List<_ConfigKey> keys;
  const _PluginDef({
    required this.zoneId,
    required this.label,
    required this.sublabel,
    required this.icon,
    required this.keys,
  });
}
