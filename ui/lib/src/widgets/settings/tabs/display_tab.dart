import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../models/settings_state.dart';
import '../../common/styled_dropdown.dart';

class DisplayTab extends StatelessWidget {
  const DisplayTab({super.key});

  Widget _buildLabelWithTooltip(String label, String tooltip) {
    return Tooltip(
      message: tooltip,
      child: Row(
        children: [
          Text(label),
          const SizedBox(width: 4),
          const Icon(Icons.help_outline, size: 16),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<SettingsState>(
      builder: (context, settings, child) {
        return ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Card(
              elevation: 2,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Units',
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                    Text(
                      'Choose your preferred measurement units for temperature and distance',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 16),
                    Row(
                      children: [
                        Expanded(
                          flex: 2,
                          child: _buildLabelWithTooltip(
                            'Temperature Unit',
                            'Choose between Celsius (°C) and Fahrenheit (°F) for temperature display',
                          ),
                        ),
                        Expanded(
                          flex: 3,
                          child: StyledDropdown<String>(
                            width: 120,
                            label: '',
                            value: settings.temperatureUnit,
                            items: const [
                              DropdownMenuItem(
                                  value: 'Celsius', child: Text('°C')),
                              DropdownMenuItem(
                                  value: 'Fahrenheit', child: Text('°F')),
                            ],
                            onChanged: (value) =>
                                settings.setTemperatureUnit(value!),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    Row(
                      children: [
                        Expanded(
                          flex: 2,
                          child: _buildLabelWithTooltip(
                            'Distance Unit',
                            'Choose between Kilometers (km) and Miles (mi) for distance measurements',
                          ),
                        ),
                        Expanded(
                          flex: 3,
                          child: StyledDropdown<String>(
                            width: 120,
                            label: '',
                            value: settings.distanceUnit,
                            items: const [
                              DropdownMenuItem(
                                  value: 'Kilometers', child: Text('km')),
                              DropdownMenuItem(
                                  value: 'Miles', child: Text('mi')),
                            ],
                            onChanged: (value) =>
                                settings.setDistanceUnit(value!),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            Card(
              elevation: 2,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Date & Time Format',
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                    Text(
                      'Customize how dates and times are displayed throughout the application',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 16),
                    Row(
                      children: [
                        Expanded(
                          flex: 2,
                          child: _buildLabelWithTooltip(
                            'Time Format',
                            '24-hour format shows times from 00:00 to 23:59\n12-hour format uses AM/PM indicators',
                          ),
                        ),
                        Expanded(
                          flex: 3,
                          child: StyledDropdown<String>(
                            width: 180,
                            label: '',
                            value: settings.timeFormat,
                            items: const [
                              DropdownMenuItem(
                                value: '24h',
                                child: Text('24-hour (14:30)'),
                              ),
                              DropdownMenuItem(
                                value: '12h',
                                child: Text('12-hour (2:30 PM)'),
                              ),
                            ],
                            onChanged: (value) =>
                                settings.setTimeFormat(value!),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    Row(
                      children: [
                        Expanded(
                          flex: 2,
                          child: _buildLabelWithTooltip(
                            'Date Format',
                            'YYYY-MM-DD: 2023-12-31\nDD/MM/YYYY: 31/12/2023\nMM/DD/YYYY: 12/31/2023',
                          ),
                        ),
                        Expanded(
                          flex: 3,
                          child: StyledDropdown<String>(
                            width: 180,
                            label: '',
                            value: settings.dateFormat,
                            items: const [
                              DropdownMenuItem(
                                value: 'YYYY-MM-DD',
                                child: Text('YYYY-MM-DD'),
                              ),
                              DropdownMenuItem(
                                value: 'DD/MM/YYYY',
                                child: Text('DD/MM/YYYY'),
                              ),
                              DropdownMenuItem(
                                value: 'MM/DD/YYYY',
                                child: Text('MM/DD/YYYY'),
                              ),
                            ],
                            onChanged: (value) =>
                                settings.setDateFormat(value!),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}
