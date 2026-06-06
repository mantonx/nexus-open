import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:provider/provider.dart';

import 'package:open_next/src/models/settings_state.dart';
import 'package:open_next/src/services/nexus_api_service.dart';
import 'package:open_next/src/services/ws_service.dart';
import 'package:open_next/src/widgets/settings/tabs/preview_tab.dart';
import 'package:open_next/src/widgets/settings/tabs/plugins_tab.dart';
// AppTheme is intentionally not imported here — google_fonts 5.1.0 has a
// compile-time incompatibility with Dart 3.11 in test mode. Tests use a
// plain ThemeData() instead. Upgrade google_fonts to fix (see pubspec.yaml).

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Builds a minimal widget tree with the providers the app uses.
Widget _wrap(
  Widget child, {
  SettingsState? settings,
  WsService? ws,
}) {
  return MultiProvider(
    providers: [
      ChangeNotifierProvider<SettingsState>.value(
          value: settings ?? SettingsState()),
      ChangeNotifierProvider<WsService>.value(value: ws ?? _FakeWsService()),
    ],
    child: MaterialApp(
      theme: ThemeData(
        colorScheme: const ColorScheme.light(
          tertiary: Color(0xFFDB8720),
        ),
      ),
      home: child,
    ),
  );
}

/// Mock HTTP client that returns canned responses by URL path.
MockClient _mockClient(Map<String, dynamic> routes) {
  return MockClient((request) async {
    final path = request.url.path;
    final body = routes[path];
    if (body != null) {
      return http.Response(json.encode(body), 200,
          headers: {'content-type': 'application/json'});
    }
    return http.Response('not found', 404);
  });
}

/// Minimal health + config mock
MockClient get _defaultClient => _mockClient({
      '/api/health': {'status': 'ok', 'version': '1.0.0', 'first_run': false},
      '/api/config': {
        'location': 'London',
        'time_format': '24h',
        'unit': 'metric',
        'background_color': '#000000',
        'background_image': 'bg.png',
        'text_color': '#ffffff',
        'image_paths': <String>[],
        'display': {
          'font_family': 'GoRegular',
          'font_size': 11.0,
          'time_font_size': 14.0,
          'layout': 'dashboard',
          'date_format': 'MM/DD/YYYY',
        },
      },
    });

// ── Fake WsService (never connects, always disconnected) ─────────────────────

class _FakeWsService extends WsService {
  _FakeWsService() : super.fake();
}

// ── Tests ────────────────────────────────────────────────────────────────────

void main() {
  group('SettingsState', () {
    test('load/save round-trip preserves config fields', () async {
      int postCount = 0;
      final client = MockClient((req) async {
        if (req.method == 'POST' && req.url.path == '/api/config') {
          postCount++;
          return http.Response('{"status":"success"}', 200,
              headers: {'content-type': 'application/json'});
        }
        return http.Response(
            json.encode({
              'location': 'Tokyo',
              'time_format': '12h',
              'unit': 'metric',
              'background_color': '#111111',
              'background_image': 'bg.png',
              'text_color': '#eeeeee',
              'image_paths': <String>[],
              'display': {
                'font_family': 'GoRegular',
                'font_size': 11.0,
                'time_font_size': 14.0,
                'layout': 'dashboard',
                'date_format': 'DD/MM/YYYY',
              },
            }),
            200,
            headers: {'content-type': 'application/json'});
      });

      final svc = NexusApiService(client: client);
      final state = SettingsState(apiService: svc);
      await state.loadFromBackend();

      expect(state.location, 'Tokyo');
      expect(state.timeFormat, '12h');
      expect(state.dateFormat, 'DD/MM/YYYY');

      await state.saveToBackend();
      expect(postCount, 1);
    });

    test('updateConfig mutates only supplied fields', () async {
      final state = SettingsState(
        apiService: NexusApiService(client: _defaultClient),
      );
      await state.loadFromBackend();

      final originalTime = state.timeFormat;
      state.updateConfig(location: 'Paris');

      expect(state.location, 'Paris');
      expect(state.timeFormat, originalTime); // unchanged
    });
  });

  group('PreviewTab', () {
    // PreviewTab is now the Display Colours tab — the live preview strip
    // moved to the persistent _DisplayStrip in the NavigationRail.

    testWidgets('shows Display Colours section', (tester) async {
      final settings = SettingsState(
          apiService: NexusApiService(client: _defaultClient));
      await settings.loadFromBackend();

      await tester.pumpWidget(_wrap(
        const Scaffold(body: PreviewTab()),
        settings: settings,
      ));
      await tester.pump();

      expect(find.text('Display Colours'), findsOneWidget);
    });

    testWidgets('shows colour swatches for text and background', (tester) async {
      final settings = SettingsState(
          apiService: NexusApiService(client: _defaultClient));
      await settings.loadFromBackend();

      await tester.pumpWidget(_wrap(
        const Scaffold(body: PreviewTab()),
        settings: settings,
      ));
      await tester.pump();

      expect(find.text('Text colour'), findsOneWidget);
      expect(find.text('Background colour'), findsOneWidget);
    });
  });

  group('PluginsTab', () {
    testWidgets('renders plugin cards for known plugins', (tester) async {
      final client = MockClient((req) async {
        if (req.url.path.contains('/config')) {
          return http.Response(
              json.encode({'config': {}, 'plugin': req.url.path}), 200,
              headers: {'content-type': 'application/json'});
        }
        if (req.url.path.contains('/status')) {
          return http.Response(
              json.encode({'status': 'ok', 'error': ''}), 200,
              headers: {'content-type': 'application/json'});
        }
        return http.Response('{}', 200,
            headers: {'content-type': 'application/json'});
      });

      final settings =
          SettingsState(apiService: NexusApiService(client: client));

      await tester.pumpWidget(_wrap(
        const Scaffold(body: PluginsTab()),
        settings: settings,
      ));

      // Pump to flush async loads (avoid pumpAndSettle — AnimatedContainer loops)
      for (int i = 0; i < 20; i++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      // Labels in the redesigned card are the short names
      expect(find.text('CPU'), findsOneWidget);
      expect(find.text('GPU'), findsOneWidget);
      expect(find.text('Weather'), findsOneWidget);
      expect(find.text('Network'), findsOneWidget);
    });

    testWidgets('does not show error subtitle when zone is ok',
        (tester) async {
      // Verify no error text when status is ok — the complement of the error case.
      // (ModulesTab creates its own NexusApiService; error display is validated
      //  via the Go API integration test when the sampler is running.)
      final client = MockClient((req) async {
        return http.Response(
            json.encode({'config': {}, 'status': 'ok', 'error': ''}), 200,
            headers: {'content-type': 'application/json'});
      });
      final settings =
          SettingsState(apiService: NexusApiService(client: client));

      await tester.pumpWidget(_wrap(
        const Scaffold(body: PluginsTab()),
        settings: settings,
      ));
      // Use timed pumps — AnimatedContainer in cards prevents pumpAndSettle
      for (int i = 0; i < 20; i++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      // No error/timeout text should be present when all zones report ok
      expect(find.textContaining('Plugin error'), findsNothing);
      expect(find.textContaining('Plugin timeout'), findsNothing);
    });
  });
}

