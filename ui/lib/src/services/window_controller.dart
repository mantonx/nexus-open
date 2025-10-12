import 'dart:async';
import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:window_manager/window_manager.dart';

/// Controls window visibility based on commands from the backend API
class WindowController {
  static const String _apiBaseUrl = 'http://localhost:1985';
  static const Duration _pollInterval = Duration(milliseconds: 500);

  Timer? _pollTimer;
  String _lastState = 'shown';

  /// Start listening for window state changes from the API
  void startListening() {
    _pollTimer = Timer.periodic(_pollInterval, (_) => _checkWindowState());
  }

  /// Stop listening for window state changes
  void dispose() {
    _pollTimer?.cancel();
    _pollTimer = null;
  }

  /// Check the window state from the API and update accordingly
  Future<void> _checkWindowState() async {
    try {
      final response = await http.get(
        Uri.parse('$_apiBaseUrl/api/window/state'),
      ).timeout(const Duration(seconds: 2));

      if (response.statusCode == 200) {
        final data = json.decode(response.body);
        final state = data['state'] as String?;

        if (state != null && state != _lastState) {
          _lastState = state;
          await _updateWindowVisibility(state);
        }
      }
    } catch (e) {
      // Silently ignore errors to avoid spamming logs
      // The backend might not be ready yet or temporarily unavailable
    }
  }

  /// Update window visibility based on state
  Future<void> _updateWindowVisibility(String state) async {
    if (state == 'shown') {
      await windowManager.show();
      await windowManager.focus();
    } else if (state == 'hidden') {
      await windowManager.hide();
    }
  }
}
