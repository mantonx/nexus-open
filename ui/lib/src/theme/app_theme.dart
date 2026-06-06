import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'app_tokens.dart';

export 'app_tokens.dart';

/// Application theme — hardware-first dark aesthetic.
///
/// Depth hierarchy (dark): rail (darkest) → bg → surface → elevated → overlay.
/// Two-accent system: [AppColors.accent] blue for interactive UI elements,
/// [AppColors.hardwareAccent] amber only for hardware-display–related widgets.
///
/// Access semantic colours via [AppSemanticColors]:
///   Theme.of(context).colorScheme.success
///   Theme.of(context).colorScheme.hardwareAccent
class AppTheme {
  AppTheme._();

  // ── Dark theme ─────────────────────────────────────────────────────────────

  static ThemeData get darkTheme {
    const cs = ColorScheme.dark(
      // Surfaces — depth: rail(0A) → bg(13) → surface(1C) → elevated(25)
      surface:                 AppColors.darkBg,
      surfaceContainerLowest:  AppColors.darkRail,
      surfaceContainer:        AppColors.darkSurface,
      surfaceContainerHigh:    AppColors.darkElevated,
      surfaceContainerHighest: AppColors.darkOverlay,
      // Brand — blue for interactive
      primary:                 AppColors.accent,
      onPrimary:               Colors.white,
      primaryContainer:        Color(0xFF0F2D52),
      onPrimaryContainer:      AppColors.accent,
      // Secondary — hardware amber for display-related elements
      secondary:               AppColors.hardwareAccent,
      onSecondary:             Colors.white,
      secondaryContainer:      Color(0xFF3D2200),
      onSecondaryContainer:    AppColors.hardwareAccent,
      // Tertiary — data visualisation cyan
      tertiary:                AppColors.dataAccent,
      onTertiary:              AppColors.darkBg,
      // State
      error:                   AppColors.error,
      onError:                 Colors.white,
      // Text
      onSurface:               AppColors.textPrimary,
      onSurfaceVariant:        AppColors.textSecondary,
      // Outlines
      outline:                 AppColors.darkBorder,
      outlineVariant:          Color(0xFF1E1E24),
      // Inverse
      inverseSurface:          AppColors.lightSurface,
      onInverseSurface:        AppColors.darkBg,
    );
    return _build(cs, Brightness.dark);
  }

  // ── Light theme ────────────────────────────────────────────────────────────

