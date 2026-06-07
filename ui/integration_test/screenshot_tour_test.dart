import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:provider/provider.dart';
import 'package:open_next/main.dart' as app;
import 'package:open_next/src/models/settings_state.dart';

// When NEXUS_WITH_BACKEND=1, the Go backend drives firstRun state via the
// health endpoint — no need to force it manually.
final _withBackend = Platform.environment['NEXUS_WITH_BACKEND'] == '1';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('screenshot tour — full app coverage', (tester) async {
    app.main();
    if (_withBackend) {
      // Wait for the real HTTP round-trips (health + config) to complete.
      // pumpAndSettle drains the frame queue after each pump until the tree
      // is idle, which lets the async futures from SettingsState._initialize()
      // resolve before we start navigating.
      await tester.pumpAndSettle(const Duration(seconds: 10));
    } else {
      await tester.pump(const Duration(seconds: 2));
    }

    // ── Onboarding ────────────────────────────────────────────────────────────
    final context = tester.element(find.byType(MaterialApp));
    if (!_withBackend) {
      // No backend: force onboarding manually so the tour covers that flow.
      context.read<SettingsState>().forceFirstRun();
    }
    await tester.pump(const Duration(milliseconds: 400));

    final welcomeBtn = find.text('Get started');
    if (welcomeBtn.evaluate().isNotEmpty) {
      await _screenshot(tester, 'onboarding_welcome');
      await tester.tap(welcomeBtn);
      await tester.pump(const Duration(milliseconds: 300));

      await _screenshot(tester, 'onboarding_connect');
      final skipBtn = find.text('Skip for now');
      final continueBtn = find.text('Continue');
      if (skipBtn.evaluate().isNotEmpty) {
        await tester.tap(skipBtn);
      } else if (continueBtn.evaluate().isNotEmpty) {
        await tester.tap(continueBtn);
      }
      await tester.pump(const Duration(milliseconds: 300));

      await _screenshot(tester, 'onboarding_location');
      if (continueBtn.evaluate().isNotEmpty) {
        await tester.tap(continueBtn);
      }
      await tester.pump(const Duration(milliseconds: 300));

      await _screenshot(tester, 'onboarding_done');
      final openBtn = find.text('Open settings');
      if (openBtn.evaluate().isNotEmpty) {
        await tester.tap(openBtn);
      }
      // Wait for settings page to render
      await tester.pump(const Duration(milliseconds: 1200));
    }

    // ── Settings tabs (3-mode rail: Editor / Global / Device) ────────────────
    // Extra settle time for Editor tab — it fires getDraft() + getPluginCatalog()
    // concurrently on load, which may take a moment against a real backend.
    await tester.pumpAndSettle(const Duration(seconds: 3));
    await _screenshot(tester, 'tab_editor');

    // Global tab — wraps Display, Images, Location sub-tabs
    final globalNav = find.byTooltip('Global Settings');
    if (globalNav.evaluate().isNotEmpty) {
      await tester.tap(globalNav);
      await tester.pumpAndSettle(const Duration(milliseconds: 800));
    }
    await _screenshot(tester, 'tab_global');

    // Navigate Global sub-tabs: Display → Images → Location
    for (final (label, name) in [
      ('Display',  'tab_display'),
      ('Images',   'tab_images'),
      ('Location', 'tab_location'),
    ]) {
      final subTab = find.text(label);
      if (subTab.evaluate().isNotEmpty) {
        await tester.tap(subTab);
        await tester.pumpAndSettle(const Duration(milliseconds: 600));
      }
      await _screenshot(tester, name);

      // Scroll to capture below-the-fold content
      final listFinder = find.byType(ListView);
      if (listFinder.evaluate().isNotEmpty) {
        await tester.drag(listFinder.first, const Offset(0, -400));
        await tester.pump(const Duration(milliseconds: 300));
        await _screenshot(tester, '${name}_scrolled');
        await tester.drag(listFinder.first, const Offset(0, 800));
        await tester.pump(const Duration(milliseconds: 200));
      }

      // ── Colour picker dialog (Display sub-tab) ──────────────────────────
      if (name == 'tab_display') {
        final swatchFinder = find.bySemanticsLabel(RegExp(r'Text colour swatch'));
        if (swatchFinder.evaluate().isNotEmpty) {
          await tester.tap(swatchFinder.first);
          await tester.pump(const Duration(milliseconds: 400));
          if (find.byType(AlertDialog).evaluate().isNotEmpty) {
            await _screenshot(tester, 'dialog_colour_picker');
            await tester.tap(find.text('Cancel'));
            await tester.pump(const Duration(milliseconds: 200));
          }
        }
      }
    }

    // Device tab
    final deviceNav = find.byTooltip('Device & Health');
    if (deviceNav.evaluate().isNotEmpty) {
      await tester.tap(deviceNav);
      await tester.pumpAndSettle(const Duration(milliseconds: 800));
    }
    await _screenshot(tester, 'tab_device');

    // ── Light mode ────────────────────────────────────────────────────────────
    final lightModeBtn = find.byTooltip('Light mode');
    if (lightModeBtn.evaluate().isNotEmpty) {
      await tester.tap(lightModeBtn);
      await tester.pump(const Duration(milliseconds: 400));

      // Tour the three rail modes in light mode
      for (final (tooltip, name) in [
        ('Layout Editor',  'light_tab_editor'),
        ('Global Settings','light_tab_global'),
        ('Device & Health','light_tab_device'),
      ]) {
        final navItem = find.byTooltip(tooltip);
        if (navItem.evaluate().isNotEmpty) {
          await tester.tap(navItem);
          await tester.pump(const Duration(milliseconds: 500));
        }
        await _screenshot(tester, name);
      }

      // Restore dark mode
      final darkModeBtn = find.byTooltip('Dark mode');
      if (darkModeBtn.evaluate().isNotEmpty) {
        await tester.tap(darkModeBtn);
        await tester.pump(const Duration(milliseconds: 300));
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
