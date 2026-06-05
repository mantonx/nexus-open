// Models generated from api/openapi.yaml via freezed + json_serializable.
// After editing this file run: dart run build_runner build --delete-conflicting-outputs

import 'package:freezed_annotation/freezed_annotation.dart';

part 'api_models.freezed.dart';
part 'api_models.g.dart';

// ── DisplayConfig ────────────────────────────────────────────────────────────

@freezed
class DisplayConfig with _$DisplayConfig {
  const factory DisplayConfig({
    @JsonKey(name: 'font_family') @Default('GoRegular') String fontFamily,
    @JsonKey(name: 'font_size') @Default(11.0) double fontSize,
    @JsonKey(name: 'time_font_size') @Default(14.0) double timeFontSize,
    @Default('dashboard') String layout,
    @JsonKey(name: 'date_format') @Default('MM/DD/YYYY') String dateFormat,
  }) = _DisplayConfig;

  factory DisplayConfig.fromJson(Map<String, dynamic> json) =>
      _$DisplayConfigFromJson(json);
}

// ── NexusConfig (Config schema) ──────────────────────────────────────────────

@freezed
class NexusConfig with _$NexusConfig {
  const factory NexusConfig({
    @Default('') String location,
    @JsonKey(name: 'time_format') @Default('24h') String timeFormat,
    @Default('imperial') String unit,
    @JsonKey(name: 'background_color') @Default('#000000') String backgroundColor,
    @JsonKey(name: 'background_image') @Default('background.png') String backgroundImage,
    @JsonKey(name: 'text_color') @Default('#FFFFFF') String textColor,
    @JsonKey(name: 'image_paths') @Default([]) List<String> imagePaths,
    @Default(DisplayConfig()) DisplayConfig display,
  }) = _NexusConfig;

  factory NexusConfig.fromJson(Map<String, dynamic> json) =>
      _$NexusConfigFromJson(json);
}

// ── DeviceInfo ───────────────────────────────────────────────────────────────

@freezed
class DeviceInfo with _$DeviceInfo {
  const factory DeviceInfo({
    @Default('iCUE Nexus') String model,
    @Default('') String firmware,
    @JsonKey(name: 'vendorId') @Default('0x1b1c') String vendorId,
    @JsonKey(name: 'productId') @Default('0x1b8e') String productId,
    @JsonKey(name: 'connect_error') String? connectError,
  }) = _DeviceInfo;

  factory DeviceInfo.fromJson(Map<String, dynamic> json) =>
      _$DeviceInfoFromJson(json);
}

// ── BrightnessRequest ────────────────────────────────────────────────────────

@freezed
class BrightnessRequest with _$BrightnessRequest {
  const factory BrightnessRequest({
    @Default(75) int brightness,
  }) = _BrightnessRequest;

  factory BrightnessRequest.fromJson(Map<String, dynamic> json) =>
      _$BrightnessRequestFromJson(json);
}

// ── ApiError ─────────────────────────────────────────────────────────────────

@freezed
class ApiError with _$ApiError {
  const factory ApiError({
    @Default('error') String error,
    String? message,
  }) = _ApiError;

  factory ApiError.fromJson(Map<String, dynamic> json) =>
      _$ApiErrorFromJson(json);
}

// ── Layout models ─────────────────────────────────────────────────────────────
// Plain Dart — not freezed, because the editor mutates these in place.

class LayoutZone {
  String id;
  int pageId;
  int ord;
  int widthPx;
  String plugin;
  int refreshMs;
  String align;
  Map<String, dynamic> config;
  Map<String, dynamic> themeOverride;

  LayoutZone({
    required this.id,
    required this.pageId,
    required this.ord,
    required this.widthPx,
    this.plugin = 'builtin:placeholder',
    this.refreshMs = 2000,
    this.align = 'center',
    Map<String, dynamic>? config,
    Map<String, dynamic>? themeOverride,
  })  : config = config ?? {},
        themeOverride = themeOverride ?? {};

  factory LayoutZone.fromJson(Map<String, dynamic> j) => LayoutZone(
        id: j['id'] as String,
        pageId: (j['page_id'] as num).toInt(),
        ord: (j['ord'] as num).toInt(),
        widthPx: (j['width_px'] as num).toInt(),
        plugin: j['plugin'] as String? ?? 'builtin:placeholder',
        refreshMs: (j['refresh_ms'] as num?)?.toInt() ?? 2000,
        align: j['align'] as String? ?? 'center',
        config: (j['config'] as Map<String, dynamic>?) ?? {},
        themeOverride: (j['theme_override'] as Map<String, dynamic>?) ?? {},
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'page_id': pageId,
        'ord': ord,
        'width_px': widthPx,
        'plugin': plugin,
        'refresh_ms': refreshMs,
        'align': align,
        'config': config,
        'theme_override': themeOverride,
      };

  LayoutZone copyWith({
    String? id,
    int? pageId,
    int? ord,
    int? widthPx,
    String? plugin,
    int? refreshMs,
    String? align,
  }) =>
      LayoutZone(
        id: id ?? this.id,
        pageId: pageId ?? this.pageId,
        ord: ord ?? this.ord,
        widthPx: widthPx ?? this.widthPx,
        plugin: plugin ?? this.plugin,
        refreshMs: refreshMs ?? this.refreshMs,
        align: align ?? this.align,
        config: config,
        themeOverride: themeOverride,
      );
}

class LayoutPage {
  int id;
  String name;
  int ord;
  List<LayoutZone> zones;

  LayoutPage({
    required this.id,
    required this.name,
    required this.ord,
    required this.zones,
  });

  factory LayoutPage.fromJson(Map<String, dynamic> j) => LayoutPage(
        id: (j['id'] as num).toInt(),
        name: j['name'] as String,
        ord: (j['ord'] as num).toInt(),
        zones: ((j['zones'] as List<dynamic>?) ?? [])
            .map((z) => LayoutZone.fromJson(z as Map<String, dynamic>))
            .toList(),
      );

  int get totalWidth => zones.fold(0, (sum, z) => sum + z.widthPx);
  bool get isValid => totalWidth == 640;
}

// ── ApiSuccess ───────────────────────────────────────────────────────────────

@freezed
class ApiSuccess with _$ApiSuccess {
  const factory ApiSuccess({
    @Default('success') String status,
    String? message,
    @JsonKey(includeIfNull: false) dynamic data,
  }) = _ApiSuccess;

  factory ApiSuccess.fromJson(Map<String, dynamic> json) =>
      _$ApiSuccessFromJson(json);
}
