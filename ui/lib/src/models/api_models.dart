// Models generated from api/openapi.yaml via freezed + json_serializable.
// After editing this file run: dart run build_runner build --delete-conflicting-outputs

// ignore_for_file: invalid_annotation_target

import 'package:freezed_annotation/freezed_annotation.dart';

part 'api_models.freezed.dart';
part 'api_models.g.dart';

// ── DisplayConfig ────────────────────────────────────────────────────────────

@freezed
sealed class DisplayConfig with _$DisplayConfig {
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
sealed class NexusConfig with _$NexusConfig {
  const factory NexusConfig({
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
sealed class DeviceInfo with _$DeviceInfo {
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
sealed class BrightnessRequest with _$BrightnessRequest {
  const factory BrightnessRequest({
    @Default(75) int brightness,
  }) = _BrightnessRequest;

  factory BrightnessRequest.fromJson(Map<String, dynamic> json) =>
      _$BrightnessRequestFromJson(json);
}

// ── ApiError ─────────────────────────────────────────────────────────────────

@freezed
sealed class ApiError with _$ApiError {
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

class PluginConfigShowIf {
  final String key;
  final String notEq;
  const PluginConfigShowIf({required this.key, required this.notEq});
  factory PluginConfigShowIf.fromJson(Map<String, dynamic> j) =>
      PluginConfigShowIf(key: j['key'] as String, notEq: j['not_eq'] as String);

  bool isVisible(Map<String, dynamic> currentConfig) {
    final v = currentConfig[key];
    return v?.toString() != notEq;
  }
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
  final PluginConfigShowIf? showIf;

  const PluginConfigField({
    required this.key,
    required this.label,
    required this.type,
    this.defaultValue,
    this.options = const [],
    this.min,
    this.max,
    this.help,
    this.showIf,
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
        showIf: j['show_if'] == null
            ? null
            : PluginConfigShowIf.fromJson(j['show_if'] as Map<String, dynamic>),
      );
}

class PluginDescriptor {
  final String id;
  final String name;
  final String description;
  final String version;
  final bool hasGraph;
  final List<PluginConfigField> schemaFields;

  const PluginDescriptor({
    required this.id,
    required this.name,
    required this.description,
    required this.version,
    required this.hasGraph,
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
      hasGraph: j['has_graph'] as bool? ?? false,
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

// ── DaemonInfo ───────────────────────────────────────────────────────────────

class DaemonInfo {
  final String version;
  final String commit;
  final String buildTime;
  final String goVersion;
  final int pluginCount;

  const DaemonInfo({
    required this.version,
    required this.commit,
    required this.buildTime,
    required this.goVersion,
    required this.pluginCount,
  });

  factory DaemonInfo.fromJson(Map<String, dynamic> j) => DaemonInfo(
        version: j['version'] as String? ?? '',
        commit: j['commit'] as String? ?? '',
        buildTime: j['build_time'] as String? ?? '',
        goVersion: j['go_version'] as String? ?? '',
        pluginCount: (j['plugin_count'] as num?)?.toInt() ?? 0,
      );
}

// ── ApiSuccess ───────────────────────────────────────────────────────────────

@freezed
sealed class ApiSuccess with _$ApiSuccess {
  const factory ApiSuccess({
    @Default('success') String status,
    String? message,
    @JsonKey(includeIfNull: false) dynamic data,
  }) = _ApiSuccess;

  factory ApiSuccess.fromJson(Map<String, dynamic> json) =>
      _$ApiSuccessFromJson(json);
}
