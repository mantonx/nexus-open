import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import 'tabs/location_tab.dart';
import 'tabs/display_tab.dart';
import 'tabs/preview_tab.dart';
import 'tabs/images_tab.dart';

/// Settings page with tabbed interface for configuring app preferences
class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  @override
  void initState() {
    super.initState();
    // Load settings from backend when page opens
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<SettingsState>().loadFromBackend();
    });
  }

  Future<void> _handleSave() async {
    final settingsState = context.read<SettingsState>();
    await settingsState.saveToBackend();

    if (!mounted) return;

    final success = settingsState.errorMessage == null;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(success
            ? 'Settings saved successfully'
            : 'Failed to save settings: ${settingsState.errorMessage}'),
        backgroundColor: success ? Colors.green : Colors.red,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Consumer<SettingsState>(
      builder: (context, settingsState, child) {
        return DefaultTabController(
          length: 4, // Added Images tab
          child: Scaffold(
            appBar: AppBar(
              title: Row(
                children: [
                  Text('Settings',
                      style: TextStyle(color: theme.colorScheme.tertiary)),
                  const SizedBox(width: 8),
                  // Connection status indicator
                  if (settingsState.isLoading)
                    const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  else
                    Icon(
                      settingsState.isConnected
                          ? Icons.cloud_done
                          : Icons.cloud_off,
                      size: 20,
                      color: settingsState.isConnected
                          ? Colors.green
                          : Colors.orange,
                    ),
                ],
              ),
              bottom: TabBar(
                isScrollable: true,
                tabs: const [
                  Tab(icon: Icon(Icons.location_on), text: 'Location'),
                  Tab(icon: Icon(Icons.display_settings), text: 'Display'),
                  Tab(icon: Icon(Icons.image), text: 'Images'),
                  Tab(icon: Icon(Icons.preview), text: 'Preview'),
                ],
                indicatorColor: theme.colorScheme.tertiary,
              ),
            ),
            body: TabBarView(
              children: [
                LocationTab(
                  onLocationSelected: (location) {
                    settingsState.updateConfig(location: location);
                  },
                ),
                const DisplayTab(),
                const ImagesTab(),
                const PreviewTab(),
              ],
            ),
            floatingActionButton: FloatingActionButton.extended(
              onPressed: settingsState.isLoading ? null : _handleSave,
              backgroundColor: theme.colorScheme.tertiary,
              icon: settingsState.isLoading
                  ? const SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                      ),
                    )
                  : const Icon(Icons.save),
              label: Text(settingsState.isLoading ? 'Saving...' : 'Save'),
            ),
          ),
        );
      },
    );
  }
}
