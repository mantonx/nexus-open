part of 'editor_tab.dart';

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
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.md, AppSpacing.md, AppSpacing.md, AppSpacing.sm),
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
          child: GridView.builder(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.sm, 0, AppSpacing.sm, AppSpacing.md),
            gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
              crossAxisCount: 2,
              mainAxisSpacing: 6,
              crossAxisSpacing: 6,
              mainAxisExtent: 76,
            ),
            itemCount: catalog.length,
            itemBuilder: (ctx, i) =>
                _PluginCard(entry: catalog[i], onAdd: onAdd),
          ),
        ),
      ],
    );
  }
}

class _PluginCard extends StatelessWidget {
  const _PluginCard({required this.entry, required this.onAdd});

  final PluginCatalogEntry entry;
  final ValueChanged<String> onAdd;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    final card = Tooltip(
      message: entry.descriptor.description,
      child: InkWell(
        onTap: () => onAdd(entry.id),
        borderRadius: AppRadius.smBr,
        child: Container(
          decoration: BoxDecoration(
            color: cs.surfaceContainerLow,
            borderRadius: AppRadius.smBr,
            border: Border.all(color: cs.outline.withValues(alpha: 0.5)),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: AppColors.accent.withValues(alpha: 0.1),
                  borderRadius: AppRadius.smBr,
                ),
                child: Icon(_pluginIcon(entry.id), size: 14, color: AppColors.accent),
              ),
              const SizedBox(height: 4),
              Text(
                entry.descriptor.name.isNotEmpty
                    ? entry.descriptor.name
                    : entry.id,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: cs.onSurface,
                  fontSize: 10,
                  fontWeight: FontWeight.w500,
                ),
                textAlign: TextAlign.center,
                overflow: TextOverflow.ellipsis,
                maxLines: 2,
              ),
            ],
          ),
        ),
      ),
    );

    return Draggable<String>(
      data: entry.id,
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          width: 80,
          height: 72,
          decoration: BoxDecoration(
            color: cs.surfaceContainerHigh,
            borderRadius: AppRadius.smBr,
            border: Border.all(color: AppColors.accent, width: 1.5),
            boxShadow: [
              BoxShadow(
                color: AppColors.accent.withValues(alpha: 0.2),
                blurRadius: 8,
                spreadRadius: 1,
              ),
            ],
          ),
          padding: const EdgeInsets.all(AppSpacing.sm),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(_pluginIcon(entry.id), size: 16, color: AppColors.accent),
              const SizedBox(height: 4),
              Text(
                entry.descriptor.name.isNotEmpty
                    ? entry.descriptor.name
                    : entry.id,
                style: theme.textTheme.labelSmall
                    ?.copyWith(color: AppColors.accent, fontSize: 9),
                textAlign: TextAlign.center,
                overflow: TextOverflow.ellipsis,
                maxLines: 2,
              ),
            ],
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.35, child: card),
      child: card,
    );
  }
}
