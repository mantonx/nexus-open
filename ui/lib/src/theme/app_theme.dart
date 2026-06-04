import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'app_tokens.dart';

export 'app_tokens.dart';

/// Application theme — dark-first dashboard aesthetic.
///
/// Both themes are built with [useMaterial3] and share the same
/// [AppColors.accent] orange as the seed / tertiary colour.
/// Access semantic colours via the [AppSemanticColors] extension:
///   Theme.of(context).colorScheme.success
///   Theme.of(context).colorScheme.warning
class AppTheme {
  AppTheme._();

  // ── Dark theme (primary experience) ───────────────────────────────────────

  static ThemeData get darkTheme {
    final cs = const ColorScheme.dark(
      // Surfaces
      surface:                  AppColors.darkSurface,
      surfaceContainerLowest:   AppColors.darkBg,
      surfaceContainer:         AppColors.darkSurface,
      surfaceContainerHigh:     AppColors.darkElevated,
      surfaceContainerHighest:  AppColors.darkElevated,
      // Brand
      primary:                  AppColors.accent,
      onPrimary:                Colors.white,
      primaryContainer:         Color(0xFF3D2600),
      onPrimaryContainer:       AppColors.accent,
      secondary:                AppColors.dataAccent,
      onSecondary:              AppColors.darkBg,
      secondaryContainer:       Color(0xFF003D4D),
      onSecondaryContainer:     AppColors.dataAccent,
      tertiary:                 AppColors.accent,
      onTertiary:               Colors.white,
      // State / semantic
      error:                    AppColors.error,
      onError:                  Colors.white,
      // Text / icon on surfaces
      onSurface:                AppColors.textPrimary,
      onSurfaceVariant:         AppColors.textSecondary,
      // Outlines
      outline:                  AppColors.darkBorder,
      outlineVariant:           Color(0xFF1E1E24),
      // Inverse (used by snackbars, tooltips)
      inverseSurface:           AppColors.lightSurface,
      onInverseSurface:         AppColors.darkBg,
    );

    return _build(cs, Brightness.dark);
  }

  // ── Light theme ────────────────────────────────────────────────────────────

  static ThemeData get lightTheme {
    final cs = const ColorScheme.light(
      // Surfaces
      surface:                  AppColors.lightSurface,
      surfaceContainerLowest:   AppColors.lightBg,
      surfaceContainer:         AppColors.lightSurface,
      surfaceContainerHigh:     AppColors.lightElevated,
      surfaceContainerHighest:  AppColors.lightElevated,
      // Brand
      primary:                  AppColors.lightNavy,
      onPrimary:                Colors.white,
      primaryContainer:         Color(0xFFDDE3F5),
      onPrimaryContainer:       AppColors.lightNavy,
      secondary:                AppColors.accent,
      onSecondary:              Colors.white,
      secondaryContainer:       Color(0xFFFFF0D9),
      onSecondaryContainer:     AppColors.accentDark,
      tertiary:                 AppColors.accent,
      onTertiary:               Colors.white,
      // State / semantic
      error:                    AppColors.errorLight,
      onError:                  Colors.white,
      // Text / icon on surfaces
      onSurface:                AppColors.lightNavy,
      onSurfaceVariant:         AppColors.textMuted,
      // Outlines
      outline:                  AppColors.lightBorder,
      outlineVariant:           AppColors.lightElevated,
      // Inverse
      inverseSurface:           AppColors.darkSurface,
      onInverseSurface:         AppColors.textPrimary,
    );

    return _build(cs, Brightness.light);
  }

  // ── Shared builder ─────────────────────────────────────────────────────────

