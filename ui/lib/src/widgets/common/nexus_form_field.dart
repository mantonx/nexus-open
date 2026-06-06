import 'package:flutter/material.dart';
import '../../theme/app_tokens.dart';

/// Unified text input for the app.
///
/// Replaces the duplicate [InputDecoration] definitions scattered across
/// the plugins tab and location tab. Styling comes entirely from the
/// theme's [InputDecorationTheme] — this widget just provides consistent
/// label/hint/validation wiring.
class NexusFormField extends StatelessWidget {
  const NexusFormField({
    super.key,
    this.label,
    this.hint,
    this.initialValue,
    this.controller,
    this.focusNode,
    this.onChanged,
    this.onSubmitted,
    this.validator,
    this.enabled = true,
    this.obscureText = false,
    this.keyboardType,
    this.textInputAction,
    this.suffixIcon,
    this.prefixIcon,
    this.maxLines = 1,
  });

  final String? label;
  final String? hint;
  final String? initialValue;
  final TextEditingController? controller;
  final FocusNode? focusNode;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final FormFieldValidator<String>? validator;
  final bool enabled;
  final bool obscureText;
  final TextInputType? keyboardType;
  final TextInputAction? textInputAction;
  final Widget? suffixIcon;
  final Widget? prefixIcon;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      initialValue: controller == null ? initialValue : null,
      controller: controller,
      focusNode: focusNode,
      onChanged: onChanged,
      onFieldSubmitted: onSubmitted,
      validator: validator,
      enabled: enabled,
      obscureText: obscureText,
      keyboardType: keyboardType,
      textInputAction: textInputAction,
      maxLines: maxLines,
      decoration: InputDecoration(
        labelText: label,
        hintText: hint,
        suffixIcon: suffixIcon,
        prefixIcon: prefixIcon,
        // All other styling comes from InputDecorationTheme in AppTheme
      ),
    );
  }
}

/// A labelled [DropdownButtonFormField] that matches [NexusFormField] styling.
class NexusDropdownField<T> extends StatelessWidget {
  const NexusDropdownField({
    super.key,
    this.label,
    required this.value,
    required this.items,
    required this.onChanged,
    this.enabled = true,
  });

  final String? label;
  final T? value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?>? onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DropdownButtonFormField<T>(
      initialValue: value,
      items: items,
      onChanged: enabled ? onChanged : null,
      style: theme.textTheme.bodyLarge,
      isDense: true,
      decoration: InputDecoration(
        labelText: label,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.md,
          vertical: AppSpacing.sm,
        ),
      ),
      dropdownColor: theme.colorScheme.surfaceContainerHigh,
      borderRadius: AppRadius.smBr,
    );
  }
}
