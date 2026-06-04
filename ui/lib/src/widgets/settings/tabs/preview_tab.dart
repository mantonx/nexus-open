import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
import '../../../models/settings_state.dart';
import '../../../services/nexus_api_service.dart';
import '../../../services/ws_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

/// Display tab — colour settings, hardware preview, brightness, units, date/time.
class PreviewTab extends StatefulWidget {
  const PreviewTab({super.key});

  @override
  State<PreviewTab> createState() => _PreviewTabState();
}

class _PreviewTabState extends State<PreviewTab> {
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
    final theme = Theme.of(context);
    final connected = ws.isConnected;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        // ── Colour settings ─────────────────────────────────────────────
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
              Divider(height: AppSpacing.sm, color: cs.outline.withOpacity(0.4)),
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

        // ── Hardware preview ─────────────────────────────────────────────
        NexusSection(
          title: 'Colour Preview',
          description: 'How text will appear on the 640×48 display.',
          child: _DevicePreview(
            textColor: settings.textColorValue,
            backgroundColor: settings.backgroundColorValue,
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Brightness ───────────────────────────────────────────────────
        NexusSection(
          title: 'Brightness',
          description: 'Physical display brightness (0–100).',
          trailing: connected
              ? null
              : NexusStatusBadge(
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
                child: Text('${_brightness.round()}%',
                    style: theme.textTheme.labelLarge,
                    textAlign: TextAlign.end),
              ),
            ],
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Units ────────────────────────────────────────────────────────
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

        // ── Date & time ──────────────────────────────────────────────────
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

// ── Device bezel preview ─────────────────────────────────────────────────────

class _DevicePreview extends StatelessWidget {
  const _DevicePreview({
    required this.textColor,
    required this.backgroundColor,
  });

  final Color textColor;
  final Color backgroundColor;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        children: [
          // Outer bezel — mimics the physical iCUE Nexus housing
          Container(
            decoration: BoxDecoration(
              color: const Color(0xFF0A0A0C),
              borderRadius: AppRadius.smBr,
              border: Border.all(
                color: AppColors.dataAccent.withOpacity(0.35),
                width: 1,
              ),
              boxShadow: [
                BoxShadow(
                  color: AppColors.dataAccent.withOpacity(0.12),
                  blurRadius: 16,
                  spreadRadius: 1,
                ),
                BoxShadow(
                  color: Colors.black.withOpacity(0.6),
                  blurRadius: 12,
                  offset: const Offset(0, 4),
                ),
              ],
            ),
            padding: const EdgeInsets.symmetric(
              horizontal: 12,
              vertical: 6,
            ),
            child: Container(
              // 640×48 aspect ratio, constrained to fit the card
              height: 48,
              constraints: const BoxConstraints(maxWidth: 640),
              width: double.infinity,
              decoration: BoxDecoration(
                color: backgroundColor,
                borderRadius: BorderRadius.circular(2),
              ),
              alignment: Alignment.center,
              child: Text(
                '14:30  25°C  New York',
                style: TextStyle(
                  color: textColor,
                  fontFamily: 'monospace',
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                  letterSpacing: 1,
                ),
              ),
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Text(
            '640 × 48 px',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: AppColors.textMuted,
              letterSpacing: 0.8,
            ),
          ),
        ],
      ),
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

    // Determine a legible label colour for the swatch
    final luminance = color.computeLuminance();
    final swatchText = luminance > 0.5 ? Colors.black87 : Colors.white70;
    final hexStr =
        '#${color.value.toRadixString(16).substring(2).toUpperCase()}';

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
      child: Row(
        children: [
          Expanded(
            child: Text(label, style: theme.textTheme.bodyLarge),
          ),
          Semantics(
            label: '$label swatch, current value $hexStr, tap to change',
            button: true,
            child: InkWell(
              onTap: onTap,
              borderRadius: AppRadius.smBr,
              child: Container(
                width: 80,
                height: 36,
                decoration: BoxDecoration(
                  color: color,
                  borderRadius: AppRadius.smBr,
                  border: Border.all(color: cs.outline),
                ),
                alignment: Alignment.center,
                child: Text(
                  hexStr,
                  style: TextStyle(
                    color: swatchText,
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                    fontFamily: 'monospace',
                  ),
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
          showLabel: true,
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
