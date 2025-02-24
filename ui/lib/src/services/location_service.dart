import 'dart:convert';
import 'package:http/http.dart' as http;
import '../models/place.dart';

class LocationService {
  static const String _baseUrl = 'https://nominatim.openstreetmap.org/search';

  static Future<List<Place>> searchPlaces(String query) async {
    if (query.length < 2) return [];

    try {
      final response = await http.get(
        Uri.parse(
            '$_baseUrl?q=${Uri.encodeComponent(query)}&format=json&limit=5'),
        headers: {
          'Accept': 'application/json',
          'User-Agent': 'OpenNext/1.0', // Required by Nominatim ToS
        },
      );

      if (response.statusCode == 200) {
        final List<dynamic> results = json.decode(response.body);
        return results.map((json) => Place.fromJson(json)).toList();
      }
      return [];
    } catch (e) {
      print('Error searching places: $e');
      return [];
    }
  }

  static String formatDisplayName(String fullName) {
    final parts = fullName.split(',').map((e) => e.trim()).toList();

    // If it's a very short string, return as is
    if (parts.length <= 2) return fullName;

    String mainPart = parts.first;
    String country = parts.last;
    List<String> locationParts = [];

    // Find town/city and state/region, skipping county
    for (int i = 1; i < parts.length - 1; i++) {
      final part = parts[i];
      // Skip numeric parts (like postal codes), county names, and very long parts
      if (!part.contains(RegExp(r'\d')) &&
          !part.toLowerCase().contains('county') &&
          part.length < 25) {
        locationParts.add(part);
      }
    }

    // For US addresses, show only city and state
    if (country.trim() == 'United States') {
      if (locationParts.length >= 2) {
        return '${locationParts[0]}, ${locationParts[1]}';
      }
    }

    // For addresses, show: "Street Address, Town, State, Country"
    if (mainPart.contains(RegExp(r'\d'))) {
      List<String> result = [mainPart];

      // Add up to 2 location parts (usually town and state)
      if (locationParts.isNotEmpty) {
        result.addAll(locationParts.take(2));
      }

      // Always add country
      result.add(country);

      return result.join(', ');
    }

    // For places/POIs: "Place Name, Town, Country"
    else {
      return [
        mainPart,
        if (locationParts.isNotEmpty) locationParts.first,
        country,
      ].join(', ');
    }
  }

  static String getDisplayString(Place place) {
    return formatDisplayName(place.displayName);
  }
}
