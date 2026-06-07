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

  /// Get config for a specific plugin zone
  Future<Map<String, dynamic>> getPluginConfig(String zoneId) async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/plugins/$zoneId/config'))
        .timeout(timeout);

    if (response.statusCode == 200) {
      final body = json.decode(response.body) as Map<String, dynamic>;
      if (body.containsKey('data')) {
        return Map<String, dynamic>.from(body['data'] as Map);
      }
      return body;
    }
    throw ApiException('Failed to load plugin config',
        statusCode: response.statusCode);
  }

  /// Update config for a specific plugin zone
  Future<void> updatePluginConfig(
      String zoneId, Map<String, dynamic> config) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/plugins/$zoneId/config'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(config),
        )
        .timeout(timeout);

    if (response.statusCode != 200) {
      throw ApiException('Failed to update plugin config',
          statusCode: response.statusCode);
    }
  }

  /// Trigger a synthetic swipe (mirrors POST /api/debug/swipe).
  Future<void> simulateSwipe({
    String direction = 'left',
    int durationMs = 200,
    int finalizeMs = 120,
    int steps = 20,
    double velocity = 150,
    double releaseAt = 0.7,
  }) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/debug/swipe'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode({
            'direction': direction,
            'duration_ms': durationMs,
            'finalize_ms': finalizeMs,
            'steps': steps,
            'velocity': velocity,
            'release_at': releaseAt,
          }),
        )
        .timeout(timeout);
    if (response.statusCode != 200) {
      throw ApiException('Failed to simulate swipe',
          statusCode: response.statusCode);
    }
  }

  /// Feed a single live-swipe progress update (called on every drag frame).
  Future<void> swipeUpdate(double progress, {required bool isLeft}) async {
    await _client
        .post(
          Uri.parse('$baseUrl/api/debug/swipe/update'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode({'progress': progress, 'is_left': isLeft}),
        )
        .timeout(timeout);
    // Ignore errors — drag updates are best-effort.
  }

  /// Finalise a live swipe (finger lifted).
  Future<void> swipeFinalize(double progress, double velocity,
      {required bool isLeft}) async {
    await _client
        .post(
          Uri.parse('$baseUrl/api/debug/swipe/finalize'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(
              {'progress': progress, 'velocity': velocity, 'is_left': isLeft}),
        )
        .timeout(timeout);
  }

  /// Cancel an in-progress live swipe.
  Future<void> swipeCancel() async {
    await _client
        .post(Uri.parse('$baseUrl/api/debug/swipe/cancel'))
        .timeout(timeout);
  }

  // ── Layout editor ───────────────────────────────────────────────────────────

  /// Fetch the full layout (all pages + zones).
  Future<List<LayoutPage>> getLayout() async {
    final response = await _client
        .get(Uri.parse('$baseUrl/api/layout'))
        .timeout(timeout);
    if (response.statusCode != 200) {
      throw ApiException('Failed to load layout', statusCode: response.statusCode);
    }
    final list = json.decode(response.body) as List<dynamic>;
    return list.map((e) => LayoutPage.fromJson(e as Map<String, dynamic>)).toList();
  }

  Future<int> createPage(String name, int ord) async {
    final r = await _client.post(Uri.parse('$baseUrl/api/layout/pages'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'name': name, 'ord': ord})).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to create page', statusCode: r.statusCode);
    final body = json.decode(r.body) as Map<String, dynamic>;
    return (body['data']?['id'] as num?)?.toInt() ?? 0;
  }

  Future<void> updatePage(int id, String name, int ord) async {
    final r = await _client.put(Uri.parse('$baseUrl/api/layout/pages/$id'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'name': name, 'ord': ord})).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to update page', statusCode: r.statusCode);
  }

  Future<void> deletePage(int id) async {
    final r = await _client.delete(Uri.parse('$baseUrl/api/layout/pages/$id')).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to delete page', statusCode: r.statusCode);
  }

  Future<void> reorderPages(List<int> order) async {
    final r = await _client.post(Uri.parse('$baseUrl/api/layout/pages/reorder'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'order': order})).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to reorder pages', statusCode: r.statusCode);
  }

  Future<void> createZone(LayoutZone zone) async {
    final r = await _client.post(Uri.parse('$baseUrl/api/layout/zones'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode(zone.toJson())).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to create zone', statusCode: r.statusCode);
  }

  Future<void> updateZone(LayoutZone zone) async {
    final r = await _client.put(Uri.parse('$baseUrl/api/layout/zones/${zone.id}'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode(zone.toJson())).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to update zone', statusCode: r.statusCode);
  }

  Future<void> deleteZone(String zoneId) async {
    final r = await _client.delete(Uri.parse('$baseUrl/api/layout/zones/$zoneId')).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to delete zone', statusCode: r.statusCode);
  }

  Future<void> reorderZones(int pageId, List<String> order) async {
    final r = await _client.post(Uri.parse('$baseUrl/api/layout/zones/reorder'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'page_id': pageId, 'order': order})).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to reorder zones', statusCode: r.statusCode);
  }

  Future<void> reorderDraftZones(int pageIndex, List<String> order) async {
    final r = await _client.post(Uri.parse('$baseUrl/api/layout/draft/zones/reorder'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'page_index': pageIndex, 'order': order})).timeout(timeout);
    if (r.statusCode != 200) throw ApiException('Failed to reorder draft zones', statusCode: r.statusCode);
  }

  /// Navigate directly to a page index.
  Future<void> navigatePage(int pageIndex) async {
    final response = await _client
        .post(
          Uri.parse('$baseUrl/api/navigate/page'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode({'page': pageIndex}),
        )
        .timeout(timeout);
    if (response.statusCode != 200) {
      throw ApiException('Failed to navigate to page $pageIndex',
          statusCode: response.statusCode);
    }
  }

  // ── Plugin catalog ─────────────────────────────────────────────────────────

  /// Fetch the plugin catalog (all available plugins with their schemas).
  Future<List<PluginCatalogEntry>> getPluginCatalog() async {
    final r = await _client.get(Uri.parse('$baseUrl/api/plugins')).timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to load plugin catalog', statusCode: r.statusCode);
    }
    final list = json.decode(r.body) as List<dynamic>;
    return list
        .map((e) => PluginCatalogEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  // ── Draft layout ────────────────────────────────────────────────────────────

  /// Open or fetch the current draft layout.
  Future<List<LayoutPage>> getDraft() async {
    final r = await _client.get(Uri.parse('$baseUrl/api/layout/draft')).timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to get draft', statusCode: r.statusCode);
    }
    final body = json.decode(r.body) as Map<String, dynamic>;
    final layout = body['layout'] as Map<String, dynamic>?;
    if (layout == null) return [];
    final pages = (layout['pages'] as List<dynamic>?) ?? [];
    return pages.map((e) => LayoutPage.fromJson(e as Map<String, dynamic>)).toList();
  }

  /// Add a zone to the draft on a given page.
  Future<String> addDraftZone({
    required int pageIndex,
    required String plugin,
    int refreshMs = 1000,
    String? insertBeforeId,
  }) async {
    final body = <String, dynamic>{
      'page_index': pageIndex,
      'plugin': plugin,
      'refresh_ms': refreshMs,
    };
    if (insertBeforeId != null) body['insert_before_id'] = insertBeforeId;
    final r = await _client
        .post(
          Uri.parse('$baseUrl/api/layout/draft/zones'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(body),
        )
        .timeout(timeout);
    if (r.statusCode != 200) {
      final errBody = json.decode(r.body) as Map<String, dynamic>;
      throw ApiException(
        errBody['message'] as String? ?? 'Failed to add zone',
        statusCode: r.statusCode,
      );
    }
    final respBody = json.decode(r.body) as Map<String, dynamic>;
    return (respBody['data'] as Map?)?['id'] as String? ?? '';
  }

  /// Remove a zone from the draft.
  Future<void> deleteDraftZone(String zoneId) async {
    final r = await _client
        .delete(Uri.parse('$baseUrl/api/layout/draft/zones/$zoneId'))
        .timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to delete draft zone', statusCode: r.statusCode);
    }
  }

  /// Update fields on a zone in the draft.
  Future<void> patchDraftZone(String zoneId, Map<String, dynamic> patch) async {
    final r = await _client
        .patch(
          Uri.parse('$baseUrl/api/layout/draft/zones/$zoneId'),
          headers: {'Content-Type': 'application/json'},
          body: json.encode(patch),
        )
        .timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to patch draft zone', statusCode: r.statusCode);
    }
  }

  /// Commit the draft to the store (makes changes durable).
  Future<void> commitDraft() async {
    final r = await _client
        .post(Uri.parse('$baseUrl/api/layout/commit'))
        .timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to commit draft', statusCode: r.statusCode);
    }
  }

  /// Discard the draft (reverts device to committed state).
  Future<void> discardDraft() async {
    final r = await _client
        .post(Uri.parse('$baseUrl/api/layout/discard'))
        .timeout(timeout);
    if (r.statusCode != 200) {
      throw ApiException('Failed to discard draft', statusCode: r.statusCode);
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