  static ThemeData _build(ColorScheme cs, Brightness brightness) {
    final isDark = brightness == Brightness.dark;
    final textTheme = _buildTextTheme(brightness);

    return ThemeData(
      useMaterial3:            true,
      colorScheme:             cs,
      brightness:              brightness,
      scaffoldBackgroundColor: isDark ? AppColors.darkBg : AppColors.lightBg,
      textTheme:               textTheme,

      // ── AppBar ────────────────────────────────────────────────────────────
      appBarTheme: AppBarTheme(
        backgroundColor: isDark ? AppColors.darkNavy : AppColors.lightNavy,
        foregroundColor: Colors.white,
        elevation:       AppElevation.flat,
        scrolledUnderElevation: AppElevation.low,
        titleTextStyle: textTheme.titleMedium?.copyWith(color: Colors.white),
      ),

      // ── Card ─────────────────────────────────────────────────────────────
      cardTheme: CardThemeData(
        color:     isDark ? AppColors.darkSurface : AppColors.lightSurface,
        elevation: AppElevation.flat,
        shape:     RoundedRectangleBorder(
          borderRadius: AppRadius.lgBr,
          side: BorderSide(
            color: isDark ? AppColors.darkBorder : AppColors.lightBorder,
            width: 1,
          ),
        ),
        margin: const EdgeInsets.only(bottom: AppSpacing.sm),
      ),

      // ── NavigationRail ────────────────────────────────────────────────────
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: isDark ? AppColors.darkNavy : AppColors.lightNavy,
        selectedIconTheme: const IconThemeData(
          color: AppColors.accent,
          size: AppIconSize.md,
        ),
        unselectedIconTheme: IconThemeData(
          color: Colors.white.withOpacity(0.5),
          size: AppIconSize.md,
        ),
        selectedLabelTextStyle: textTheme.labelSmall?.copyWith(
          color: AppColors.accent,
          fontWeight: FontWeight.w600,
        ),
        unselectedLabelTextStyle: textTheme.labelSmall?.copyWith(
          color: Colors.white.withOpacity(0.5),
        ),
        indicatorColor: AppColors.accent.withOpacity(0.18),
        indicatorShape: RoundedRectangleBorder(
          borderRadius: AppRadius.smBr,
        ),
        elevation: AppElevation.flat,
      ),

      // ── Divider ───────────────────────────────────────────────────────────
      dividerTheme: DividerThemeData(
        color:     isDark ? AppColors.darkBorder : AppColors.lightBorder,
        thickness: 1,
        space:     1,
      ),

      // ── Input / form fields ───────────────────────────────────────────────
      inputDecorationTheme: InputDecorationTheme(
        filled:      true,
        fillColor:   isDark ? AppColors.darkElevated : AppColors.lightElevated,
        border: OutlineInputBorder(
          borderRadius: AppRadius.smBr,
          borderSide: BorderSide(
            color: isDark ? AppColors.darkBorder : AppColors.lightBorder,
          ),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: AppRadius.smBr,
          borderSide: BorderSide(
            color: isDark ? AppColors.darkBorder : AppColors.lightBorder,
          ),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: AppRadius.smBr,
          borderSide: const BorderSide(color: AppColors.accent, width: 2),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: AppRadius.smBr,
          borderSide: BorderSide(
            color: isDark ? AppColors.error : AppColors.errorLight,
          ),
        ),
        contentPadding: AppSpacing.paddingHMdVSm,
        hintStyle: TextStyle(
          color: isDark ? AppColors.textMuted : const Color(0xFF9CA3AF),
        ),
      ),

      // ── Buttons ───────────────────────────────────────────────────────────
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          backgroundColor: AppColors.accent,
          foregroundColor: Colors.white,
          shape: RoundedRectangleBorder(borderRadius: AppRadius.mdBr),
          padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.md,
            vertical: AppSpacing.sm,
          ),
          textStyle: textTheme.labelLarge,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: AppColors.accent,
          side: const BorderSide(color: AppColors.accent),
          shape: RoundedRectangleBorder(borderRadius: AppRadius.mdBr),
          padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.md,
            vertical: AppSpacing.sm,
          ),
          textStyle: textTheme.labelLarge,
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: AppColors.accent,
          textStyle: textTheme.labelLarge,
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: IconButton.styleFrom(
          foregroundColor: isDark
              ? AppColors.textSecondary
              : AppColors.lightNavy.withOpacity(0.7),
        ),
      ),

      // ── Slider ────────────────────────────────────────────────────────────
      sliderTheme: SliderThemeData(
        activeTrackColor:   AppColors.accent,
        inactiveTrackColor: AppColors.accent.withOpacity(0.2),
        thumbColor:         AppColors.accent,
        overlayColor:       AppColors.accent.withOpacity(0.16),
        valueIndicatorColor: AppColors.accent,
        valueIndicatorTextStyle: textTheme.labelSmall?.copyWith(
          color: Colors.white,
        ),
      ),

      // ── ExpansionTile ─────────────────────────────────────────────────────
      expansionTileTheme: ExpansionTileThemeData(
        backgroundColor:          Colors.transparent,
        collapsedBackgroundColor: Colors.transparent,
        iconColor:        AppColors.accent,
        collapsedIconColor: isDark
            ? AppColors.textSecondary
            : AppColors.lightNavy.withOpacity(0.5),
        tilePadding: AppSpacing.paddingHMd,
        childrenPadding: EdgeInsets.zero,
      ),

      // ── SnackBar ──────────────────────────────────────────────────────────
      snackBarTheme: SnackBarThemeData(
        backgroundColor: isDark ? AppColors.darkElevated : AppColors.lightNavy,
        contentTextStyle: textTheme.bodyMedium?.copyWith(
          color: isDark ? AppColors.textPrimary : Colors.white,
        ),
        shape: RoundedRectangleBorder(borderRadius: AppRadius.smBr),
        behavior: SnackBarBehavior.floating,
      ),

      // ── Dialog ────────────────────────────────────────────────────────────
      dialogTheme: DialogThemeData(
        backgroundColor: isDark ? AppColors.darkElevated : AppColors.lightSurface,
        shape: RoundedRectangleBorder(borderRadius: AppRadius.lgBr),
        elevation: AppElevation.high,
        titleTextStyle: textTheme.headlineSmall,
        contentTextStyle: textTheme.bodyMedium,
      ),

      // ── Tooltip ───────────────────────────────────────────────────────────
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: isDark ? AppColors.darkElevated : AppColors.lightNavy,
          borderRadius: AppRadius.xsBr,
        ),
        textStyle: textTheme.labelSmall?.copyWith(
          color: isDark ? AppColors.textPrimary : Colors.white,
        ),
        waitDuration: AppDuration.slow,
      ),

      // ── Tab bar ───────────────────────────────────────────────────────────
      tabBarTheme: TabBarThemeData(
        labelColor:         AppColors.accent,
        unselectedLabelColor: Colors.white.withOpacity(0.5),
        indicatorColor:     AppColors.accent,
        labelStyle:         textTheme.labelLarge,
        unselectedLabelStyle: textTheme.labelLarge,
      ),

      // ── Linear progress ───────────────────────────────────────────────────
      progressIndicatorTheme: ProgressIndicatorThemeData(
        color:            AppColors.accent,
        linearTrackColor: AppColors.accent.withOpacity(0.15),
        circularTrackColor: AppColors.accent.withOpacity(0.15),
      ),

      // ── Checkbox / Radio / Switch ─────────────────────────────────────────
      checkboxTheme: CheckboxThemeData(
        fillColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.selected) ? AppColors.accent : null),
        shape: RoundedRectangleBorder(borderRadius: AppRadius.xsBr),
      ),
      switchTheme: SwitchThemeData(
        thumbColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.selected) ? AppColors.accent : null),
        trackColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.selected)
                ? AppColors.accent.withOpacity(0.4)
                : null),
      ),

      // ── Material Banner ───────────────────────────────────────────────────
      bannerTheme: MaterialBannerThemeData(
        backgroundColor: isDark
            ? AppColors.darkElevated
            : const Color(0xFFFFF8EC),
        contentTextStyle: textTheme.bodyMedium,
        elevation: AppElevation.flat,
      ),
    );
  }

  // ── Typography ─────────────────────────────────────────────────────────────

  static TextTheme _buildTextTheme(Brightness brightness) {
    final isDark = brightness == Brightness.dark;
    final base = isDark ? AppColors.textPrimary : AppColors.lightNavy;
    final muted = isDark ? AppColors.textSecondary : AppColors.textMuted;

    // Base font — Poppins for headings, system default fallback for body
    // to keep rendering crisp at small sizes on HiDPI Linux displays.
    TextStyle poppins(double size, FontWeight weight, Color color) =>
        GoogleFonts.poppins(fontSize: size, fontWeight: weight, color: color);

    return TextTheme(
      // Display — not currently used but defined for completeness
      displaySmall: poppins(24, FontWeight.w600, base),

      // Section headers, onboarding titles
      headlineLarge:  poppins(22, FontWeight.w600, base),
      headlineMedium: poppins(20, FontWeight.w600, base),
      headlineSmall:  poppins(18, FontWeight.w600, base),

      // Card headers, tab pane titles
      titleLarge:  poppins(16, FontWeight.w600, base),
      titleMedium: poppins(15, FontWeight.w600, base),
      titleSmall:  poppins(14, FontWeight.w500, base),

      // Body text
      bodyLarge:  poppins(14, FontWeight.w400, base),
      bodyMedium: poppins(13, FontWeight.w400, muted),
      bodySmall:  poppins(11, FontWeight.w400, muted),

      // Labels — buttons, form labels, nav items
      labelLarge:  poppins(13, FontWeight.w500, base),
      labelMedium: poppins(12, FontWeight.w500, muted),
      labelSmall:  poppins(10, FontWeight.w500, muted),
    );
  }

  // ── Test theme ─────────────────────────────────────────────────────────────
  // Avoids google_fonts compile issues in test environments.

  static ThemeData get testTheme => ThemeData(
        useMaterial3: true,
        colorScheme: const ColorScheme.dark(
          primary:   AppColors.accent,
          secondary: AppColors.dataAccent,
          tertiary:  AppColors.accent,
          surface:   AppColors.darkSurface,
        ),
      );
}
