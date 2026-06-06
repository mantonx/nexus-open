import 'package:integration_test/integration_test_driver.dart';

// Driver for screenshot tour.
// Delegates screenshot capture to scripts/flutter-screenshot.py
// since Linux desktop does not support binding.takeScreenshot().
Future<void> main() => integrationDriver();