  static ThemeData get lightTheme {
    const cs = ColorScheme.light(
      surface:                 AppColors.lightBg,
      surfaceContainerLowest:  AppColors.lightRail,
      surfaceContainer:        AppColors.lightSurface,
      surfaceContainerHigh:    AppColors.lightElevated,
      surfaceContainerHighest: AppColors.lightElevated,
      primary:                 AppColors.accent,
      onPrimary:               Colors.white,
      primaryContainer:        Color(0xFFD6E8FF),
      onPrimaryContainer:      Color(0xFF003066),
      secondary:               AppColors.hardwareAccent,
      onSecondary:             Colors.white,
      secondaryContainer:      Color(0xFFFFE4C4),
      onSecondaryContainer:    AppColors.hardwareAccentDark,
      tertiary:                AppColors.dataAccent,
      onTertiary:              Colors.white,
      error:                   AppColors.errorLight,
      onError:                 Colors.white,
      onSurface:               AppColors.lightNavy,
      onSurfaceVariant:        AppColors.textMuted,
      outline:                 AppColors.lightBorder,
      outlineVariant:          AppColors.lightElevated,
      inverseSurface:          AppColors.darkSurface,
      onInverseSurface:        AppColors.textPrimary,
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
        backgroundColor: isDark ? AppColors.darkRail : AppColors.lightNavy,
        foregroundColor: Colors.white,
        elevation:       AppElevation.flat,
        scrolledUnderElevation: AppElevation.low,
        titleTextStyle: textTheme.titleMedium?.copyWith(color: Colors.white),
      ),

      // ── Card ─────────────────────────────────────────────────────────────
      cardTheme: CardThemeData(
        color:     isDark ? AppColors.darkSurface : AppColors.lightSurface,
        elevation: AppElevation.flat,
        shape: RoundedRectangleBorder(
          borderRadius: AppRadius.lgBr,
          side: BorderSide(
            color: isDark ? AppColors.darkBorder : AppColors.lightBorder,
            width: 1,
          ),
        ),
        margin: const EdgeInsets.only(bottom: AppSpacing.sm),
      ),

      // ── NavigationRail ────────────────────────────────────────────────────
      // Rail bg is the darkest surface — matches hardware housing colour.
      // Selected: white icon with subtle blue indicator fill.
      // Unselected: dimmed white (no orange).
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: isDark ? AppColors.darkRail : AppColors.lightNavy,
        selectedIconTheme: const IconThemeData(
          color: Colors.white,
          size:  AppIconSize.md,
        ),
        unselectedIconTheme: IconThemeData(
          color: Colors.white.withValues(alpha: 0.4),
          size:  AppIconSize.md,
        ),
        selectedLabelTextStyle: textTheme.labelSmall?.copyWith(
          color:      Colors.white,
          fontWeight: FontWeight.w600,
        ),
        unselectedLabelTextStyle: textTheme.labelSmall?.copyWith(
          color: Colors.white.withValues(alpha: 0.4),
        ),
        indicatorColor: AppColors.accent.withValues(alpha: 0.18),
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
        filled:    true,
        fillColor: isDark ? AppColors.darkElevated : AppColors.lightElevated,
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
            vertical:   AppSpacing.sm,
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
            vertical:   AppSpacing.sm,
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
              : AppColors.lightNavy.withValues(alpha: 0.7),
        ),
      ),

      // ── Slider ────────────────────────────────────────────────────────────
      sliderTheme: SliderThemeData(
        activeTrackColor:   AppColors.accent,
        inactiveTrackColor: AppColors.accent.withValues(alpha: 0.2),
        thumbColor:         AppColors.accent,
        overlayColor:       AppColors.accent.withValues(alpha: 0.16),
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
            : AppColors.lightNavy.withValues(alpha: 0.5),
        tilePadding:      AppSpacing.paddingHMd,
        childrenPadding:  EdgeInsets.zero,
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
        titleTextStyle:   textTheme.headlineSmall,
        contentTextStyle: textTheme.bodyMedium,
      ),

      // ── Tooltip ───────────────────────────────────────────────────────────
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: isDark ? AppColors.darkOverlay : AppColors.lightNavy,
          borderRadius: AppRadius.xsBr,
        ),
        textStyle: textTheme.labelSmall?.copyWith(
          color: isDark ? AppColors.textPrimary : Colors.white,
        ),
        waitDuration: AppDuration.slow,
      ),

      // ── Tab bar ───────────────────────────────────────────────────────────
      tabBarTheme: TabBarThemeData(
        labelColor:           AppColors.accent,
        unselectedLabelColor: Colors.white.withValues(alpha: 0.5),
        indicatorColor:       AppColors.accent,
        labelStyle:           textTheme.labelLarge,
        unselectedLabelStyle: textTheme.labelLarge,
      ),

      // ── Linear progress ───────────────────────────────────────────────────
      progressIndicatorTheme: ProgressIndicatorThemeData(
        color:              AppColors.accent,
        linearTrackColor:   AppColors.accent.withValues(alpha: 0.15),
        circularTrackColor: AppColors.accent.withValues(alpha: 0.15),
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
                ? AppColors.accent.withValues(alpha: 0.4)
                : null),
      ),

      // ── Material Banner ───────────────────────────────────────────────────
      bannerTheme: MaterialBannerThemeData(
        backgroundColor: isDark
            ? AppColors.darkElevated
            : const Color(0xFFECF4FF),
        contentTextStyle: textTheme.bodyMedium,
        elevation: AppElevation.flat,
      ),
    );
  }

  // ── Typography ─────────────────────────────────────────────────────────────
  // Poppins for display/headline/title — identity-carrying, large text.
  // Inter for body/label — better hinting at small sizes on Linux HiDPI.

  static TextTheme _buildTextTheme(Brightness brightness) {
    final isDark = brightness == Brightness.dark;
    final base = isDark ? AppColors.textPrimary : AppColors.lightNavy;
    final muted = isDark ? AppColors.textSecondary : AppColors.textMuted;

    TextStyle poppins(double size, FontWeight weight, Color color) =>
        GoogleFonts.poppins(fontSize: size, fontWeight: weight, color: color);
    TextStyle inter(double size, FontWeight weight, Color color) =>
        GoogleFonts.inter(fontSize: size, fontWeight: weight, color: color);

    return TextTheme(
      displaySmall:   poppins(24, FontWeight.w600, base),
      headlineLarge:  poppins(22, FontWeight.w600, base),
      headlineMedium: poppins(20, FontWeight.w600, base),
      headlineSmall:  poppins(18, FontWeight.w600, base),
      titleLarge:     poppins(16, FontWeight.w600, base),
      titleMedium:    poppins(15, FontWeight.w600, base),
      titleSmall:     inter(14, FontWeight.w500, base),
      bodyLarge:      inter(14, FontWeight.w400, base),
      bodyMedium:     inter(13, FontWeight.w400, muted),
      bodySmall:      inter(11, FontWeight.w400, muted),
      labelLarge:     inter(13, FontWeight.w500, base),
      labelMedium:    inter(12, FontWeight.w500, muted),
      labelSmall:     inter(10, FontWeight.w500, muted),
    );
  }

  // ── Test theme ─────────────────────────────────────────────────────────────

  static ThemeData get testTheme => ThemeData(
        useMaterial3: true,
        colorScheme: const ColorScheme.dark(
          primary:   AppColors.accent,
          secondary: AppColors.hardwareAccent,
          tertiary:  AppColors.dataAccent,
          surface:   AppColors.darkSurface,
        ),
      );
}
