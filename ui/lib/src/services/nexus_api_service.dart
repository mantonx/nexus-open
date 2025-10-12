import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;

/// API service for communicating with the Nexus Open backend
class NexusApiService {
  static const String baseUrl = 'http://localhost:1985';
  static const Duration timeout = Duration(seconds: 10);

  final http.Client _client;

  NexusApiService({http.Client? client}) : _client = client ?? http.Client();

  /// Check if the backend is healthy
  Future<bool> checkHealth() async {
    try {
      final response = await _client
          .get(Uri.parse('$baseUrl/api/health'))
          .timeout(timeout);
      return response.statusCode == 200;
    } catch (e) {
      return false;
    }
  }

  /// Get current configuration
  Future<NexusConfig> getConfig() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/config'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      return NexusConfig.fromJson(json.decode(response.body));
    } else {
      throw ApiException(
        'Failed to load configuration',
        statusCode: response.statusCode,
      );
    }
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
      final error = json.decode(response.body);
      throw ApiException(
        error['message'] ?? 'Failed to update configuration',
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

    request.files.add(
      await http.MultipartFile.fromPath('image', imageFile.path),
    );

    final streamedResponse = await request.send().timeout(timeout);
    final response = await http.Response.fromStream(streamedResponse);

    if (response.statusCode == 200) {
      final data = json.decode(response.body);
      return data['data']['filename'] ?? imageFile.path.split('/').last;
    } else {
      throw ApiException(
        'Failed to upload image',
        statusCode: response.statusCode,
      );
    }
  }

  /// List all images
  Future<List<String>> listImages() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/images'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      final List<dynamic> images = json.decode(response.body);
      return images.map((e) => e.toString()).toList();
    } else {
      throw ApiException(
        'Failed to list images',
        statusCode: response.statusCode,
      );
    }
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
      throw ApiException(
        'Failed to delete image',
        statusCode: response.statusCode,
      );
    }
  }

  /// Close the HTTP client
  void dispose() {
    _client.close();
  }
}

/// Configuration model matching the Go backend
class NexusConfig {
  final String location;
  final String timeFormat; // "12h" or "24h"
  final String unit; // "metric" or "imperial"
  final String backgroundColor; // hex color
  final String backgroundImage;
  final String textColor; // hex color
  final List<String> imagePaths;

  NexusConfig({
    required this.location,
    required this.timeFormat,
    required this.unit,
    required this.backgroundColor,
    required this.backgroundImage,
    required this.textColor,
    required this.imagePaths,
  });

  factory NexusConfig.fromJson(Map<String, dynamic> json) {
    return NexusConfig(
      location: json['location'] ?? '',
      timeFormat: json['time_format'] ?? '24h',
      unit: json['unit'] ?? 'imperial',
      backgroundColor: json['background_color'] ?? '#000000',
      backgroundImage: json['background_image'] ?? 'background.png',
      textColor: json['text_color'] ?? '#FFFFFF',
      imagePaths: (json['image_paths'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          [],
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'location': location,
      'time_format': timeFormat,
      'unit': unit,
      'background_color': backgroundColor,
      'background_image': backgroundImage,
      'text_color': textColor,
      'image_paths': imagePaths,
    };
  }

  NexusConfig copyWith({
    String? location,
    String? timeFormat,
    String? unit,
    String? backgroundColor,
    String? backgroundImage,
    String? textColor,
    List<String>? imagePaths,
  }) {
    return NexusConfig(
      location: location ?? this.location,
      timeFormat: timeFormat ?? this.timeFormat,
      unit: unit ?? this.unit,
      backgroundColor: backgroundColor ?? this.backgroundColor,
      backgroundImage: backgroundImage ?? this.backgroundImage,
      textColor: textColor ?? this.textColor,
      imagePaths: imagePaths ?? this.imagePaths,
    );
  }
}

/// Exception thrown by API calls
class ApiException implements Exception {
  final String message;
  final int? statusCode;

  ApiException(this.message, {this.statusCode});

  @override
  String toString() {
    if (statusCode != null) {
      return 'ApiException ($statusCode): $message';
    }
    return 'ApiException: $message';
  }
}
