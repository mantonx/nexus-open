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
  static const EdgeInsets cardPadding    = EdgeInsets.all(md);
  static const EdgeInsets sectionPadding =
      EdgeInsets.symmetric(horizontal: md, vertical: sm);
}

// ── Border radius ─────────────────────────────────────────────────────────────

class AppRadius {
  AppRadius._();

  static const double xs   = 4.0;
  static const double sm   = 8.0;
  static const double md   = 12.0;
  static const double lg   = 16.0;
  static const double xl   = 24.0;
  static const double pill = 999.0;

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

  static const double xs   = 12.0;
  static const double sm   = 16.0;
  static const double md   = 20.0;
  static const double lg   = 24.0;
  static const double xl   = 32.0;
  static const double hero = 48.0;
}

// ── Elevation ─────────────────────────────────────────────────────────────────

class AppElevation {
  AppElevation._();

  static const double flat    = 0.0;
  static const double low     = 1.0;
  static const double mid     = 3.0;
  static const double high    = 6.0;
  static const double overlay = 12.0;
}

// ── Durations ─────────────────────────────────────────────────────────────────

class AppDuration {
  AppDuration._();

  static const Duration fast   = Duration(milliseconds: 120);
  static const Duration normal = Duration(milliseconds: 200);
  static const Duration slow   = Duration(milliseconds: 350);
}

// ── Colour palette ────────────────────────────────────────────────────────────
//
// Two-accent system:
//   accent       — electric blue for interactive UI elements (buttons, focus,
//                  selection). Reads as "software" — tappable, actionable.
//   hardwareAccent — amber retained for hardware-display–related elements:
//                  the preview frame border, zone colours, severity indicators.
//                  Ties the UI visually to the physical device.
//
// Depth hierarchy (dark, outermost-to-innermost):
//   darkRail  →  darkBg  →  darkSurface  →  darkElevated  →  darkOverlay

class AppColors {
  AppColors._();

  // ── Interactive accent (software UI) ─────────────────────────────────────
  static const Color accent      = Color(0xFF4F9EFF); // Electric blue
  static const Color accentDark  = Color(0xFF2D7FE0); // Pressed state
  static const Color accentSubtle = Color(0x1A4F9EFF); // 10% — selection bg

  // ── Hardware accent (display / device elements only) ─────────────────────
  static const Color hardwareAccent      = Color(0xFFE07B20); // Amber
  static const Color hardwareAccentDark  = Color(0xFFC46618); // Pressed

  // ── Data visualisation accent (ties to hardware display rendering) ────────
  static const Color dataAccent = Color(0xFF00C8FF); // Cyan

  // ── Dark theme surfaces (outermost → innermost) ───────────────────────────
  // Rail is the darkest — it should recede, matching the hardware housing.
  static const Color darkRail     = Color(0xFF0A0A0C); // Nav rail — near-black
  static const Color darkBg       = Color(0xFF131316); // Page background
  static const Color darkSurface  = Color(0xFF1C1C21); // Cards, sections
  static const Color darkElevated = Color(0xFF252529); // Dropdowns, expanded
  static const Color darkOverlay  = Color(0xFF2E2E34); // Menus, tooltips
  static const Color darkBorder   = Color(0xFF2A2A32); // Dividers, outlines

  // ── Light theme surfaces ──────────────────────────────────────────────────
  static const Color lightRail     = Color(0xFF1A2236); // Keep navy for light
  static const Color lightBg       = Color(0xFFF2F2F5);
  static const Color lightSurface  = Color(0xFFFFFFFF);
  static const Color lightElevated = Color(0xFFECECF0);
  static const Color lightBorder   = Color(0xFFDEDEE6);
  static const Color lightNavy     = Color(0xFF202C46);

  // ── Semantic colours ──────────────────────────────────────────────────────
  static const Color success      = Color(0xFF30D158);
  static const Color successLight = Color(0xFF1A7F37);
  static const Color warning      = Color(0xFFFFD60A);
  static const Color warningLight = Color(0xFFB45309);
  static const Color error        = Color(0xFFFF453A);
  static const Color errorLight   = Color(0xFFDC2626);

  // ── Text ──────────────────────────────────────────────────────────────────
  static const Color textPrimary   = Color(0xFFE8E8ED);
  static const Color textSecondary = Color(0xFFAAADB3);
  static const Color textMuted     = Color(0xFF5A5A62);
}

// ── Semantic colour extension ─────────────────────────────────────────────────

extension AppSemanticColors on ColorScheme {
  bool get _isDark => brightness == Brightness.dark;

  Color get success    => _isDark ? AppColors.success      : AppColors.successLight;
  Color get warning    => _isDark ? AppColors.warning      : AppColors.warningLight;
  Color get critical   => _isDark ? AppColors.error        : AppColors.errorLight;
  Color get dataAccent => AppColors.dataAccent;

  // Hardware accent — amber — for display-related elements only.
  Color get hardwareAccent => AppColors.hardwareAccent;

  // Interactive accent subtle fill — 10% blue for selected backgrounds.
  Color get accentSubtle => AppColors.accentSubtle;

  // The sidebar / rail background colour.
  Color get railBackground =>
      _isDark ? AppColors.darkRail : AppColors.lightRail;
}
