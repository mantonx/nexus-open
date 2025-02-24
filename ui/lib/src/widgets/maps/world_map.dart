import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';

class WorldMap extends StatelessWidget {
  final double latitude;
  final double longitude;
  final double width;
  final Color indicatorColor;
  final Color mapColor;
  final double zoom; // Add zoom parameter
  final MapController? controller; // Add controller for external control

  const WorldMap({
    super.key,
    required this.latitude,
    required this.longitude,
    this.width = 400,
    this.indicatorColor = Colors.red,
    this.mapColor = Colors.white24,
    this.zoom = 4,
    this.controller,
  });

  @override
  Widget build(BuildContext context) {
    final currentLocation = LatLng(latitude, longitude);

    return SizedBox(
      width: width,
      height: width * 0.9, // Increased from 0.75 to 0.9 for better readability
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: Stack(
          children: [
            FlutterMap(
              mapController: controller,
              key: ValueKey('$latitude-$longitude-$zoom'),
              options: MapOptions(
                center: currentLocation,
                zoom: zoom,
                maxZoom: 18,
                minZoom: 2,
                interactiveFlags: InteractiveFlag.all,
              ),
              children: [
                TileLayer(
                  urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
                  userAgentPackageName: 'open_next',
                  maxZoom: 19,
                ),
                MarkerLayer(
                  markers: [
                    Marker(
                      point: currentLocation,
                      width: 40,
                      height: 40,
                      builder: (context) => Icon(
                        Icons.location_on,
                        color: indicatorColor,
                        size: 30,
                      ),
                    ),
                  ],
                ),
              ],
            ),
            // Add zoom controls
            Positioned(
              right: 8,
              top: 8,
              child: Column(
                children: [
                  FloatingActionButton.small(
                    heroTag: 'zoomIn',
                    onPressed: () => controller?.moveAndRotate(
                      currentLocation,
                      (controller?.zoom ?? zoom) + 1,
                      0,
                    ),
                    child: const Icon(Icons.add),
                  ),
                  const SizedBox(height: 8),
                  FloatingActionButton.small(
                    heroTag: 'zoomOut',
                    onPressed: () => controller?.moveAndRotate(
                      currentLocation,
                      (controller?.zoom ?? zoom) - 1,
                      0,
                    ),
                    child: const Icon(Icons.remove),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
