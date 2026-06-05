// Backend integration tests — run with NEXUS_WITH_BACKEND=1.
//
// These tests verify the live Flutter ↔ Go contract: connected state, config
// values reflected in UI, save round-trip, and WebSocket-driven behaviour.
//
// Usage (via script):
//   ./scripts/integration-test.sh --flutter
//
// Usage (direct):
//   NEXUS_WITH_BACKEND=1 flutter drive \
//     --driver=test_driver/integration_test.dart \
//     --target=integration_test/backend_integration_test.dart \
//     -d linux
//
// When NEXUS_WITH_BACKEND != '1' the entire test is skipped, so it is safe
// to include alongside the screenshot tour in a plain flutter drive run.
//
// NOTE: Flutter integration tests with flutter drive run inside a single
// persistent app instance. All assertions run as sequential steps inside one
// testWidgets call — launching app.main() multiple times across testWidgets
// blocks causes '!inTest' assertion failures in LiveTestWidgetsFlutterBinding.

import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'package:provider/provider.dart';
import 'package:open_next/main.dart' as app;
import 'package:open_next/src/models/settings_state.dart';
import 'package:open_next/src/services/ws_service.dart';

final _withBackend = Platform.environment['NEXUS_WITH_BACKEND'] == '1';
const _baseUrl = 'http://localhost:1985';

// ── Helpers ───────────────────────────────────────────────────────────────────

Future<Map<String, dynamic>> _apiGet(String path) async {
  final resp = await http.get(Uri.parse('$_baseUrl$path'));
  return json.decode(resp.body) as Map<String, dynamic>;
}

Future<http.Response> _apiPost(String path, Map<String, dynamic> body) async {
  return http.post(
    Uri.parse('$_baseUrl$path'),
    headers: {'Content-Type': 'application/json'},
    body: json.encode(body),
  );
}

/// Pump until [condition] is true or [timeout] expires.
Future<bool> _pumpUntil(
  WidgetTester tester,
  bool Function() condition, {
  Duration timeout = const Duration(seconds: 8),
  Duration interval = const Duration(milliseconds: 200),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    await tester.pump(interval);
    if (condition()) return true;
  }
  return false;
}

