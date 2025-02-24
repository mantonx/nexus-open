import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

/// Defines the app's theme including colors, typography and component styles
class AppTheme {
  static ThemeData get theme {
    final textTheme = GoogleFonts.poppinsTextTheme();

    return ThemeData(
      primaryColor: const Color(0xFF202C46),
      scaffoldBackgroundColor: Colors.white,
      colorScheme: const ColorScheme.light(
        primary: Color(0xFF202C46),
        secondary: Color(0xFF3984B2),
        tertiary: Color(0xFFDB8720),
        surface: Colors.white,
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: Color(0xFF202C46),
        elevation: 2,
      ),
      cardTheme: CardThemeData(
        color: Colors.white,
        elevation: 2,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: Color(0xFFEEEEEE)),
        ),
      ),
      tabBarTheme: const TabBarThemeData(
        labelColor: Colors.white,
        unselectedLabelColor: Color(0xAAFFFFFF),
        indicatorColor: Color(0xFFF1A326),
      ),
      dividerTheme: const DividerThemeData(
        color: Color(0xFFEEEEEE),
      ),
      textTheme: textTheme.copyWith(
        titleLarge: textTheme.titleLarge?.copyWith(
          color: const Color(0xFF202C46),
          fontWeight: FontWeight.w600,
        ),
        bodyLarge: textTheme.bodyLarge?.copyWith(
          color: const Color(0xFF202C46),
          fontWeight: FontWeight.w500,
        ),
        bodyMedium: textTheme.bodyMedium?.copyWith(
          color: const Color(0xFF3984B2),
          fontWeight: FontWeight.normal,
        ),
        labelLarge: textTheme.labelLarge?.copyWith(
          color: const Color(0xFF202C46),
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}
