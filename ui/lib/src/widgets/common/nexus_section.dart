import 'package:flutter/material.dart';
import '../../theme/app_tokens.dart';
import 'nexus_card.dart';

/// A labelled content section — [NexusCard] + title + optional description.
///
/// Replaces the repetitive pattern:
///   Card > Padding > Column > Text(title) > Text(description) > content
///
/// Used in every tab for grouping related controls.
class NexusSection extends StatelessWidget {
  const NexusSection({
    super.key,
    required this.title,
    required this.child,
    this.description,
    this.trailing,
    this.accentBorder = false,
    this.padding,
    this.titleSpacing = AppSpacing.md,
  });

  final String title;
  final Widget child;
  final String? description;

  /// Optional widget placed at the right side of the title row (e.g. a chip,
  /// status badge, or icon button).
  final Widget? trailing;

  final bool accentBorder;
  final EdgeInsetsGeometry? padding;

  /// Vertical space between the title block and [child]. Defaults to [AppSpacing.md].
  final double titleSpacing;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return NexusCard(
      accentBorder: accentBorder,
      padding: padding ?? AppSpacing.cardPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Title row
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              // Subtle left-edge anchor — structural, not a colour call-out.
              Container(
                width: 2,
                height: 12,
                margin: const EdgeInsets.only(right: AppSpacing.sm),
                decoration: BoxDecoration(
                  color: cs.onSurfaceVariant.withValues(alpha: 0.35),
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
                    if (description != null) ...[
                      const SizedBox(height: 2),
                      Text(description!, style: theme.textTheme.bodySmall),
                    ],
                  ],
                ),
              ),
              if (trailing != null) ...[
                const SizedBox(width: AppSpacing.sm),
                trailing!,
              ],
            ],
          ),
          const SizedBox(height: AppSpacing.sm),
          Divider(height: 1, thickness: 1, color: cs.outline),
          SizedBox(height: titleSpacing),
          child,
        ],
      ),
    );
  }
}
