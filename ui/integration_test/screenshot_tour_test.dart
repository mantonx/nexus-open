import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:open_next/main.dart' as app;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('screenshot tour — all tabs', (tester) async {
    app.main();
    await tester.pumpAndSettle(const Duration(seconds: 3));

    // Dismiss onboarding if present
    final skipFinder = find.text('Skip');
    if (skipFinder.evaluate().isNotEmpty) {
      await tester.tap(skipFinder);
      await tester.pumpAndSettle();
    }

    final tabs = [
      ('Display & Colors', 'tab_display'),
      ('Location',         'tab_location'),
      ('Modules',          'tab_modules'),
      ('Images',           'tab_images'),
    ];

    for (final (tooltip, name) in tabs) {
      final navItem = find.byTooltip(tooltip);
      if (navItem.evaluate().isNotEmpty) {
        await tester.tap(navItem);
        await tester.pump(const Duration(milliseconds: 800));
      }
      await _screenshot(tester, name);

      // Scroll down to capture below-the-fold content, then scroll back up.
      // Only fires if a ListView is present and has scrollable content.
      final listFinder = find.byType(ListView);
      if (listFinder.evaluate().isNotEmpty) {
        await tester.drag(listFinder.first, const Offset(0, -400));
        await tester.pump(const Duration(milliseconds: 300));
        await _screenshot(tester, '${name}_scrolled');
        // Reset scroll position for the next tab
        await tester.drag(listFinder.first, const Offset(0, 800));
        await tester.pump(const Duration(milliseconds: 200));
      }
    }
  });
}

Future<void> _screenshot(WidgetTester tester, String name) async {
  final doneFile = '/tmp/nexus-shot-done-$name';
  debugPrint('NEXUS_SCREENSHOT:$name');
  for (var i = 0; i < 30; i++) {
    await tester.pump(const Duration(milliseconds: 100));
    // ignore: avoid_slow_async_io — intentional polling
    if (await File(doneFile).exists()) break;
  }
}
