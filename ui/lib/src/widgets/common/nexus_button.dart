import 'package:flutter/material.dart';
import '../../theme/app_tokens.dart';

/// Standardised button variants for the app.
///
/// Wraps Flutter's built-in button types with consistent brand styling
/// baked in, so call sites don't need inline `styleFrom` calls.
///
/// Variants:
///   NexusButton.primary    — filled orange, main actions
///   NexusButton.secondary  — outlined orange, secondary actions
///   NexusButton.destructive — filled red, irreversible actions only
///   NexusButton.ghost      — no border/fill, low-emphasis actions
class NexusButton extends StatelessWidget {
  const NexusButton._({
    super.key,
    required this.label,
    required this.onPressed,
    required this._variant,
    this.icon,
    this.loading = false,
    this.expand = false,
  });

  factory NexusButton.primary({
    Key? key,
    required String label,
    required VoidCallback? onPressed,
    Widget? icon,
    bool loading = false,
    bool expand = false,
  }) =>
      NexusButton._(
        key: key,
        label: label,
        onPressed: onPressed,
        variant: _Variant.primary,
        icon: icon,
        loading: loading,
        expand: expand,
      );

  factory NexusButton.secondary({
    Key? key,
    required String label,
    required VoidCallback? onPressed,
    Widget? icon,
    bool loading = false,
    bool expand = false,
  }) =>
      NexusButton._(
        key: key,
        label: label,
        onPressed: onPressed,
        variant: _Variant.secondary,
        icon: icon,
        loading: loading,
        expand: expand,
      );

  factory NexusButton.destructive({
    Key? key,
    required String label,
    required VoidCallback? onPressed,
    Widget? icon,
  }) =>
      NexusButton._(
        key: key,
        label: label,
        onPressed: onPressed,
        variant: _Variant.destructive,
        icon: icon,
      );

  factory NexusButton.ghost({
    Key? key,
    required String label,
    required VoidCallback? onPressed,
    Widget? icon,
  }) =>
      NexusButton._(
        key: key,
        label: label,
        onPressed: onPressed,
        variant: _Variant.ghost,
        icon: icon,
      );

  final String label;
  final VoidCallback? onPressed;
  final _Variant _variant;
  final Widget? icon;
  final bool loading;
  final bool expand;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    Widget child = loading
        ? Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                width: AppIconSize.sm,
                height: AppIconSize.sm,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  valueColor: AlwaysStoppedAnimation<Color>(
                    _variant == _Variant.secondary
                        ? AppColors.accent
                        : Colors.white,
                  ),
                ),
              ),
              const SizedBox(width: AppSpacing.sm),
              Text(label),
            ],
          )
        : Text(label);

    Widget button;

    switch (_variant) {
      case _Variant.primary:
        button = icon != null && !loading
            ? FilledButton.icon(
                onPressed: onPressed,
                icon: icon!,
                label: child,
              )
            : FilledButton(onPressed: onPressed, child: child);

      case _Variant.secondary:
        button = icon != null && !loading
            ? OutlinedButton.icon(
                onPressed: onPressed,
                icon: icon!,
                label: child,
              )
            : OutlinedButton(onPressed: onPressed, child: child);

      case _Variant.destructive:
        final style = FilledButton.styleFrom(
          backgroundColor: cs.critical,
          foregroundColor: Colors.white,
          shape: RoundedRectangleBorder(borderRadius: AppRadius.mdBr),
        );
        button = icon != null
            ? FilledButton.icon(
                onPressed: onPressed,
                icon: icon!,
                label: child,
                style: style,
              )
            : FilledButton(onPressed: onPressed, style: style, child: child);

      case _Variant.ghost:
        button = icon != null
            ? TextButton.icon(onPressed: onPressed, icon: icon!, label: child)
            : TextButton(onPressed: onPressed, child: child);
    }

    return expand
        ? SizedBox(width: double.infinity, child: button)
        : button;
  }
}

enum _Variant { primary, secondary, destructive, ghost }
