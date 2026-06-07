// Models generated from api/openapi.yaml via freezed + json_serializable.
// After editing this file run: dart run build_runner build --delete-conflicting-outputs

// ignore_for_file: invalid_annotation_target

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
    @Default('Jersey City') String location,
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
    @Default('') String manufacturer,
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
        // Draft API uses 'width'; DB API uses 'width_px'. Accept both.
        pageId: (j['page_id'] as num?)?.toInt() ?? 0,
        ord: (j['ord'] as num?)?.toInt() ?? 0,
        widthPx: ((j['width_px'] ?? j['width']) as num?)?.toInt() ?? 0,
        plugin: j['plugin'] as String? ?? 'builtin:placeholder',
        refreshMs: (j['refresh_ms'] as num?)?.toInt() ?? 2000,
        align: j['align'] as String? ?? 'center',
        config: (j['plugin_config'] as Map<String, dynamic>?)
            ?? (j['config'] as Map<String, dynamic>?)
            ?? {},
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
        // Draft pages have no 'id' or 'ord' — default to 0.
        id: (j['id'] as num?)?.toInt() ?? 0,
        name: j['name'] as String? ?? '',
        ord: (j['ord'] as num?)?.toInt() ?? 0,
        zones: ((j['zones'] as List<dynamic>?) ?? [])
            .map((z) => LayoutZone.fromJson(z as Map<String, dynamic>))
            .toList(),
      );

  int get totalWidth => zones.fold(0, (sum, z) => sum + z.widthPx);
  bool get isValid => totalWidth == 640;
}

// ── Plugin catalog ───────────────────────────────────────────────────────────

class PluginFieldOption {
  final String value;
  final String label;
  const PluginFieldOption({required this.value, required this.label});
  factory PluginFieldOption.fromJson(Map<String, dynamic> j) =>
      PluginFieldOption(
        value: j['value'] as String,
        label: j['label'] as String? ?? j['value'] as String,
      );
}

class PluginConfigField {
  final String key;
  final String label;
  final String type; // "string"|"enum"|"int"|"bool"|"color"
  final dynamic defaultValue;
  final List<PluginFieldOption> options;
  final int? min;
  final int? max;
  final String? help;

  const PluginConfigField({
    required this.key,
    required this.label,
    required this.type,
    this.defaultValue,
    this.options = const [],
    this.min,
    this.max,
    this.help,
  });

  factory PluginConfigField.fromJson(Map<String, dynamic> j) =>
      PluginConfigField(
        key: j['key'] as String,
        label: j['label'] as String? ?? j['key'] as String,
        type: j['type'] as String? ?? 'string',
        defaultValue: j['default'],
        options: ((j['options'] as List<dynamic>?) ?? [])
            .map((o) => PluginFieldOption.fromJson(o as Map<String, dynamic>))
            .toList(),
        min: (j['min'] as num?)?.toInt(),
        max: (j['max'] as num?)?.toInt(),
        help: j['help'] as String?,
      );
}

class PluginDescriptor {
  final String id;
  final String name;
  final String description;
  final String version;
  final List<PluginConfigField> schemaFields;

  const PluginDescriptor({
    required this.id,
    required this.name,
    required this.description,
    required this.version,
    required this.schemaFields,
  });

  factory PluginDescriptor.fromJson(Map<String, dynamic> j) {
    final schema = j['config_schema'] as Map<String, dynamic>?;
    final fields = ((schema?['fields'] as List<dynamic>?) ?? [])
        .map((f) => PluginConfigField.fromJson(f as Map<String, dynamic>))
        .toList();
    return PluginDescriptor(
      id: j['id'] as String? ?? '',
      name: j['name'] as String? ?? '',
      description: j['description'] as String? ?? '',
      version: j['version'] as String? ?? '',
      schemaFields: fields,
    );
  }
}

class PluginCatalogEntry {
  final String id;
  final String kind; // "builtin" | "exec"
  final PluginDescriptor descriptor;

  const PluginCatalogEntry({
    required this.id,
    required this.kind,
    required this.descriptor,
  });

  factory PluginCatalogEntry.fromJson(Map<String, dynamic> j) =>
      PluginCatalogEntry(
        id: j['id'] as String? ?? '',
        kind: j['kind'] as String? ?? 'builtin',
        descriptor: PluginDescriptor.fromJson(
            (j['descriptor'] as Map<String, dynamic>?) ?? {}),
      );
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
