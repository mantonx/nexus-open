import 'dart:io';
import 'package:http/http.dart' as http;
import 'package:path/path.dart' as p;

/// An [http.BaseClient] that injects the local capability token into every
/// request as [X-Nexus-Token].  The token is written by the daemon at
/// ~/.config/nexus-open/token (mode 0600) and must match the value the server
/// loaded at startup.
class TokenClient extends http.BaseClient {
  final http.Client _inner;
  final String _token;

  TokenClient._(this._inner, this._token);

  /// Load the token from disk and return a [TokenClient].  Returns a plain
  /// [http.Client] wrapped without a token when the file cannot be read, so
  /// the app degrades gracefully on the first run or a read-only filesystem.
  static Future<http.Client> create({http.Client? inner}) async {
    final client = inner ?? http.Client();
    final token = await _readToken();
    if (token == null) return client;
    return TokenClient._(client, token);
  }

  /// Read the token from disk without constructing a full client.
  /// Returns null when the file is absent or unreadable.
  static Future<String?> readToken() => _readToken();

  static Future<String?> _readToken() async {
    try {
      final configHome = Platform.environment['XDG_CONFIG_HOME'] ??
          p.join(Platform.environment['HOME'] ?? '', '.config');
      final path = p.join(configHome, 'nexus-open', 'token');
      final raw = await File(path).readAsString();
      final tok = raw.trim();
      return tok.length == 64 ? tok : null;
    } catch (_) {
      return null;
    }
  }

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) {
    request.headers['X-Nexus-Token'] = _token;
    return _inner.send(request);
  }

  @override
  void close() => _inner.close();
}
