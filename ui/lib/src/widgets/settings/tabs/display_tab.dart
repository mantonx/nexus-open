import 'package:flutter/material.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
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
  late NexusApiService _api;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
  }

  Future<void> _setBrightness(double v) async {
    try {
      await _api.setBrightness(v.round());
    } catch (_) {}
  }

  void _pickColor(
    BuildContext context,
    Color initial,
    String label,
    void Function(Color) onChanged,
  ) {
    showDialog(
      context: context,
      builder: (_) => _ColorPickerDialog(
        initial: initial,
        label: label,
        onConfirm: onChanged,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final ws = context.watch<WsService>();
    final cs = Theme.of(context).colorScheme;
    final connected = ws.isConnected;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        // ── Brightness ───────────────────────────────────────────────────
        NexusSection(
          title: 'Brightness',
          description: 'Physical display brightness (0–100).',
          trailing: connected
              ? null
              : const NexusStatusBadge(status: NexusStatus.warning, label: 'Device required'),
          child: Row(
            children: [
              const Icon(Icons.brightness_low, size: AppIconSize.sm),
              Expanded(
                child: Slider(
                  value: _brightness,
                  min: 0,
                  max: 100,
                  divisions: 20,
                  label: _brightness.round().toString(),
                  activeColor: AppColors.accent,
                  onChanged: connected ? (v) => setState(() => _brightness = v) : null,
                  onChangeEnd: connected ? _setBrightness : null,
                ),
              ),
              const Icon(Icons.brightness_high, size: AppIconSize.sm),
            ],
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Display colours ──────────────────────────────────────────────
        NexusSection(
          title: 'Display Colours',
          description: 'Colours rendered on the 640×48 hardware display.',
          child: Column(
            children: [
              _ColorRow(
                label: 'Text colour',
                color: settings.textColorValue,
                onTap: () => _pickColor(
                  context,
                  settings.textColorValue,
                  'Text colour',
                  settings.setTextColor,
                ),
              ),
              Divider(
                  height: AppSpacing.sm,
                  color: cs.outline.withValues(alpha: 0.4)),
              _ColorRow(
                label: 'Background colour',
                color: settings.backgroundColorValue,
                onTap: () => _pickColor(
                  context,
                  settings.backgroundColorValue,
                  'Background colour',
                  settings.setBackgroundColor,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Units ────────────────────────────────────────────────────────
        NexusSection(
          title: 'Units',
          description: 'Measurement units for temperature and distance.',
          titleSpacing: AppSpacing.sm,
          child: Column(
            children: [
              _SettingRow(
                label: 'Temperature',
                tooltip: 'Celsius (°C) or Fahrenheit (°F)',
                child: NexusDropdownField<String>(
                  value: settings.temperatureUnit,
                  items: const [
                    DropdownMenuItem(
                        value: 'Celsius', child: Text('°C — Celsius')),
                    DropdownMenuItem(
                        value: 'Fahrenheit', child: Text('°F — Fahrenheit')),
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
                    DropdownMenuItem(
                        value: 'Kilometers', child: Text('km — Kilometers')),
                    DropdownMenuItem(
                        value: 'Miles', child: Text('mi — Miles')),
                  ],
                  onChanged: (v) => settings.setDistanceUnit(v!),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Date & time format ───────────────────────────────────────────
        NexusSection(
          title: 'Date & Time',
          description: 'How dates and times appear on the display.',
          titleSpacing: AppSpacing.sm,
          child: Column(
            children: [
              _SettingRow(
                label: 'Time format',
                tooltip: '24-hour or 12-hour AM/PM',
                child: NexusDropdownField<String>(
                  value: settings.timeFormat,
                  items: const [
                    DropdownMenuItem(value: '24h', child: Text('24h (14:30)')),
                    DropdownMenuItem(
                        value: '12h', child: Text('12h (2:30 PM)')),
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
                    DropdownMenuItem(
                        value: 'YYYY-MM-DD', child: Text('YYYY-MM-DD')),
                    DropdownMenuItem(
                        value: 'DD/MM/YYYY', child: Text('DD/MM/YYYY')),
                    DropdownMenuItem(
                        value: 'MM/DD/YYYY', child: Text('MM/DD/YYYY')),
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

// ── Colour row ────────────────────────────────────────────────────────────────

class _ColorRow extends StatelessWidget {
  const _ColorRow({
    required this.label,
    required this.color,
    required this.onTap,
  });

  final String label;
  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = Theme.of(context).colorScheme;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
      child: Row(
        children: [
          Expanded(
            child: Text(label, style: theme.textTheme.bodyLarge),
          ),
          Semantics(
            label: '$label colour swatch, tap to change',
            button: true,
            child: InkWell(
              onTap: onTap,
              borderRadius: AppRadius.smBr,
              child: Container(
                width: 48,
                height: 36,
                decoration: BoxDecoration(
                  color: color,
                  borderRadius: AppRadius.smBr,
                  border: Border.all(color: cs.outline),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Colour picker dialog ──────────────────────────────────────────────────────

class _ColorPickerDialog extends StatefulWidget {
  const _ColorPickerDialog({
    required this.initial,
    required this.label,
    required this.onConfirm,
  });

  final Color initial;
  final String label;
  final void Function(Color) onConfirm;

  @override
  State<_ColorPickerDialog> createState() => _ColorPickerDialogState();
}

class _ColorPickerDialogState extends State<_ColorPickerDialog> {
  late Color _current;

  @override
  void initState() {
    super.initState();
    _current = widget.initial;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text('Pick ${widget.label.toLowerCase()}'),
      content: SingleChildScrollView(
        child: ColorPicker(
          pickerColor: _current,
          onColorChanged: (c) => setState(() => _current = c),
          enableAlpha: false,
          displayThumbColor: true,
          paletteType: PaletteType.hsvWithHue,
        ),
      ),
      actions: [
        NexusButton.ghost(
          label: 'Cancel',
          onPressed: () => Navigator.of(context).pop(),
        ),
        NexusButton.primary(
          label: 'Apply',
          onPressed: () {
            widget.onConfirm(_current);
            Navigator.of(context).pop();
          },
        ),
      ],
    );
  }
}
