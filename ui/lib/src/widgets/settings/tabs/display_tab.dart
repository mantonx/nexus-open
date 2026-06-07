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

        // ── Colour preview ───────────────────────────────────────────────
        NexusSection(
          title: 'Colour Preview',
          description: 'How text will appear on the 640×48 display.',
          child: _DevicePreview(
            textColor: settings.textColorValue,
            backgroundColor: settings.backgroundColorValue,
            brightness: _brightness,
          ),
        ),
        const SizedBox(height: AppSpacing.sm),

        // ── Brightness ───────────────────────────────────────────────────
        NexusSection(
          title: 'Brightness',
          description: 'Physical display brightness (0–100).',
          titleSpacing: AppSpacing.sm,
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

// ── Hardware display preview ──────────────────────────────────────────────────
// Replicates the Corsair iCUE Nexus physical appearance:
//   - Black textured plastic housing (6.07" × 1.38" × 0.63")
//   - Glossy surround larger than the active display (display is recessed)
//   - Mounting slots on both ends

class _DevicePreview extends StatelessWidget {
  const _DevicePreview({
    required this.textColor,
    required this.backgroundColor,
    this.brightness = 75,
  });

  final Color textColor;
  final Color backgroundColor;
  final double brightness;

  static const _housingHighlight = Color(0xFF242428);
  static const _glossySurround = Color(0xFF0A0A0C);
  static const _mountingSlot = Color(0xFF1A1A1E);
  static const _displayBorder = Color(0xFF1C1C20);

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        children: [
          LayoutBuilder(
            builder: (context, constraints) {
              final w = constraints.maxWidth;
              return Container(
                width: w,
                decoration: BoxDecoration(
                  gradient: const LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [
                      Color(0xFF1A1A1C),
                      Color(0xFF0E0E10),
                      Color(0xFF121214),
                    ],
                    stops: [0.0, 0.4, 1.0],
                  ),
                  borderRadius: const BorderRadius.all(Radius.circular(4)),
                  border: Border.all(color: _housingHighlight, width: 0.5),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(alpha: 0.75),
                      blurRadius: 20,
                      offset: const Offset(0, 8),
                    ),
                    BoxShadow(
                      color: Colors.black.withValues(alpha: 0.4),
                      blurRadius: 4,
                      offset: const Offset(0, 2),
                    ),
                  ],
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 0, vertical: 10),
                  child: Row(
                    children: [
                      const _MountingEnd(slot: _mountingSlot),
                      Expanded(
                        child: Container(
                          decoration: BoxDecoration(
                            color: _glossySurround,
                            gradient: LinearGradient(
                              begin: Alignment.topLeft,
                              end: Alignment.bottomRight,
                              colors: [
                                Colors.white.withValues(alpha: 0.05),
                                Colors.transparent,
                              ],
                            ),
                          ),
                          padding: const EdgeInsets.symmetric(
                              horizontal: 8, vertical: 8),
                          child: Container(
                            decoration: BoxDecoration(
                              border:
                                  Border.all(color: _displayBorder, width: 1),
                              borderRadius: BorderRadius.circular(1),
                              boxShadow: [
                                BoxShadow(
                                  color: Colors.black.withValues(alpha: 0.9),
                                  blurRadius: 6,
                                  spreadRadius: 1,
                                ),
                              ],
                            ),
                            child: ClipRRect(
                              borderRadius: BorderRadius.circular(1),
                              child: Stack(
                                children: [
                                  Opacity(
                                    opacity:
                                        (brightness / 100).clamp(0.0, 1.0),
                                    child: Container(
                                      height: 48,
                                      color: backgroundColor,
                                      alignment: Alignment.center,
                                      child: Text(
                                        '14:30  25°C  New York',
                                        style: TextStyle(
                                          color: textColor,
                                          fontFamily: 'monospace',
                                          fontSize: 15,
                                          fontWeight: FontWeight.w600,
                                          letterSpacing: 1.5,
                                        ),
                                      ),
                                    ),
                                  ),
                                  Positioned.fill(
                                    child: CustomPaint(
                                        painter: _ScanlinePainter()),
                                  ),
                                  Positioned.fill(
                                    child: DecoratedBox(
                                      decoration: BoxDecoration(
                                        gradient: LinearGradient(
                                          begin: Alignment.topLeft,
                                          end: Alignment.centerRight,
                                          colors: [
                                            Colors.white
                                                .withValues(alpha: 0.06),
                                            Colors.transparent,
                                          ],
                                          stops: const [0.0, 0.4],
                                        ),
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                      const _MountingEnd(slot: _mountingSlot),
                    ],
                  ),
                ),
              );
            },
          ),
          const SizedBox(height: AppSpacing.sm),
          Text(
            'Corsair iCUE Nexus  ·  640 × 48',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: AppColors.textMuted,
                  letterSpacing: 0.6,
                ),
          ),
        ],
      ),
    );
  }
}

class _MountingEnd extends StatelessWidget {
  const _MountingEnd({required this.slot});
  final Color slot;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 14,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          _slot(slot),
          const SizedBox(height: 10),
          _slot(slot),
        ],
      ),
    );
  }

  Widget _slot(Color color) => Container(
        width: 4,
        height: 14,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(2),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.6),
              blurRadius: 2,
              offset: const Offset(1, 1),
            ),
          ],
        ),
      );
}

class _ScanlinePainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.black.withValues(alpha: 0.07)
      ..strokeWidth = 0.5;
    for (double y = 0; y < size.height; y += 2) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
    }
  }

  @override
  bool shouldRepaint(_ScanlinePainter _) => false;
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
    final luminance = color.computeLuminance();
    final swatchText = luminance > 0.5 ? Colors.black87 : Colors.white70;
    final hexStr =
        '#${color.toARGB32().toRadixString(16).substring(2).toUpperCase()}';

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
