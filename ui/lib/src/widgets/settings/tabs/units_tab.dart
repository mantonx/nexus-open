import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';

/// Tab for configuring measurement units
class UnitsTab extends StatefulWidget {
  const UnitsTab({super.key});

  @override
  State<UnitsTab> createState() => _UnitsTabState();
}

class _UnitsTabState extends State<UnitsTab> {
  String unit = 'metric';

  Future<void> _updateConfig(String newUnit) async {
    final url = Uri.parse('http://localhost:1985/api/config');
    final response = await http.post(
      url,
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'unit': newUnit}),
    );

    if (response.statusCode == 200) {
      setState(() {
        unit = newUnit;
      });
    } else {
      // Handle error
      print('Failed to update config: ${response.body}');
    }
  }

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
              'Measurement Units',
              style: theme.textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.bold,
                fontSize: 18,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'Choose your preferred measurement system',
              style: theme.textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            Card(
              elevation: 2,
              child: Column(
                children: [
                  RadioListTile(
                    title: Text(
                      'Metric',
                      style: TextStyle(color: theme.colorScheme.primary),
                    ),
                    subtitle: Text(
                      'Celsius, kilometers, etc.',
                      style: theme.textTheme.bodyMedium,
                    ),
                    value: 'metric',
                    groupValue: unit,
                    onChanged: (value) => _updateConfig(value!),
                  ),
                  const Divider(height: 1),
                  RadioListTile(
                    title: Text(
                      'Imperial',
                      style: TextStyle(color: theme.colorScheme.primary),
                    ),
                    subtitle: Text(
                      'Fahrenheit, miles, etc.',
                      style: theme.textTheme.bodyMedium,
                    ),
                    value: 'imperial',
                    groupValue: unit,
                    onChanged: (value) => _updateConfig(value!),
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
