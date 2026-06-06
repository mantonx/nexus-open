import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../models/settings_state.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

class DisplayTab extends StatefulWidget {
  const DisplayTab({super.key});

  @override
  State<DisplayTab> createState() => _DisplayTabState();
}

class _DisplayTabState extends State<DisplayTab> {
  double _brightness = 75;
  final _api = NexusApiService();

  Future<void> _setBrightness(double value) async {
    try {
      await _api.setBrightness(value.round());
    } catch (_) {}
  }

  @override
  void dispose() {
    _api.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final ws = context.watch<WsService>();
    final cs = Theme.of(context).colorScheme;
    final theme = Theme.of(context);
    final connected = ws.isConnected;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        // ── Brightness ──────────────────────────────────────────────────
        NexusSection(
          title: 'Brightness',
          description: 'Physical display brightness (0–100).',
          trailing: connected
              ? null
              : const NexusStatusBadge(
                  status: NexusStatus.warning, label: 'Device required'),
          child: Row(
            children: [
              Icon(Icons.brightness_low,
                  size: AppIconSize.md,
                  color: connected ? cs.onSurface : cs.onSurfaceVariant),
              Expanded(
                child: Slider(
                  value: _brightness,
                  min: 0,
                  max: 100,
                  divisions: 20,
                  label: '${_brightness.round()}%',
                  onChanged: connected
                      ? (v) => setState(() => _brightness = v)
                      : null,
                  onChangeEnd: connected ? _setBrightness : null,
                ),
              ),
              Icon(Icons.brightness_high,
                  size: AppIconSize.md,
                  color: connected ? cs.onSurface : cs.onSurfaceVariant),
              const SizedBox(width: AppSpacing.sm),
              SizedBox(
                width: 40,
                child: Text(
                  '${_brightness.round()}%',
                  style: theme.textTheme.labelLarge,
                  textAlign: TextAlign.end,
                ),
              ),
            ],
          ),
        ),

        const SizedBox(height: AppSpacing.sm),

        // ── Units ───────────────────────────────────────────────────────
        NexusSection(
          title: 'Units',
          description: 'Measurement units for temperature and distance.',
          child: Column(
            children: [
              _SettingRow(
                label: 'Temperature',
                tooltip: 'Celsius (°C) or Fahrenheit (°F)',
                child: NexusDropdownField<String>(
                  value: settings.temperatureUnit,
                  items: const [
                    DropdownMenuItem(value: 'Celsius',    child: Text('°C — Celsius')),
                    DropdownMenuItem(value: 'Fahrenheit', child: Text('°F — Fahrenheit')),
                  ],
                  onChanged: (v) => settings.setTemperatureUnit(v!),
                ),
              ),
              const SizedBox(height: AppSpacing.sm),
              _SettingRow(
                label: 'Distance',
                tooltip: 'Kilometers or Miles',
                child: NexusDropdownField<String>(
                  value: settings.distanceUnit,
                  items: const [
                    DropdownMenuItem(value: 'Kilometers', child: Text('km — Kilometers')),
                    DropdownMenuItem(value: 'Miles',      child: Text('mi — Miles')),
                  ],
                  onChanged: (v) => settings.setDistanceUnit(v!),
                ),
              ),
            ],
          ),
        ),

        const SizedBox(height: AppSpacing.sm),

        // ── Date & time format ──────────────────────────────────────────
        NexusSection(
          title: 'Date & Time',
          description: 'How dates and times appear on the display.',
          child: Column(
            children: [
              _SettingRow(
                label: 'Time format',
                tooltip: '24-hour or 12-hour AM/PM',
                child: NexusDropdownField<String>(
                  value: settings.timeFormat,
                  items: const [
                    DropdownMenuItem(value: '24h', child: Text('24h (14:30)')),
                    DropdownMenuItem(value: '12h', child: Text('12h (2:30 PM)')),
                  ],
                  onChanged: (v) => settings.setTimeFormat(v!),
                ),
              ),
              const SizedBox(height: AppSpacing.sm),
              _SettingRow(
                label: 'Date format',
                tooltip: 'YYYY-MM-DD · DD/MM/YYYY · MM/DD/YYYY',
                child: NexusDropdownField<String>(
                  value: settings.dateFormat,
                  items: const [
                    DropdownMenuItem(value: 'YYYY-MM-DD', child: Text('YYYY-MM-DD')),
                    DropdownMenuItem(value: 'DD/MM/YYYY', child: Text('DD/MM/YYYY')),
                    DropdownMenuItem(value: 'MM/DD/YYYY', child: Text('MM/DD/YYYY')),
                  ],
                  onChanged: (v) => settings.setDateFormat(v!),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Setting row ───────────────────────────────────────────────────────────────

class _SettingRow extends StatelessWidget {
  const _SettingRow({
    required this.label,
    required this.child,
    this.tooltip,
  });

  final String label;
  final Widget child;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Expanded(
          flex: 2,
          child: Tooltip(
            message: tooltip ?? '',
            child: Row(
              children: [
                Text(label, style: theme.textTheme.bodyLarge),
                if (tooltip != null) ...[
                  const SizedBox(width: AppSpacing.xs),
                  Icon(Icons.help_outline,
                      size: AppIconSize.sm,
                      color: theme.colorScheme.onSurfaceVariant),
                ],
              ],
            ),
          ),
        ),
        Expanded(flex: 3, child: child),
      ],
    );
  }
}
