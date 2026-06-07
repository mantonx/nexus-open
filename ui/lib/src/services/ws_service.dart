import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

/// Typed message from the backend WebSocket stream.
sealed class WsEvent {}

class WsFrameEvent extends WsEvent {
  final Uint8List pngBytes;
  WsFrameEvent(this.pngBytes);
}

class WsWindowStateEvent extends WsEvent {
  final String state; // "shown" | "hidden"
  WsWindowStateEvent(this.state);
}

class WsZoneInfo {
  final String id;
  final int width;
  const WsZoneInfo({required this.id, required this.width});
  factory WsZoneInfo.fromJson(Map<String, dynamic> j) =>
      WsZoneInfo(id: j['id'] as String, width: j['width'] as int);
}

class WsPageInfo {
  final String name;
  final List<WsZoneInfo> zones;
  const WsPageInfo({required this.name, required this.zones});
  factory WsPageInfo.fromJson(Map<String, dynamic> j) => WsPageInfo(
        name: j['name'] as String,
        zones: (j['zones'] as List<dynamic>)
            .map((z) => WsZoneInfo.fromJson(z as Map<String, dynamic>))
            .toList(),
      );
}

class WsPageStateEvent extends WsEvent {
  final int currentPage;
  final int numPages;
  final List<WsPageInfo> pages;
  WsPageStateEvent({
    required this.currentPage,
    required this.numPages,
    required this.pages,
  });
}

class WsConnectedEvent extends WsEvent {}

class WsDisconnectedEvent extends WsEvent {}

class WsDraftStateEvent extends WsEvent {
  final bool active;
  final String? reason; // "idle_timeout" if auto-discarded
  WsDraftStateEvent({required this.active, this.reason});
}

/// Persistent WebSocket connection to ws://localhost:1985/api/ws.
///
/// Reconnects with exponential backoff on disconnect. Exposes a single
/// broadcast stream of [WsEvent]s that any widget can listen to.
class WsService extends ChangeNotifier {
  static const _wsUrl = 'ws://localhost:1985/api/ws';
  static const _maxBackoff = Duration(seconds: 30);

  final _controller = StreamController<WsEvent>.broadcast();
  Stream<WsEvent> get events => _controller.stream;

  WebSocketChannel? _channel;
  bool _disposed = false;
  bool _connected = false;
  Duration _backoff = const Duration(seconds: 1);

  bool get isConnected => _connected;

  WsService() {
    _connect();
  }

  /// Fake constructor for tests — never attempts a real connection.
  WsService.fake() : _disposed = false;

  /// Inject a synthetic event (test use only).
  void injectEvent(WsEvent event) {
    if (!_controller.isClosed) _controller.add(event);
  }

  /// Simulate a successful connection (test use only).
  void markConnected() {
    _connected = true;
    notifyListeners();
  }

  void _connect() async {
    if (_disposed) return;

    try {
      _channel = WebSocketChannel.connect(Uri.parse(_wsUrl));
      await _channel!.ready;

      _connected = true;
      _backoff = const Duration(seconds: 1);
      _controller.add(WsConnectedEvent());
      notifyListeners();

      _channel!.stream.listen(
        _onMessage,
        onError: (_) => _scheduleReconnect(),
        onDone: _scheduleReconnect,
        cancelOnError: true,
      );
    } catch (_) {
      _scheduleReconnect();
    }
  }

  void _onMessage(dynamic raw) {
    try {
      final map = json.decode(raw as String) as Map<String, dynamic>;
      final type = map['type'] as String?;
      final data = map['data'];

      switch (type) {
        case 'frame':
          final bytes = base64Decode(data as String);
          _controller.add(WsFrameEvent(bytes));
        case 'window_state':
          _controller.add(WsWindowStateEvent(data as String));
        case 'page_state':
          final d = data as Map<String, dynamic>;
          _controller.add(WsPageStateEvent(
            currentPage: d['current_page'] as int,
            numPages: d['num_pages'] as int,
            pages: (d['pages'] as List<dynamic>)
                .map((p) => WsPageInfo.fromJson(p as Map<String, dynamic>))
                .toList(),
          ));
        case 'draft_state':
          final d = data as Map<String, dynamic>? ?? {};
          _controller.add(WsDraftStateEvent(
            active: d['active'] as bool? ?? false,
            reason: d['reason'] as String?,
          ));
      }
    } catch (_) {
      // Malformed message — ignore.
    }
  }

  void _scheduleReconnect() {
    if (_disposed) return;
    if (_connected) {
      _connected = false;
      _controller.add(WsDisconnectedEvent());
      notifyListeners();
    }
    Future.delayed(_backoff, () {
      if (!_disposed) {
        _backoff = _backoff * 2 > _maxBackoff ? _maxBackoff : _backoff * 2;
        _connect();
      }
    });
  }

  @override
  void dispose() {
    _disposed = true;
    _channel?.sink.close();
    _controller.close();
    super.dispose();
  }
}
