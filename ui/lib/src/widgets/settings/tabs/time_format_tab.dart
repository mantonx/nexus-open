import 'package:flutter/material.dart';

/// Tab for configuring time format
class TimeFormatTab extends StatefulWidget {
  const TimeFormatTab({super.key});

  @override
  State<TimeFormatTab> createState() => _TimeFormatTabState();
}

class _TimeFormatTabState extends State<TimeFormatTab> {
  String clockFormat = '24';

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SingleChildScrollView(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Time Format',
              style: theme.textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.bold,
                fontSize: 18,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'Select your preferred time format',
              style: theme.textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            Card(
              elevation: 2,
              child: Column(
                children: [
                  RadioListTile(
                    title: Text(
                      '24-hour',
                      style: TextStyle(color: theme.colorScheme.primary),
                    ),
                    subtitle: Text(
                      'Example: 14:30',
                      style: theme.textTheme.bodyMedium,
                    ),
                    value: '24',
                    groupValue: clockFormat,
                    onChanged: (value) => setState(() => clockFormat = value!),
                  ),
                  const Divider(height: 1),
                  RadioListTile(
                    title: Text(
                      '12-hour',
                      style: TextStyle(color: theme.colorScheme.primary),
                    ),
                    subtitle: Text(
                      'Example: 2:30 PM',
                      style: theme.textTheme.bodyMedium,
                    ),
                    value: '12',
                    groupValue: clockFormat,
                    onChanged: (value) => setState(() => clockFormat = value!),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
