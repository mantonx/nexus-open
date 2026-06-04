import 'package:flutter/material.dart';
import '../../theme/app_tokens.dart';

/// The standard container for all settings content sections.
///
/// Replaces the scattered `Card + Padding + Column` pattern.
/// Use [NexusSection] when you also need a title row.
class NexusCard extends StatelessWidget {
  const NexusCard({
    super.key,
    required this.child,
    this.padding,
    this.accentBorder = false,
    this.accentColor,
    this.onTap,
    this.margin,
  });

  final Widget child;
  final EdgeInsetsGeometry? padding;

  /// Highlights the card with an orange left-border accent — used for
  /// selected states, active module cards, etc.
  final bool accentBorder;

  /// Override the accent colour (defaults to [AppColors.accent]).
  final Color? accentColor;

  final VoidCallback? onTap;
  final EdgeInsetsGeometry? margin;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final effectiveAccent = accentColor ?? AppColors.accent;

    Widget content = Padding(
      padding: padding ?? AppSpacing.cardPadding,
      child: child,
    );

    if (accentBorder) {
      content = IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              width: 3,
              decoration: BoxDecoration(
                color: effectiveAccent,
                borderRadius: const BorderRadius.only(
                  topLeft: Radius.circular(AppRadius.lg),
                  bottomLeft: Radius.circular(AppRadius.lg),
                ),
              ),
            ),
            Expanded(child: content),
          ],
        ),
      );
    }

    Widget card = Container(
      margin: margin ?? const EdgeInsets.only(bottom: AppSpacing.sm),
      decoration: BoxDecoration(
        color: cs.surfaceContainer,
        borderRadius: AppRadius.lgBr,
        border: Border.all(
          color: accentBorder ? effectiveAccent.withOpacity(0.4) : cs.outline,
          width: accentBorder ? 1.5 : 1,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: content,
    );

    if (onTap != null) {
      card = InkWell(
        onTap: onTap,
        borderRadius: AppRadius.lgBr,
        child: card,
      );
    }

    return card;
  }
}
