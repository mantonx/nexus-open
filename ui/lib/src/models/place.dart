class Place {
  final String displayName;
  final double latitude;
  final double longitude;
  final String type;

  Place({
    required this.displayName,
    required this.latitude,
    required this.longitude,
    required this.type,
  });

  factory Place.fromJson(Map<String, dynamic> json) {
    return Place(
      displayName: json['display_name'] as String,
      latitude: double.parse(json['lat']),
      longitude: double.parse(json['lon']),
      type: json['type'] as String,
    );
  }
}
