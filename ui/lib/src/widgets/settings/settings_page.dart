import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'tabs/location_tab.dart';
import 'tabs/display_tab.dart';
import 'tabs/preview_tab.dart';

/// Settings page with tabbed interface for configuring app preferences
class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  bool _updating = false;
  String selectedLocation = '';

  Future<void> _handleUpdate() async {
    setState(() => _updating = true);

    print('Updating settings with location: $selectedLocation');

    try {
      final response = await http.post(
        Uri.parse('http://localhost:1985/api/config'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({
          'unit': 'imperial',
          'timeFormat': '12h',
          'dateFormat': 'DD/MM/YYYY',
          'backgroundColor': '#FFFFFF',
          'textColor': '#FFFFFF',
          'location': selectedLocation, // Use selectedLocation from state
        }),
      );

      if (!mounted) return;

      if (response.statusCode == 200) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Settings updated successfully'),
            backgroundColor: Colors.green,
          ),
        );
      } else {
        throw 'Server returned ${response.statusCode}';
      }
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Update failed: $e'),
          backgroundColor: Colors.red,
        ),
      );
    } finally {
      if (mounted) {
        setState(() => _updating = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: Text('Settings',
              style: TextStyle(color: theme.colorScheme.tertiary)),
          bottom: TabBar(
            isScrollable: true,
            tabs: const [
              Tab(icon: Icon(Icons.location_on), text: 'Location'),
              Tab(icon: Icon(Icons.display_settings), text: 'Display'),
              Tab(icon: Icon(Icons.preview), text: 'Preview'),
            ],
            indicatorColor: theme.colorScheme.tertiary,
          ),
        ),
        body: TabBarView(
          children: [
            LocationTab(onLocationSelected: (location) {
              setState(() {
                selectedLocation = location;
              });
            }),
            const DisplayTab(),
            const PreviewTab(),
          ],
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: _updating ? null : _handleUpdate,
          backgroundColor: theme.colorScheme.tertiary,
          icon: _updating
              ? const SizedBox(
                  width: 24,
                  height: 24,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                  ),
                )
              : const Icon(Icons.save),
          label: Text(_updating ? 'Updating...' : 'Update'),
        ),
      ),
    );
  }
}
