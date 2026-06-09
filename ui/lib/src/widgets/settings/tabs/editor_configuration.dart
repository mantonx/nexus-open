part of 'editor_tab.dart';

// ── Configuration panel ───────────────────────────────────────────────────────

class _Configuration extends StatelessWidget {
  const _Configuration({
    required this.zone,
    required this.catalog,
    required this.onPatch,
  });

  final LayoutZone? zone;
  final PluginCatalogEntry? catalog;
  final void Function(String zoneId, Map<String, dynamic> patch) onPatch;

  static String _shortPlugin(String plugin) {
    if (plugin.contains(':')) return plugin.split(':').last;
    return plugin.split('/').where((s) => s.isNotEmpty).last;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    if (zone == null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(AppSpacing.md),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.touch_app_outlined,
                  size: 28, color: cs.onSurfaceVariant.withValues(alpha: 0.3)),
              const SizedBox(height: 8),
              Text(
                'Select a zone to configure',
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: cs.onSurfaceVariant),
                textAlign: TextAlign.center,
              ),
            ],
          ),
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
            'CONFIGURATION',
            style: theme.textTheme.labelSmall?.copyWith(
              color: cs.onSurfaceVariant,
              letterSpacing: 1.5,
              fontSize: 9,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Row(
            children: [
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: AppColors.accent.withValues(alpha: 0.1),
                  borderRadius: AppRadius.smBr,
                ),
                child: Icon(_pluginIcon(z.plugin), size: 16, color: AppColors.accent),
              ),
              const SizedBox(width: AppSpacing.sm),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      catalog?.descriptor.name ?? _shortPlugin(z.plugin),
                      style:
                          theme.textTheme.titleSmall?.copyWith(color: cs.onSurface),
                    ),
                    if (catalog?.descriptor.description.isNotEmpty == true)
                      Text(
                        catalog!.descriptor.description,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: cs.onSurfaceVariant,
                          fontSize: 10,
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: AppSpacing.md),
          Divider(height: 1, color: cs.outline),
          const SizedBox(height: AppSpacing.sm),
          Text(
            'COLOURS',
            style: theme.textTheme.labelSmall?.copyWith(
              color: cs.onSurfaceVariant,
              letterSpacing: 1.5,
              fontSize: 9,
            ),
          ),
          const SizedBox(height: AppSpacing.xs),
          _ZoneColorRow(
            label: catalog?.descriptor.hasGraph == true ? 'Text & Graph' : 'Text',
            hexColor: z.themeOverride['accent'] as String?,
            fallbackHex: '#00C8FF',
            onChanged: (hex) => onPatch(z.id, {
              'theme_override': {...z.themeOverride, 'accent': hex},
            }),
          ),
          if (fields.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.sm),
            Divider(height: 1, color: cs.outline),
            const SizedBox(height: AppSpacing.md),
            ...fields
                .where((field) => field.showIf?.isVisible(z.config) ?? true)
                .map((field) => _SchemaField(
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

// ── Zone colour row ───────────────────────────────────────────────────────────

class _ZoneColorRow extends StatelessWidget {
  const _ZoneColorRow({
    required this.label,
    required this.onChanged,
    this.hexColor,
    this.fallbackHex = '#FFFFFF',
  });

  final String label;
  final String? hexColor;
  final String fallbackHex;
  final ValueChanged<String> onChanged;

  Color get _color {
    final h = hexColor ?? fallbackHex;
    if (h.length == 7 && h.startsWith('#')) {
      final v = int.tryParse(h.substring(1), radix: 16);
      if (v != null) return Color(0xFF000000 | v);
    }
    return Colors.white;
  }

  static String _toHex(Color c) {
    final r = (c.r * 255).round();
    final g = (c.g * 255).round();
    final b = (c.b * 255).round();
    return '#${r.toRadixString(16).padLeft(2, '0')}'
        '${g.toRadixString(16).padLeft(2, '0')}'
        '${b.toRadixString(16).padLeft(2, '0')}'.toUpperCase();
  }

  void _pick(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (_) => _ZoneColorPickerDialog(
        label: label,
        initial: _color,
        onConfirm: (c) => onChanged(_toHex(c)),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final isOverridden = hexColor != null;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          Expanded(
            child: Text(
              label,
              style: theme.textTheme.labelSmall?.copyWith(
                color: cs.onSurfaceVariant,
              ),
            ),
          ),
          if (isOverridden)
            GestureDetector(
              onTap: () => onChanged(''),
              child: Padding(
                padding: const EdgeInsets.only(right: 6),
                child: Icon(Icons.restart_alt,
                    size: 12, color: cs.onSurfaceVariant.withValues(alpha: 0.5)),
              ),
            ),
          Semantics(
            label: '$label colour, tap to change',
            button: true,
            child: InkWell(
              onTap: () => _pick(context),
              borderRadius: AppRadius.smBr,
              child: Container(
                width: 36,
                height: 20,
                decoration: BoxDecoration(
                  color: _color,
                  borderRadius: AppRadius.smBr,
                  border: Border.all(
                    color: isOverridden ? AppColors.accent : cs.outline,
                    width: isOverridden ? 1.5 : 1,
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

class _ZoneColorPickerDialog extends StatefulWidget {
  const _ZoneColorPickerDialog({
    required this.label,
    required this.initial,
    required this.onConfirm,
  });

  final String label;
  final Color initial;
  final ValueChanged<Color> onConfirm;

  @override
  State<_ZoneColorPickerDialog> createState() => _ZoneColorPickerDialogState();
}

class _ZoneColorPickerDialogState extends State<_ZoneColorPickerDialog> {
  late Color _current;

  @override
  void initState() {
    super.initState();
    _current = widget.initial;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text('${widget.label} colour'),
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

// ── Schema field ──────────────────────────────────────────────────────────────

class _SchemaField extends StatelessWidget {
  const _SchemaField(
      {required this.field,
      required this.currentValue,
      required this.onChanged});
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
                    color: Theme.of(context)
                        .colorScheme
                        .onSurfaceVariant
                        .withValues(alpha: 0.55),
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
        final optLabels = {for (final o in field.options) o.value: o.label};
        final val = currentValue as String? ??
            field.defaultValue as String? ??
            (opts.isNotEmpty ? opts.first : '');
        return _EnumField(
            value: val, options: opts, labels: optLabels, onChanged: onChanged);
      case 'bool':
        final val =
            currentValue as bool? ?? field.defaultValue as bool? ?? false;
        return SizedBox(
          height: 28,
          child: Align(
            alignment: Alignment.centerLeft,
            child: Switch(
                value: val,
                onChanged: onChanged,
                activeThumbColor: AppColors.accent),
          ),
        );
      case 'int':
        final val =
            _toInt(currentValue) ?? _toInt(field.defaultValue) ?? 0;
        return _IntField(
            value: val, min: field.min, max: field.max, onChanged: onChanged);
      default:
        final val =
            currentValue as String? ?? field.defaultValue as String? ?? '';
        return SizedBox(
            height: 28,
            child: _StringFieldStateful(
                value: val, onChanged: (v) => onChanged(v)));
    }
  }
}

// ── Form field widgets ────────────────────────────────────────────────────────

class _IntField extends StatefulWidget {
  const _IntField(
      {required this.value, required this.onChanged, this.min, this.max});
  final int value;
  final ValueChanged<int> onChanged;
  final int? min;
  final int? max;

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
        decoration: const InputDecoration(
          isDense: true,
          contentPadding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          border: OutlineInputBorder(),
        ),
        onSubmitted: (v) {
          final n = int.tryParse(v);
          if (n == null) return;
          final clamped = (widget.min != null && n < widget.min!)
              ? widget.min!
              : (widget.max != null && n > widget.max!)
                  ? widget.max!
                  : n;
          widget.onChanged(clamped);
        },
      ),
    );
  }
}

class _EnumField extends StatelessWidget {
  const _EnumField({
    required this.value,
    required this.options,
    required this.onChanged,
    this.labels = const {},
  });
  final String value;
  final List<String> options;
  final Map<String, String> labels;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final effective = options.contains(value) ? value : options.first;
    return SizedBox(
      height: 28,
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border.all(color: cs.outline),
          borderRadius: BorderRadius.circular(4),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              value: effective,
              isDense: true,
              style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurface),
              dropdownColor: cs.surface,
              items: options
                  .map((o) =>
                      DropdownMenuItem(value: o, child: Text(labels[o] ?? o)))
                  .toList(),
              onChanged: (v) {
                if (v != null) onChanged(v);
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _StringFieldStateful extends StatefulWidget {
  const _StringFieldStateful(
      {required this.value, required this.onChanged});
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
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

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
