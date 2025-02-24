import 'package:flutter/material.dart';

class SettingsState extends ChangeNotifier {
  // Display settings
  String _temperatureUnit = 'Celsius';
  String _distanceUnit = 'Kilometers';
  String _timeFormat = '24h';
  String _dateFormat = 'YYYY-MM-DD';

  // Appearance settings
  Color _textColor = Colors.white;
  Color _backgroundColor = Colors.black;
  bool _showBackground = true;

  // Getters
  String get temperatureUnit => _temperatureUnit;
  String get distanceUnit => _distanceUnit;
  String get timeFormat => _timeFormat;
  String get dateFormat => _dateFormat;
  Color get textColor => _textColor;
  Color get backgroundColor => _backgroundColor;
  bool get showBackground => _showBackground;

  // Setters
  void setTemperatureUnit(String value) {
    _temperatureUnit = value;
    notifyListeners();
  }

  void setDistanceUnit(String value) {
    _distanceUnit = value;
    notifyListeners();
  }

  void setTimeFormat(String value) {
    _timeFormat = value;
    notifyListeners();
  }

  void setDateFormat(String value) {
    _dateFormat = value;
    notifyListeners();
  }

  void setTextColor(Color value) {
    _textColor = value;
    notifyListeners();
  }

  void setBackgroundColor(Color value) {
    _backgroundColor = value;
    notifyListeners();
  }

  void setShowBackground(bool value) {
    _showBackground = value;
    notifyListeners();
  }
}
