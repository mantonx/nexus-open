import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_typeahead/flutter_typeahead.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import '../../maps/world_map.dart';
import '../../../services/location_service.dart';
import '../../../models/place.dart';

class LocationTab extends StatefulWidget {
  final Function(String) onLocationSelected;

  const LocationTab({super.key, required this.onLocationSelected});

  @override
  State<LocationTab> createState() => _LocationTabState();
}

class _LocationTabState extends State<LocationTab> {
  final _formKey = GlobalKey<FormState>();
  final _cityController = TextEditingController(text: 'Jersey City, NJ');
  final _mapController = MapController();
  static const double _defaultZoom = 4.0;
  static const double _selectedZoom = 12.0;
  Place? _selectedPlace;
  bool _isSearching = false;

  @override
  void initState() {
    super.initState();
    _searchLocation(_cityController.text);
  }

  @override
  void dispose() {
    _cityController.dispose();
    super.dispose();
  }

  Future<void> _searchLocation(String query) async {
    if (query.isEmpty) {
      setState(() => _isSearching = false);
      return;
    }

    setState(() => _isSearching = true);
    try {
      final places = await LocationService.searchPlaces(query);
      if (mounted) {
        setState(() {
          if (places.isNotEmpty) {
            _selectedPlace = places.first;
          }
          _isSearching = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _selectedPlace = null;
          _isSearching = false;
        });
      }
    }
  }

  void _updateMapLocation(Place place) {
    setState(() {
      _selectedPlace = place;
      _cityController.text = LocationService.getDisplayString(
          place); // Use formatted display string
      _isSearching = false;
    });

    print(_cityController.text);

    // Notify parent widget of the selected location
    widget.onLocationSelected(_cityController.text);

    // Animate to new location with zoom
    _mapController.move(
      LatLng(place.latitude, place.longitude),
      _selectedZoom,
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Form(
              key: _formKey,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Location Settings',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  const SizedBox(height: 16),
                  TypeAheadFormField<Place>(
                    textFieldConfiguration: TextFieldConfiguration(
                      controller: _cityController,
                      decoration: InputDecoration(
                        labelText: 'Location',
                        helperText: 'Enter city name or address',
                        border: const OutlineInputBorder(),
                        suffixIcon: _isSearching &&
                                _cityController.text.isNotEmpty
                            ? const Padding(
                                padding: EdgeInsets.all(8.0),
                                child:
                                    CircularProgressIndicator(strokeWidth: 2),
                              )
                            : null,
                      ),
                    ),
                    suggestionsCallback: (pattern) async {
                      if (pattern.isEmpty) {
                        setState(() => _isSearching = false);
                        return [];
                      }
                      return LocationService.searchPlaces(pattern);
                    },
                    itemBuilder: (context, Place place) {
                      return ListTile(
                        title: Text(LocationService.getDisplayString(
                            place)), // Use formatted display string
                        subtitle: Text('${place.type}'),
                      );
                    },
                    onSuggestionSelected: _updateMapLocation,
                    noItemsFoundBuilder: (context) => const Padding(
                      padding: EdgeInsets.all(16.0),
                      child: Text('No locations found'),
                    ),
                  ),
                  if (_selectedPlace != null) ...[
                    const SizedBox(height: 8),
                    Text(
                      'Found: ${LocationService.getDisplayString(_selectedPlace!)}', // Use formatted display string
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                  const SizedBox(height: 24),
                  SizedBox(
                    height: 400,
                    child: Column(
                      children: [
                        Expanded(
                          child: WorldMap(
                            latitude: _selectedPlace?.latitude ?? 51.5074,
                            longitude: _selectedPlace?.longitude ?? -0.1278,
                            width: MediaQuery.of(context).size.width - 64,
                            mapColor: Theme.of(context).colorScheme.onSurface,
                            indicatorColor:
                                Theme.of(context).colorScheme.primary,
                            zoom: _selectedPlace != null
                                ? _selectedZoom
                                : _defaultZoom,
                            controller: _mapController,
                          ),
                        ),
                        const Padding(
                          padding: EdgeInsets.all(8.0),
                          child: Text(
                            '© OpenStreetMap contributors',
                            style: TextStyle(fontSize: 12),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}
