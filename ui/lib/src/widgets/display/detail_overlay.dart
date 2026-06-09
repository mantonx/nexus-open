// DetailOverlay — animated chrome layer for the detail view.
//
// When active=true the overlay slides down (y: -height → 0) over the display
// preview. The Go frame stream already contains the detail pixels; this widget
// only adds interactive chrome: a grab handle, swipe-down dismiss, and a close
// tap region in the top-right corner.
//
// Usage:
//   DetailOverlay(
//     active: _detailActive,
//     displaySize: Size(displayW, displayH),
//     onDismiss: () => _api.tapZone(closeButtonX),
//     child: RawImage(image: _uiImage, fit: BoxFit.fill),
//   )

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import '../../theme/nexus_tokens.g.dart';

class DetailOverlay extends StatefulWidget {
  const DetailOverlay({
    super.key,
    required this.active,
    required this.displaySize,
    required this.onDismiss,
    required this.child,
  });

  /// Whether the detail view is currently showing.
  final bool active;

  /// Pixel dimensions of the display preview widget (not hardware pixels).
  final Size displaySize;

  /// Called when the user swipes down or taps the close region.
  final VoidCallback onDismiss;

  /// The display content (RawImage or ColoredBox placeholder).
  final Widget child;

  @override
  State<DetailOverlay> createState() => _DetailOverlayState();
}

class _DetailOverlayState extends State<DetailOverlay>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl;
  late final Animation<Offset> _slide;

  // Accumulated vertical drag since pan start, in logical pixels.
  double _dragY = 0;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 120),
    );
    _slide = Tween<Offset>(
      begin: const Offset(0, -1), // fully above
      end: Offset.zero,           // fully visible
    ).animate(CurvedAnimation(parent: _ctrl, curve: Curves.easeOut));

    if (widget.active) _ctrl.value = 1.0;
  }

  @override
  void didUpdateWidget(DetailOverlay old) {
    super.didUpdateWidget(old);
    if (widget.active != old.active) {
      if (widget.active) {
        _dragY = 0;
        _ctrl.forward();
      } else {
        _ctrl.reverse();
      }
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  void _onDragUpdate(DragUpdateDetails d) {
    if (!widget.active) return;
    _dragY += d.delta.dy;
    // Mirror drag into animation value so the overlay follows the finger.
    final progress = (_dragY / widget.displaySize.height).clamp(0.0, 1.0);
    _ctrl.value = 1.0 - progress;
  }

  void _onDragEnd(DragEndDetails d) {
    if (!widget.active) return;
    if (_dragY > 12 || d.velocity.pixelsPerSecond.dy > 200) {
      widget.onDismiss();
    } else {
      // Snap back.
      _dragY = 0;
      _ctrl.forward();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.active && _ctrl.isDismissed) {
      // Overlay fully hidden — render only the child with no overhead.
      return widget.child;
    }

    return SlideTransition(
      position: _slide,
      child: GestureDetector(
        onVerticalDragUpdate: _onDragUpdate,
        onVerticalDragEnd: _onDragEnd,
        behavior: HitTestBehavior.translucent,
        child: Stack(
          fit: StackFit.expand,
          children: [
            widget.child,
            // Grab handle — centered, top of overlay.
            Positioned(
              top: _scaleY(2.5),
              left: 0,
              right: 0,
              child: Center(
                child: Container(
                  width: _scaleX(NexusGrid.grabHandleW.toDouble()),
                  height: _scaleY(NexusGrid.grabHandleH),
                  decoration: BoxDecoration(
                    color: const Color(0xFF4A4A4A),
                    borderRadius: BorderRadius.circular(1.25),
                  ),
                ),
              ),
            ),
            // Close tap region — top-right ~100×24px of the overlay.
            Positioned(
              top: 0,
              right: 0,
              width: _scaleX(100),
              height: _scaleY(24),
              child: GestureDetector(
                onTap: widget.onDismiss,
                behavior: HitTestBehavior.opaque,
                child: const SizedBox.expand(),
              ),
            ),
          ],
        ),
      ),
    );
  }

  // Scale from hardware pixels (640×48 space) to widget pixels.
  double _scaleX(double hardwareX) =>
      hardwareX / NexusGrid.displayWidth * widget.displaySize.width;

  double _scaleY(double hardwareY) =>
      hardwareY / NexusGrid.displayHeight * widget.displaySize.height;
}

// ── DetailBlitter ──────────────────────────────────────────────────────────────
//
// Widgetbook / test helper — blits a raw 640×48 RGBA byte buffer to canvas.
// Not used in the live app (Go renders into the frame stream directly).

class DetailBlitter extends StatelessWidget {
  const DetailBlitter({
    super.key,
    required this.rgbaBytes,
  });

  /// Raw RGBA bytes, 640×48×4 = 122 880 bytes.
  final Uint8List rgbaBytes;

  static const _w = NexusGrid.displayWidth;
  static const _h = NexusGrid.displayHeight;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(painter: _RawFramePainter(rgbaBytes));
  }
}

class _RawFramePainter extends CustomPainter {
  _RawFramePainter(this.bytes);

  final Uint8List bytes;

  @override
  void paint(Canvas canvas, Size size) {
    // decodeImageFromPixels is async; for CustomPainter we pre-decode in
    // a FutureBuilder wrapper. This painter just blits whatever image was
    // passed in via FutureBuilder (see DetailBlitterAsync below).
    // Here we draw a placeholder until the image is ready.
    canvas.drawRect(
      Offset.zero & size,
      Paint()..color = NexusColors.screenBg,
    );
  }

  @override
  bool shouldRepaint(_RawFramePainter old) => old.bytes != bytes;
}

/// Async wrapper that decodes the raw RGBA buffer before painting.
class DetailBlitterAsync extends StatefulWidget {
  const DetailBlitterAsync({super.key, required this.rgbaBytes});

  final Uint8List rgbaBytes;

  @override
  State<DetailBlitterAsync> createState() => _DetailBlitterAsyncState();
}

class _DetailBlitterAsyncState extends State<DetailBlitterAsync> {
  ui.Image? _image;

  @override
  void initState() {
    super.initState();
    _decode(widget.rgbaBytes);
  }

  @override
  void didUpdateWidget(DetailBlitterAsync old) {
    super.didUpdateWidget(old);
    if (old.rgbaBytes != widget.rgbaBytes) _decode(widget.rgbaBytes);
  }

  void _decode(Uint8List bytes) {
    ui.decodeImageFromPixels(
      bytes,
      DetailBlitter._w,
      DetailBlitter._h,
      ui.PixelFormat.rgba8888,
      (img) {
        if (!mounted) { img.dispose(); return; }
        final old = _image;
        setState(() => _image = img);
        old?.dispose();
      },
    );
  }

  @override
  void dispose() {
    _image?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final img = _image;
    if (img == null) {
      return const ColoredBox(color: NexusColors.screenBg);
    }
    return RawImage(image: img, fit: BoxFit.fill);
  }
}
