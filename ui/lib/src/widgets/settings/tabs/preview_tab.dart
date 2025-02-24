import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
import '../../../models/settings_state.dart';
import 'package:intl/intl.dart';

class PreviewTab extends StatelessWidget {
  const PreviewTab({super.key});

  void _openColorPicker(Color initialColor, Function(Color) onColorChanged,
      BuildContext context) {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        Color tempColor = initialColor;
        return AlertDialog(
          title: const Text('Pick a color'),
          content: SingleChildScrollView(
            child: ColorPicker(
              pickerColor: tempColor,
              onColorChanged: (color) => tempColor = color,
              enableAlpha: false,
              displayThumbColor: true,
              showLabel: true,
              paletteType: PaletteType.hsvWithHue,
            ),
          ),
          actions: [
            TextButton(
              onPressed: () {
                onColorChanged(tempColor);
                Navigator.of(context).pop();
              },
              child: const Text('Done'),
            ),
          ],
        );
      },
    );
  }

  String _formatCurrentTime(SettingsState settings) {
    final now = DateTime.now();
    if (settings.timeFormat == '24h') {
      return DateFormat('HH:mm').format(now);
    }
    return DateFormat('hh:mm a').format(now);
  }

  String _formatCurrentDate(SettingsState settings) {
    final now = DateTime.now();
    switch (settings.dateFormat) {
      case 'DD/MM/YYYY':
        return DateFormat('dd/MM/yyyy').format(now);
      case 'MM/DD/YYYY':
        return DateFormat('MM/dd/yyyy').format(now);
      default:
        return DateFormat('yyyy-MM-dd').format(now);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<SettingsState>(
      builder: (context, settings, child) {
        return ListView(
          children: [
            Card(
              margin: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.all(16),
                    child: Text(
                      'Display Colors',
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                  ListTile(
                    title: const Text('Text Color'),
                    trailing: InkWell(
                      onTap: () => _openColorPicker(
                        settings.textColor,
                        (color) => settings.setTextColor(color),
                        context,
                      ),
                      child: Container(
                        width: 40,
                        height: 40,
                        decoration: BoxDecoration(
                          color: settings.textColor,
                          border: Border.all(color: Colors.grey),
                          borderRadius: BorderRadius.circular(8),
                        ),
                      ),
                    ),
                  ),
                  ListTile(
                    title: const Text('Background Color'),
                    trailing: InkWell(
                      onTap: () => _openColorPicker(
                        settings.backgroundColor,
                        (color) => settings.setBackgroundColor(color),
                        context,
                      ),
                      child: Container(
                        width: 40,
                        height: 40,
                        decoration: BoxDecoration(
                          color: settings.backgroundColor,
                          border: Border.all(color: Colors.grey),
                          borderRadius: BorderRadius.circular(8),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
            Container(
              margin: const EdgeInsets.all(16),
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: Colors.black87,
                borderRadius: BorderRadius.circular(8),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withOpacity(0.5),
                    blurRadius: 10,
                    spreadRadius: 2,
                  ),
                ],
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'LCD Display Preview',
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          color: Colors.white70,
                        ),
                  ),
                  const SizedBox(height: 16),
                  Center(
                    child: Container(
                      width: 640,
                      height: 48,
                      decoration: BoxDecoration(
                        color: settings.backgroundColor,
                        border: Border.all(
                          color: Colors.grey.shade800,
                          width: 2,
                        ),
                        image: const DecorationImage(
                          image: AssetImage('assets/background.gif'),
                          fit: BoxFit.cover,
                          opacity: 0.1,
                        ),
                        boxShadow: [
                          BoxShadow(
                            color: settings.textColor.withOpacity(0.15),
                            blurRadius: 5,
                            spreadRadius: 1,
                          ),
                        ],
                      ),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          _buildLCDText(_formatCurrentTime(settings), settings),
                          _buildLCDText(_formatCurrentDate(settings), settings),
                          _buildLCDText(
                              '21°${settings.temperatureUnit == 'Celsius' ? 'C' : 'F'}',
                              settings),
                        ]
                            .map((widget) => Padding(
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 16),
                                  child: widget,
                                ))
                            .toList(),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }

  Widget _buildLCDText(String text, SettingsState settings) {
    return Text(
      text,
      style: TextStyle(
        color: settings.textColor,
        fontSize: 16,
        fontFamily: 'monospace',
        shadows: [
          Shadow(
            color: settings.textColor.withOpacity(0.5),
            blurRadius: 2,
            offset: const Offset(0, 0),
          ),
        ],
      ),
    );
  }
}