/// Pump a fixed number of frames — safe for tabs with AnimatedContainers that
/// never fully settle (pumpAndSettle times out on those).
Future<void> _pumpFrames(WidgetTester tester, {int count = 20}) async {
  for (var i = 0; i < count; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

Future<void> _tapTab(WidgetTester tester, String tooltip) async {
  final tab = find.byTooltip(tooltip);
  if (tab.evaluate().isNotEmpty) {
    await tester.tap(tab);
    await _pumpFrames(tester);
  }
}

// ── Entry point ───────────────────────────────────────────────────────────────

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('backend integration — full contract verification', (tester) async {
    if (!_withBackend) {
      markTestSkipped('requires NEXUS_WITH_BACKEND=1');
      return;
    }

    app.main();
    // pumpAndSettle waits for the HTTP health + config futures to resolve
    // before we start making assertions.
    await tester.pumpAndSettle(const Duration(seconds: 10));

    final context = tester.element(find.byType(MaterialApp));
    final settings = context.read<SettingsState>();
    final ws = context.read<WsService>();

    // ── 1. Connectivity ──────────────────────────────────────────────────────

    // No disconnected banner.
    expect(
      find.text('Backend disconnected — changes cannot be saved'),
      findsNothing,
      reason: 'disconnected banner must not appear when backend is running',
    );

    // SettingsState connected via HTTP.
    expect(settings.isConnected, isTrue,
        reason: 'HTTP health + config load should succeed');

    // WsService connected via WebSocket.
    final wsConnected = await _pumpUntil(tester, () => ws.isConnected);
    expect(wsConnected, isTrue,
        reason: 'WsService should connect to ws://localhost:1985/api/ws');

    // ── 2. Config contract ───────────────────────────────────────────────────

    // Fields from GET /api/config are reflected in SettingsState.
    final raw = await _apiGet('/api/config');
    expect(settings.config, isNotNull);

    final backendBg = raw['background_color'] as String?;
    if (backendBg != null) {
      expect(settings.backgroundColor, equals(backendBg),
          reason: 'backgroundColor must match GET /api/config response');
    }
    final backendText = raw['text_color'] as String?;
    if (backendText != null) {
      expect(settings.textColor, equals(backendText),
          reason: 'textColor must match GET /api/config response');
    }

    // ── 3. Save round-trip ───────────────────────────────────────────────────

    // Snapshot original for restore.
    final original = await _apiGet('/api/config');

    const testColor = '#123456';
    settings.setBackgroundColor(testColor);
    await settings.saveToBackend();
    await tester.pump(const Duration(milliseconds: 500));

    final saved = await _apiGet('/api/config');
    expect(saved['background_color'], equals(testColor),
        reason: 'POST /api/config must persist the mutated color');

    // Restore original values.
    final d = (original['display'] as Map<String, dynamic>?) ?? {};
    await _apiPost('/api/config', {
      'background_color': original['background_color'] ?? '#000000',
      'background_image': original['background_image'] ?? 'background.png',
      'text_color': original['text_color'] ?? '#FFFFFF',
      'image_paths': original['image_paths'] ?? <String>[],
      'display': {
        'font_family': d['font_family'] ?? 'GoRegular',
        'font_size': (d['font_size'] as num?)?.toDouble() ?? 11.0,
        'time_font_size': (d['time_font_size'] as num?)?.toDouble() ?? 14.0,
        'layout': d['layout'] ?? 'dashboard',
        'date_format': d['date_format'] ?? 'MM/DD/YYYY',
      },
    });

    // ── 4. Reload from backend ───────────────────────────────────────────────

    // Push a known value out-of-band, then verify retryConnection picks it up.
    await _apiPost('/api/config', {
      'background_color': '#ABCDEF',
      'background_image': 'background.png',
      'text_color': '#FFFFFF',
      'image_paths': <String>[],
      'display': {
        'font_family': 'GoRegular',
        'font_size': 11.0,
        'time_font_size': 14.0,
        'layout': 'dashboard',
        'date_format': 'MM/DD/YYYY',
      },
    });
    await settings.retryConnection();
    await tester.pumpAndSettle(const Duration(seconds: 5));
    expect(settings.backgroundColor, equals('#ABCDEF'),
        reason: 'retryConnection must reload backend state into SettingsState');

    // Restore original again.
    await _apiPost('/api/config', {
      'background_color': original['background_color'] ?? '#000000',
      'background_image': original['background_image'] ?? 'background.png',
      'text_color': original['text_color'] ?? '#FFFFFF',
      'image_paths': original['image_paths'] ?? <String>[],
      'display': d.isEmpty
          ? {
              'font_family': 'GoRegular',
              'font_size': 11.0,
              'time_font_size': 14.0,
              'layout': 'dashboard',
              'date_format': 'MM/DD/YYYY',
            }
          : d,
    });

    // ── 5. Display tab ───────────────────────────────────────────────────────

    await _tapTab(tester, 'Display & Colors');

    // WS connected → no "Device required" warning badge on brightness.
    expect(find.text('Device required'), findsNothing,
        reason: 'Device required badge must be absent when WS is connected');

    // ── 6. Modules tab ───────────────────────────────────────────────────────

    await _tapTab(tester, 'Modules');

    expect(find.text('CPU'), findsOneWidget);
    expect(find.text('GPU'), findsOneWidget);
    expect(find.text('Weather'), findsOneWidget);
    expect(find.text('Network'), findsOneWidget);

    // WS live → no "Not connected" badge.
    expect(find.text('Not connected'), findsNothing);

    // Configure card expand / collapse.
    final configure = find.text('Configure');
    expect(configure, findsWidgets);

    await tester.tap(configure.first);
    await _pumpFrames(tester);
    expect(find.text('Collapse'), findsOneWidget);

    await tester.tap(find.text('Collapse'));
    await _pumpFrames(tester);
    expect(find.text('Collapse'), findsNothing);

    // ── 7. Images tab ────────────────────────────────────────────────────────

    await _tapTab(tester, 'Images');
    expect(find.text('Upload'), findsOneWidget);

    // ── 8. WebSocket stays connected through window state changes ────────────

    await _apiPost('/api/window/hide', {});
    await _pumpFrames(tester, count: 10);

    await _apiPost('/api/window/show', {});
    await _pumpFrames(tester, count: 5);

    expect(ws.isConnected, isTrue,
        reason: 'WS connection must survive window hide/show cycle');
  });
}
