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
