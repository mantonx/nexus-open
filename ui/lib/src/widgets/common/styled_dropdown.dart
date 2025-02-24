import 'package:flutter/material.dart';

class StyledDropdown<T> extends StatelessWidget {
  final String label;
  final String? helperText;
  final T value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?> onChanged;
  final double? width;

  const StyledDropdown({
    super.key,
    required this.label,
    this.helperText,
    required this.value,
    required this.items,
    required this.onChanged,
    this.width,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: width,
      child: DropdownButtonFormField<T>(
        value: value,
        items: items,
        onChanged: onChanged,
        decoration: InputDecoration(
          labelText: label,
          helperText: helperText,
          border: const OutlineInputBorder(),
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 12,
            vertical: 8,
          ),
        ),
        borderRadius: BorderRadius.circular(8),
        isDense: true,
      ),
    );
  }
}
