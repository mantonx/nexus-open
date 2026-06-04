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

    return NexusCard(
      accentBorder: accentBorder,
      padding: padding ?? AppSpacing.cardPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Title row
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title, style: theme.textTheme.titleMedium),
                    if (description != null) ...[
                      const SizedBox(height: AppSpacing.xs),
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
          SizedBox(height: titleSpacing),
          child,
        ],
      ),
    );
  }
}
