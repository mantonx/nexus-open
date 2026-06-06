import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_typeahead/flutter_typeahead.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import '../../maps/world_map.dart';
import '../../common/common.dart';
import '../../../services/location_service.dart';
import '../../../models/place.dart';
import '../../../theme/app_tokens.dart';

class LocationTab extends StatefulWidget {
  final Function(String) onLocationSelected;
  final String initialLocation;

  const LocationTab({
    super.key,
    required this.onLocationSelected,
    this.initialLocation = '',
  });

  @override
  State<LocationTab> createState() => _LocationTabState();
}

class _LocationTabState extends State<LocationTab> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _cityController;
  final _mapController = MapController();
  final _searchFocusNode = FocusNode();
  static const double _defaultZoom = 4.0;
  static const double _selectedZoom = 12.0;
  Place? _selectedPlace;
  bool _isSearching = false;

  @override
  void initState() {
    super.initState();
    _cityController = TextEditingController(text: widget.initialLocation);
    if (widget.initialLocation.isNotEmpty) {
      _searchLocation(widget.initialLocation);
    }
    // Auto-focus the search field when this tab opens (11.2)
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _searchFocusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _cityController.dispose();
    _searchFocusNode.dispose();
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
    final theme = Theme.of(context);
    final cs = Theme.of(context).colorScheme;

    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        NexusSection(
          title: 'Location',
          description: 'Used for the weather plugin.',
          child: Form(
            key: _formKey,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                TypeAheadFormField<Place>(
                  textFieldConfiguration: TextFieldConfiguration(
                    controller: _cityController,
                    focusNode: _searchFocusNode,
                    style: theme.textTheme.bodyLarge,
                    decoration: InputDecoration(
                      labelText: 'City or address',
                      hintText: 'Search for a city…',
                      // Mirror the app's InputDecorationTheme exactly
                      filled: true,
                      fillColor: cs.surfaceContainerHigh,
                      contentPadding: AppSpacing.paddingHMdVSm,
                      border: OutlineInputBorder(
                        borderRadius: AppRadius.smBr,
                        borderSide: BorderSide(color: cs.outline),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: AppRadius.smBr,
                        borderSide: BorderSide(color: cs.outline),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: AppRadius.smBr,
                        borderSide: const BorderSide(
                            color: AppColors.accent, width: 2),
                      ),
                      suffixIcon: _isSearching && _cityController.text.isNotEmpty
                          ? Padding(
                              padding: AppSpacing.paddingXs,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: AppColors.accent),
                            )
                          : null,
                    ),
                  ),
                  suggestionsCallback: (pattern) async {
                    if (pattern.isEmpty) {
                      WidgetsBinding.instance.addPostFrameCallback(
                        (_) { if (mounted) setState(() => _isSearching = false); },
                      );
                      return [];
                    }
                    return LocationService.searchPlaces(pattern);
                  },
                  itemBuilder: (context, Place place) => ListTile(
                    title: Text(LocationService.getDisplayString(place)),
                    subtitle: Text(place.type,
                        style: theme.textTheme.bodySmall),
                  ),
                  onSuggestionSelected: _updateMapLocation,
                  noItemsFoundBuilder: (_) => Padding(
                    padding: AppSpacing.paddingMd,
                    child: Text('No locations found',
                        style: theme.textTheme.bodySmall),
                  ),
                ),
                if (_selectedPlace != null) ...[
                  const SizedBox(height: AppSpacing.xs),
                  Row(
                    children: [
                      Icon(Icons.check_circle_outline,
                          size: AppIconSize.sm, color: cs.success),
                      const SizedBox(width: AppSpacing.xs),
                      Expanded(
                        child: Text(
                          LocationService.getDisplayString(_selectedPlace!),
                          style: theme.textTheme.bodySmall
                              ?.copyWith(color: cs.success),
                        ),
                      ),
                    ],
                  ),
                ],
                const SizedBox(height: AppSpacing.md),
                // Map — use available height rather than fixed 400px
                SizedBox(
                  height: 340,
                  child: Column(
                    children: [
                      Expanded(
                        child: ClipRRect(
                          borderRadius: AppRadius.smBr,
                          child: WorldMap(
                            latitude: _selectedPlace?.latitude ?? 51.5074,
                            longitude: _selectedPlace?.longitude ?? -0.1278,
                            width: MediaQuery.of(context).size.width - 64,
                            mapColor: cs.onSurface,
                            indicatorColor: AppColors.accent,
                            zoom: _selectedPlace != null
                                ? _selectedZoom
                                : _defaultZoom,
                            controller: _mapController,
                          ),
                        ),
                      ),
                      Padding(
                        padding: AppSpacing.paddingVSm,
                        child: Text(
                          '© OpenStreetMap contributors',
                          style: theme.textTheme.labelSmall
                              ?.copyWith(color: cs.onSurfaceVariant),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
