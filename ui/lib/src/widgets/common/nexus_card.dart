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

    // Two-layer approach: outer for shadow, inner for border+clip.
    // Individual BorderSide colors require no borderRadius — so we use a
    // uniform border and paint the top highlight as a separate 1px overlay.
    final isDark = cs.brightness == Brightness.dark;
    Widget card = Container(
      margin: margin ?? const EdgeInsets.only(bottom: AppSpacing.sm),
      decoration: BoxDecoration(
        borderRadius: AppRadius.lgBr,
        boxShadow: [
          BoxShadow(
            color: Colors.black.withOpacity(isDark ? 0.35 : 0.10),
            blurRadius: isDark ? 8 : 6,
            offset: const Offset(0, 2),
          ),
          BoxShadow(
            color: Colors.black.withOpacity(isDark ? 0.12 : 0.06),
            blurRadius: isDark ? 24 : 16,
            offset: const Offset(0, 6),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: AppRadius.lgBr,
        child: Container(
          decoration: BoxDecoration(
            color: cs.surfaceContainer,
            borderRadius: AppRadius.lgBr,
            border: Border.all(
              color: accentBorder
                  ? effectiveAccent.withOpacity(0.4)
                  : cs.outline,
              width: accentBorder ? 1.5 : 1,
            ),
          ),
          child: Stack(
            children: [
              content,
              // Top-edge highlight: simulates light source from above
              if (!accentBorder)
                Positioned(
                  top: 0, left: 0, right: 0,
                  child: Container(
                    height: 1,
                    color: Colors.white.withOpacity(0.06),
                  ),
                ),
            ],
          ),
        ),
      ),
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
