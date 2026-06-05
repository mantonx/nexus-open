import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../services/nexus_api_service.dart';

const _kThemeKey = 'nexus_theme_mode';

class SettingsState extends ChangeNotifier {
  final NexusApiService _apiService;

  // Loading and error states
  bool _isLoading = false;
  bool _isConnected = false;
  bool _deviceConnected = false;
  String? _errorMessage;

  // Theme preference — persisted locally via shared_preferences
  ThemeMode _themeMode = ThemeMode.dark;
  bool _isFirstRun = false;

  // Configuration
  NexusConfig? _config;

  SettingsState({NexusApiService? apiService})
      : _apiService = apiService ?? NexusApiService() {
    _initialize();
  }

  // Getters
  bool get isLoading => _isLoading;
  bool get isConnected => _isConnected;
  bool get deviceConnected => _deviceConnected;
  String? get errorMessage => _errorMessage;
  NexusConfig? get config => _config;
  ThemeMode get themeMode => _themeMode;
  bool get isFirstRun => _isFirstRun;

  Future<void> setThemeMode(ThemeMode mode) async {
    _themeMode = mode;
    notifyListeners();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kThemeKey, mode.name);
  }

  /// Force onboarding to show — used by the screenshot tour via VM service.
  // ignore: use_setters_to_change_properties
  void forceFirstRun() {
    _isFirstRun = true;
    notifyListeners();
  }

  void setConnected(bool value) {
    _isConnected = value;
    notifyListeners();
  }

  void dismissFirstRun() {
    _isFirstRun = false;
    notifyListeners();
  }

  String get location => _config?.location ?? '';
  String get timeFormat => _config?.timeFormat ?? '24h';
  String get unit => _config?.unit ?? 'imperial';
  String get backgroundColor => _config?.backgroundColor ?? '#000000';
  String get backgroundImage => _config?.backgroundImage ?? 'background.png';
  String get textColor => _config?.textColor ?? '#FFFFFF';
  List<String> get imagePaths => _config?.imagePaths ?? [];

  // Additional unit getters
  String get temperatureUnit => unit == 'metric' ? 'Celsius' : 'Fahrenheit';
  String get distanceUnit => unit == 'metric' ? 'Kilometers' : 'Miles';
  String get dateFormat => _config?.display.dateFormat ?? 'MM/DD/YYYY';

  // Color getters
  Color get backgroundColorValue => _hexToColor(backgroundColor);
  Color get textColorValue => _hexToColor(textColor);

  /// Initialize by loading config from backend
  Future<void> _initialize() async {
    final prefs = await SharedPreferences.getInstance();
    final saved = prefs.getString(_kThemeKey);
    if (saved != null) {
      _themeMode = ThemeMode.values.firstWhere(
        (m) => m.name == saved,
        orElse: () => ThemeMode.dark,
      );
    }
    await loadFromBackend();
  }

  /// Load configuration from backend
  Future<void> loadFromBackend() async {
    _setLoading(true);
    _clearError();

    try {
      // Check health first
      final health = await _apiService.checkHealth();
      _isConnected = health.healthy;
      _isFirstRun = health.firstRun;
      _deviceConnected = health.deviceConnected;

      if (!health.healthy) {
        throw ApiException('Backend is not responding');
      }

      // Load configuration
      _config = await _apiService.getConfig();
      notifyListeners();
    } catch (e) {
      _setError('Failed to load configuration: $e');
      _isConnected = false;
    } finally {
      _setLoading(false);
    }
  }

  /// Save configuration to backend
  Future<void> saveToBackend() async {
    if (_config == null) {
      _setError('No configuration to save');
      return;
    }

    _setLoading(true);
    _clearError();

    try {
      await _apiService.updateConfig(_config!);
      notifyListeners();
    } catch (e) {
      _setError('Failed to save configuration: $e');
    } finally {
      _setLoading(false);
    }
  }

  /// Update configuration locally.
  /// Only the fields you pass are changed; others keep their current value.
  void updateConfig({
    String? location,
    String? timeFormat,
    String? unit,
    String? backgroundColor,
    String? backgroundImage,
    String? textColor,
    List<String>? imagePaths,
    String? dateFormat,
  }) {
    if (_config == null) return;

    // dateFormat lives in the nested display sub-object
    final display = dateFormat != null
        ? _config!.display.copyWith(dateFormat: dateFormat)
        : _config!.display;

    _config = _config!.copyWith(
      location: location ?? _config!.location,
      timeFormat: timeFormat ?? _config!.timeFormat,
      unit: unit ?? _config!.unit,
      backgroundColor: backgroundColor ?? _config!.backgroundColor,
      backgroundImage: backgroundImage ?? _config!.backgroundImage,
      textColor: textColor ?? _config!.textColor,
      imagePaths: imagePaths ?? _config!.imagePaths,
      display: display,
    );
    notifyListeners();
  }

  /// Set location
  void setLocation(String value) {
    updateConfig(location: value);
  }

  /// Set time format
  void setTimeFormat(String value) {
    updateConfig(timeFormat: value);
  }

  /// Set unit (metric/imperial)
  void setUnit(String value) {
    updateConfig(unit: value);
  }

  /// Set temperature unit
  void setTemperatureUnit(String value) {
    // Convert temperature unit to metric/imperial
    final unit = value == 'Celsius' ? 'metric' : 'imperial';
    updateConfig(unit: unit);
  }

  /// Set distance unit
  void setDistanceUnit(String value) {
    // Convert distance unit to metric/imperial
    final unit = value == 'Kilometers' ? 'metric' : 'imperial';
    updateConfig(unit: unit);
  }

  /// Set date format
  void setDateFormat(String value) {
    updateConfig(dateFormat: value);
  }

  /// Set background color (accepts both Color and String)
  void setBackgroundColor(dynamic color) {
    if (color is Color) {
      final hexColor = '#${color.value.toRadixString(16).substring(2).toUpperCase()}';
      updateConfig(backgroundColor: hexColor);
    } else if (color is String) {
      updateConfig(backgroundColor: color);
    }
  }

  /// Set text color (accepts both Color and String)
  void setTextColor(dynamic color) {
    if (color is Color) {
      final hexColor = '#${color.value.toRadixString(16).substring(2).toUpperCase()}';
      updateConfig(textColor: hexColor);
    } else if (color is String) {
      updateConfig(textColor: color);
    }
  }

  /// Set background image
  void setBackgroundImage(String imageName) {
    updateConfig(backgroundImage: imageName);
  }

  /// Add image path
  void addImagePath(String path) {
    final newPaths = List<String>.from(imagePaths)..add(path);
    updateConfig(imagePaths: newPaths);
  }

  /// Remove image path
  void removeImagePath(String path) {
    final newPaths = List<String>.from(imagePaths)..remove(path);
    updateConfig(imagePaths: newPaths);
  }

  /// Retry connection
  Future<void> retryConnection() async {
    await loadFromBackend();
  }

  // Helper methods
  void _setLoading(bool value) {
    _isLoading = value;
    notifyListeners();
  }

  void _setError(String message) {
    _errorMessage = message;
    notifyListeners();
  }

  void _clearError() {
    _errorMessage = null;
    notifyListeners();
  }

  Color _hexToColor(String hexColor) {
    final hex = hexColor.replaceAll('#', '');
    return Color(int.parse('FF$hex', radix: 16));
  }

  @override
  void dispose() {
    _apiService.dispose();
    super.dispose();
  }
}
