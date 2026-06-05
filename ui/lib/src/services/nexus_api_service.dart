import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;
import '../models/api_models.dart';

export '../models/api_models.dart' show NexusConfig, DeviceInfo, ApiError;

/// API service for communicating with the Nexus Open backend.
/// Uses [NexusConfig] and [DeviceInfo] from api_models.dart (freezed).
class NexusApiService {
  static const String baseUrl = 'http://localhost:1985';
  static const Duration timeout = Duration(seconds: 10);

  final http.Client _client;

  NexusApiService({http.Client? client}) : _client = client ?? http.Client();

  /// Check backend and hardware health.
  Future<({bool healthy, bool firstRun, bool deviceConnected})> checkHealth() async {
    try {
      final response = await _client
          .get(Uri.parse('$baseUrl/api/health'))
          .timeout(timeout);
      if (response.statusCode == 200) {
        final body = json.decode(response.body) as Map<String, dynamic>;
        return (
          healthy: true,
          firstRun: body['first_run'] as bool? ?? false,
          deviceConnected: body['device_connected'] as bool? ?? false,
        );
      }
      return (healthy: false, firstRun: false, deviceConnected: false);
    } catch (e) {
      return (healthy: false, firstRun: false, deviceConnected: false);
    }
  }

  /// Get current configuration
  Future<NexusConfig> getConfig() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/config'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      return NexusConfig.fromJson(
          json.decode(response.body) as Map<String, dynamic>);
    }
    throw ApiException('Failed to load configuration',
        statusCode: response.statusCode);
  }

  /// Update configuration
  Future<void> updateConfig(NexusConfig config) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/config'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(config.toJson()),
        )
        .timeout(timeout);

    if (response.statusCode != 200) {
      final body = json.decode(response.body) as Map<String, dynamic>;
      throw ApiException(
        body['message'] as String? ?? 'Failed to update configuration',
        statusCode: response.statusCode,
      );
    }
  }

  /// Upload an image
  Future<String> uploadImage(File imageFile) async {
    final request = http.MultipartRequest(
      'POST',
      Uri.parse('$baseUrl/api/images/upload'),
    );
    request.files
        .add(await http.MultipartFile.fromPath('image', imageFile.path));

    final streamedResponse = await request.send().timeout(timeout);
    final response = await http.Response.fromStream(streamedResponse);

    if (response.statusCode == 200) {
      final data = json.decode(response.body) as Map<String, dynamic>;
      return (data['data'] as Map?)?['filename'] as String? ??
          imageFile.path.split('/').last;
    }
    throw ApiException('Failed to upload image', statusCode: response.statusCode);
  }

  /// List all images
  Future<List<String>> listImages() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/images'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      return (json.decode(response.body) as List<dynamic>)
          .map((e) => e.toString())
          .toList();
    }
    throw ApiException('Failed to list images', statusCode: response.statusCode);
  }

  /// Delete an image
  Future<void> deleteImage(String filename) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/images/delete'),
          headers: {'Content-Type': 'application/x-www-form-urlencoded'},
          body: {'filename': filename},
        )
        .timeout(timeout);

    if (response.statusCode != 200) {
      throw ApiException('Failed to delete image',
          statusCode: response.statusCode);
    }
  }

  /// Get device info (model, firmware, connect_error)
  Future<DeviceInfo> getDeviceInfo() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/device/info'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      final body = json.decode(response.body) as Map<String, dynamic>;
      final data = (body['data'] ?? body) as Map<String, dynamic>;
      return DeviceInfo.fromJson(data);
    }
    throw ApiException('Failed to get device info',
        statusCode: response.statusCode);
  }

  /// Set display brightness (0–100)
  Future<void> setBrightness(int value) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/device/brightness'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(BrightnessRequest(brightness: value).toJson()),
        )
        .timeout(timeout);

    if (response.statusCode != 200) {
      throw ApiException('Failed to set brightness',
          statusCode: response.statusCode);
    }
  }

  /// Get zone status (ok / error / timeout / loading)
  Future<Map<String, String>> getZoneStatus(String zoneId) async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/zones/$zoneId/status'))
        .timeout(timeout);
    if (response.statusCode == 200) {
      final body = json.decode(response.body) as Map<String, dynamic>;
      return {
        'status': body['status'] as String? ?? 'loading',
        'error': body['error'] as String? ?? '',
      };
    }
    return {'status': 'loading', 'error': ''};
  }

  /// Get config for a specific module zone
  Future<Map<String, dynamic>> getModuleConfig(String zoneId) async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/modules/$zoneId/config'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      final body = json.decode(response.body) as Map<String, dynamic>;
      if (body.containsKey('data')) {
        return Map<String, dynamic>.from(body['data'] as Map);
      }
      return body;
    }
    throw ApiException('Failed to load module config',
        statusCode: response.statusCode);
  }

  /// Update config for a specific module zone
  Future<void> updateModuleConfig(
      String zoneId, Map<String, dynamic> config) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/modules/$zoneId/config'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(config),
        )
        .timeout(timeout);

    if (response.statusCode != 200) {
      throw ApiException('Failed to update module config',
          statusCode: response.statusCode);
    }
  }

  void dispose() => _client.close();
}

/// Exception thrown by API calls
class ApiException implements Exception {
  final String message;
  final int? statusCode;

  ApiException(this.message, {this.statusCode});

  @override
  String toString() => statusCode != null
      ? 'ApiException ($statusCode): $message'
      : 'ApiException: $message';
}
