// Device tab — connection status, firmware info, per-zone health.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../../services/nexus_api_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

class DeviceTab extends StatefulWidget {
  const DeviceTab({super.key});

  @override
  State<DeviceTab> createState() => _DeviceTabState();
}

class _DeviceTabState extends State<DeviceTab> {
  DeviceInfo? _info;
  bool _loading = true;
  String? _error;
  late NexusApiService _api;
  Timer? _pollTimer;
  bool _initialized = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api = context.read<NexusApiService>();
    if (!_initialized) {
      _initialized = true;
      _fetchInfo();
      _pollTimer = Timer.periodic(const Duration(seconds: 5), (_) => _fetchInfo());
    }
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchInfo() async {
    try {
      final info = await _api.getDeviceInfo();
      if (mounted) setState(() { _info = info; _loading = false; _error = null; });
    } catch (e) {
      if (mounted) setState(() { _loading = false; _error = e.toString(); });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return ListView(
      padding: AppSpacing.paddingMd,
      children: [
        // ── Device info ───────────────────────────────────────────────────
        if (_loading)
          const Center(child: CircularProgressIndicator(color: AppColors.accent))
        else if (_error != null)
          NexusCard(
            child: Row(
              children: [
                Icon(Icons.error_outline, size: AppIconSize.sm, color: cs.error),
                const SizedBox(width: AppSpacing.sm),
                Expanded(child: Text(_error!, style: theme.textTheme.bodySmall?.copyWith(color: cs.error))),
              ],
            ),
          )
        else if (_info != null) ...[
          NexusSection(
            title: 'Device',
            child: Column(
              children: [
                if (_info!.manufacturer.isNotEmpty)
                  _InfoRow(label: 'Manufacturer', value: _info!.manufacturer),
                _InfoRow(label: 'Model', value: _info!.model),
                _InfoRow(label: 'Firmware', value: _info!.firmware),
                if (_info!.vendorId.isNotEmpty)
                  _InfoRow(label: 'Vendor ID', value: _info!.vendorId),
                if (_info!.productId.isNotEmpty)
                  _InfoRow(label: 'Product ID', value: _info!.productId),
              ],
            ),
          ),
          if (_info!.connectError != null && _info!.connectError!.isNotEmpty) ...[
            const SizedBox(height: AppSpacing.sm),
            NexusCard(
              child: Row(
                children: [
                  Icon(Icons.warning_amber_outlined, size: AppIconSize.sm, color: cs.warning),
                  const SizedBox(width: AppSpacing.sm),
                  Expanded(
                    child: Text(
                      _info!.connectError!,
                      style: theme.textTheme.bodySmall?.copyWith(color: cs.onSurfaceVariant),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ],
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
      child: Row(
        children: [
          SizedBox(
            width: 80,
            child: Text(label, style: theme.textTheme.labelSmall?.copyWith(color: cs.onSurfaceVariant)),
          ),
          Expanded(child: Text(value, style: theme.textTheme.bodySmall)),
        ],
      ),
    );
  }
}
