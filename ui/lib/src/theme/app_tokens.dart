import 'package:flutter/material.dart';

// ── Spacing ───────────────────────────────────────────────────────────────────
// All spacing values are multiples of the 4px base unit.
// Use these everywhere — no raw numeric literals in widget files.

class AppSpacing {
  AppSpacing._();

  static const double xs  = 4.0;
  static const double sm  = 8.0;
  static const double md  = 16.0;
  static const double lg  = 24.0;
  static const double xl  = 32.0;
  static const double xxl = 48.0;

  // Convenience EdgeInsets
  static const EdgeInsets paddingXs  = EdgeInsets.all(xs);
  static const EdgeInsets paddingSm  = EdgeInsets.all(sm);
  static const EdgeInsets paddingMd  = EdgeInsets.all(md);
  static const EdgeInsets paddingLg  = EdgeInsets.all(lg);
  static const EdgeInsets paddingXl  = EdgeInsets.all(xl);

  static const EdgeInsets paddingHMd = EdgeInsets.symmetric(horizontal: md);
  static const EdgeInsets paddingVSm = EdgeInsets.symmetric(vertical: sm);
  static const EdgeInsets paddingHMdVSm =
      EdgeInsets.symmetric(horizontal: md, vertical: sm);

  // Card / section inner padding
  static const EdgeInsets cardPadding = EdgeInsets.all(md);
  static const EdgeInsets sectionPadding =
      EdgeInsets.symmetric(horizontal: md, vertical: sm);
}

// ── Border radius ─────────────────────────────────────────────────────────────

class AppRadius {
  AppRadius._();

  static const double xs   = 4.0;   // Chips, badges, tight accents
  static const double sm   = 8.0;   // Form fields, small containers
  static const double md   = 12.0;  // Buttons
  static const double lg   = 16.0;  // Cards, panels
  static const double xl   = 24.0;  // Bottom sheets, dialogs
  static const double pill = 999.0; // Fully rounded

  static BorderRadius get xsBr   => BorderRadius.circular(xs);
  static BorderRadius get smBr   => BorderRadius.circular(sm);
  static BorderRadius get mdBr   => BorderRadius.circular(md);
  static BorderRadius get lgBr   => BorderRadius.circular(lg);
  static BorderRadius get xlBr   => BorderRadius.circular(xl);
  static BorderRadius get pillBr => BorderRadius.circular(pill);
}

// ── Icon sizes ────────────────────────────────────────────────────────────────

class AppIconSize {
  AppIconSize._();

  static const double xs   = 12.0;  // Status dots, decorative indicators
  static const double sm   = 16.0;  // Inline icons, help, secondary actions
  static const double md   = 20.0;  // Navigation, standard action buttons
  static const double lg   = 24.0;  // Primary actions, prominent indicators
  static const double xl   = 32.0;  // Empty states, section icons
  static const double hero = 48.0;  // Onboarding, hero moments
}

// ── Elevation ─────────────────────────────────────────────────────────────────

class AppElevation {
  AppElevation._();

  static const double flat    = 0.0;  // Flat surfaces, sidebars
  static const double low     = 1.0;  // Cards at rest
  static const double mid     = 3.0;  // Cards on hover / selected
  static const double high    = 6.0;  // Dialogs, bottom sheets
  static const double overlay = 12.0; // Menus, dropdowns, tooltips
}

// ── Durations ─────────────────────────────────────────────────────────────────

class AppDuration {
  AppDuration._();

  static const Duration fast   = Duration(milliseconds: 120);
  static const Duration normal = Duration(milliseconds: 200);
  static const Duration slow   = Duration(milliseconds: 350);
}

// ── Colour palette ────────────────────────────────────────────────────────────
// Raw colour constants. Prefer theme.colorScheme.* in widgets where possible.
// Use these only for values that don't map to a Material colour role.

class AppColors {
  AppColors._();

  // Brand
  static const Color accent      = Color(0xFFDB8720); // Orange — primary interactive
  static const Color accentDark  = Color(0xFFC47218); // Darker orange for pressed states
  static const Color dataAccent  = Color(0xFF00C8FF); // Cyan — ties UI to hardware display

  // Dark theme surfaces
  static const Color darkBg       = Color(0xFF0D0D0F);
  static const Color darkSurface  = Color(0xFF16161A);
  static const Color darkElevated = Color(0xFF1E1E24);
  static const Color darkBorder   = Color(0xFF2A2A35);
  static const Color darkNavy     = Color(0xFF1A2236); // NavBar / rail

  // Light theme surfaces
  static const Color lightBg       = Color(0xFFF5F5F7);
  static const Color lightSurface  = Color(0xFFFFFFFF);
  static const Color lightElevated = Color(0xFFEEEEF2);
  static const Color lightBorder   = Color(0xFFE2E2E8);
  static const Color lightNavy     = Color(0xFF202C46);

  // Semantic — use via ColorScheme extension in widgets
  static const Color success = Color(0xFF30D158); // Dark theme
  static const Color successLight = Color(0xFF1A7F37); // Light theme
  static const Color warning = Color(0xFFFFD60A); // Dark theme
  static const Color warningLight = Color(0xFFB45309); // Light theme
  static const Color error   = Color(0xFFFF453A); // Dark theme
  static const Color errorLight = Color(0xFFDC2626); // Light theme

  // Text
  static const Color textPrimary   = Color(0xFFE5E5EA);
  static const Color textSecondary = Color(0xFFB8BDC2); // WCAG AA on dark
  static const Color textMuted     = Color(0xFF636366);
}

// ── Semantic colour extension ─────────────────────────────────────────────────
// Access via: Theme.of(context).colorScheme.success etc.

extension AppSemanticColors on ColorScheme {
  bool get _isDark => brightness == Brightness.dark;

  /// Positive / connected / ok state
  Color get success => _isDark ? AppColors.success : AppColors.successLight;

  /// Degraded / warning / slow state
  Color get warning => _isDark ? AppColors.warning : AppColors.warningLight;

  /// Critical / failed / error state — distinct from Material error role
  Color get critical => _isDark ? AppColors.error : AppColors.errorLight;

  /// The hardware display's cyan accent — for data visualisation in the UI
  Color get dataAccent => AppColors.dataAccent;

  /// Orange at 20% — selection backgrounds, active nav indicator fill
  Color get accentSubtle => AppColors.accent.withOpacity(0.18);

  /// The sidebar / rail background colour
  Color get railBackground =>
      _isDark ? AppColors.darkNavy : AppColors.lightNavy;
}
