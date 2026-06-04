import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
import '../../../models/settings_state.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

/// Display colour settings — text colour and background colour for the
/// 640×48 hardware display. The live preview strip is persistent in the
/// navigation rail and visible on every section.
class PreviewTab extends StatelessWidget {
  const PreviewTab({super.key});

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
    final cs = Theme.of(context).colorScheme;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
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
                  color: cs.outline.withOpacity(0.4)),
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
        // Miniature preview of how the colours interact
        NexusSection(
          title: 'Colour Preview',
          description: 'How text will appear on the display.',
          child: Container(
            height: 48,
            decoration: BoxDecoration(
              color: settings.backgroundColorValue,
              borderRadius: AppRadius.smBr,
              border: Border.all(color: cs.outline),
            ),
            alignment: Alignment.center,
            child: Text(
              '14:30  25°C  New York',
              style: TextStyle(
                color: settings.textColorValue,
                fontFamily: 'monospace',
                fontSize: 14,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ),
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
