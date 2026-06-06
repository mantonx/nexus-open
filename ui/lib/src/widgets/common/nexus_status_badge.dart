import 'package:flutter/material.dart';
import '../../theme/app_tokens.dart';

/// Semantic status levels used throughout the app.
enum NexusStatus { ok, warning, error, loading, unknown }

/// A small pill badge that communicates plugin/connection/device status.
///
/// Replaces all [Colors.green] / [Colors.orange] / [Colors.red] hardcoding.
/// Colours come from the [AppSemanticColors] extension, so they adapt to
/// light and dark themes automatically.
///
/// Usage:
///   NexusStatusBadge(status: NexusStatus.ok, label: 'Connected')
///   NexusStatusBadge(status: NexusStatus.error, label: 'Timeout')
///   NexusStatusBadge.dot(status: NexusStatus.warning)  // icon-only variant
class NexusStatusBadge extends StatelessWidget {
  const NexusStatusBadge({
    super.key,
    required this.status,
    this.label,
    this.size = BadgeSize.small,
  });

  /// Icon-only dot variant — no label, minimal footprint.
  const NexusStatusBadge.dot({
    super.key,
    required this.status,
  })  : label = null,
        size = BadgeSize.dot;

  final NexusStatus status;
  final String? label;
  final BadgeSize size;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final theme = Theme.of(context);
    final color = _color(cs);

    if (size == BadgeSize.dot) {
      return _dot(color);
    }

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.sm,
        vertical: AppSpacing.xs / 2,
      ),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: AppRadius.pillBr,
        border: Border.all(color: color.withValues(alpha: 0.35)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _dot(color),
          if (label != null) ...[
            const SizedBox(width: AppSpacing.xs),
            Text(
              label!,
              style: theme.textTheme.labelSmall?.copyWith(color: color),
            ),
          ],
        ],
      ),
    );
  }

  Widget _dot(Color color) {
    if (status == NexusStatus.loading) {
      return SizedBox(
        width: AppIconSize.xs,
        height: AppIconSize.xs,
        child: CircularProgressIndicator(
          strokeWidth: 1.5,
          valueColor: AlwaysStoppedAnimation<Color>(color),
        ),
      );
    }
    return Container(
      width: AppIconSize.xs,
      height: AppIconSize.xs,
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
        boxShadow: status == NexusStatus.ok
            ? [BoxShadow(color: color.withValues(alpha: 0.6), blurRadius: 6)]
            : null,
      ),
    );
  }

  Color _color(ColorScheme cs) {
    switch (status) {
      case NexusStatus.ok:
        return cs.success;
      case NexusStatus.warning:
        return cs.warning;
      case NexusStatus.error:
        return cs.critical;
      case NexusStatus.loading:
        return cs.dataAccent;
      case NexusStatus.unknown:
        return cs.onSurfaceVariant;
    }
  }
}

/// Converts a raw status string from the API into a [NexusStatus].
extension NexusStatusParsing on String {
  NexusStatus toNexusStatus() {
    switch (toLowerCase()) {
      case 'ok':
        return NexusStatus.ok;
      case 'error':
        return NexusStatus.error;
      case 'timeout':
        return NexusStatus.warning;
      case 'loading':
        return NexusStatus.loading;
      default:
        return NexusStatus.unknown;
    }
  }
}

enum BadgeSize { small, dot }
