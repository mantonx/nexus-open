import 'dart:io';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:file_picker/file_picker.dart';
import '../../../models/settings_state.dart';
import '../../../services/nexus_api_service.dart';
import '../../../theme/app_tokens.dart';
import '../../common/common.dart';

class ImagesTab extends StatefulWidget {
  const ImagesTab({super.key});

  @override
  State<ImagesTab> createState() => _ImagesTabState();
}

class _ImagesTabState extends State<ImagesTab> {
  bool _uploading = false;

  Future<void> _pickAndUploadImage(SettingsState state) async {
    final result = await FilePicker.platform
        .pickFiles(type: FileType.image, allowMultiple: false);
    if (result == null || result.files.isEmpty) return;

    setState(() => _uploading = true);
    try {
      final api = NexusApiService();
      final filename = await api.uploadImage(File(result.files.first.path!));
      state.addImagePath(filename);
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('Image uploaded')));
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Upload failed: $e')));
      }
    } finally {
      setState(() => _uploading = false);
    }
  }

  Future<void> _deleteImage(SettingsState state, String filename) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Delete image'),
        content: Text('Delete "$filename"?'),
        actions: [
          NexusButton.ghost(
              label: 'Cancel', onPressed: () => Navigator.pop(ctx, false)),
          NexusButton.destructive(
              label: 'Delete', onPressed: () => Navigator.pop(ctx, true)),
        ],
      ),
    );
    if (ok != true) return;

    try {
      final api = NexusApiService();
      await api.deleteImage(filename);
      state.removeImagePath(filename);
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('Image deleted')));
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Delete failed: $e')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final state = context.watch<SettingsState>();

    return Padding(
      padding: AppSpacing.paddingMd,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Background Images',
                        style: theme.textTheme.headlineSmall),
                    Text('PNG, JPG, GIF — shown behind zone content.',
                        style: theme.textTheme.bodySmall),
                  ],
                ),
              ),
              const SizedBox(width: AppSpacing.sm),
              NexusButton.primary(
                label: _uploading ? 'Uploading…' : 'Upload',
                icon: _uploading
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white))
                    : const Icon(Icons.add_photo_alternate, size: 18),
                onPressed: _uploading
                    ? null
                    : () => _pickAndUploadImage(state),
              ),
            ],
          ),
          const SizedBox(height: AppSpacing.md),

          // Grid or empty state
          Expanded(
            child: state.imagePaths.isEmpty
                ? _EmptyState()
                : GridView.builder(
                    gridDelegate:
                        const SliverGridDelegateWithFixedCrossAxisCount(
                      crossAxisCount: 2,
                      crossAxisSpacing: AppSpacing.sm,
                      mainAxisSpacing: AppSpacing.sm,
                      childAspectRatio: 2.0,
                    ),
                    itemCount: state.imagePaths.length,
                    itemBuilder: (context, i) {
                      final filename = state.imagePaths[i];
                      return _ImageTile(
                        filename: filename,
                        isSelected: state.backgroundImage == filename,
                        onSelect: () => state.setBackgroundImage(filename),
                        onDelete: () => _deleteImage(state, filename),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

// ── Image tile ────────────────────────────────────────────────────────────────

class _ImageTile extends StatelessWidget {
  const _ImageTile({
    required this.filename,
    required this.isSelected,
    required this.onSelect,
    required this.onDelete,
  });

  final String filename;
  final bool isSelected;
  final VoidCallback onSelect;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return NexusCard(
      padding: EdgeInsets.zero,
      accentBorder: isSelected,
      onTap: onSelect,
      child: Stack(
        fit: StackFit.expand,
        children: [
          // Image
          ClipRRect(
            borderRadius: AppRadius.lgBr,
            child: Image.network(
              '${NexusApiService.baseUrl}/api/images/$filename',
              fit: BoxFit.cover,
              errorBuilder: (_, __, ___) => Container(
                color: cs.surfaceContainerHigh,
                child: Icon(Icons.broken_image,
                    size: AppIconSize.xl,
                    color: cs.onSurfaceVariant),
              ),
            ),
          ),

          // Gradient overlay for readability
          Positioned(
            bottom: 0,
            left: 0,
            right: 0,
            child: Container(
              height: 32,
              decoration: BoxDecoration(
                borderRadius: const BorderRadius.only(
                  bottomLeft: Radius.circular(AppRadius.lg),
                  bottomRight: Radius.circular(AppRadius.lg),
                ),
                gradient: LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [Colors.transparent, Colors.black54],
                ),
              ),
              padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.xs, vertical: 4),
              child: Text(
                filename,
                style: const TextStyle(
                    color: Colors.white,
                    fontSize: 10,
                    overflow: TextOverflow.ellipsis),
              ),
            ),
          ),

          // Selected check
          if (isSelected)
            Positioned(
              top: AppSpacing.xs,
              left: AppSpacing.xs,
              child: Container(
                padding: const EdgeInsets.all(3),
                decoration: BoxDecoration(
                  color: AppColors.accent,
                  borderRadius: AppRadius.xsBr,
                ),
                child: const Icon(Icons.check,
                    size: AppIconSize.sm, color: Colors.white),
              ),
            ),

          // Delete button
          Positioned(
            top: AppSpacing.xs,
            right: AppSpacing.xs,
            child: IconButton(
              icon: const Icon(Icons.close,
                  size: AppIconSize.sm, color: Colors.white),
              style: IconButton.styleFrom(
                backgroundColor: Colors.black54,
                padding: const EdgeInsets.all(4),
                minimumSize: const Size(24, 24),
              ),
              onPressed: onDelete,
            ),
          ),
        ],
      ),
    );
  }
}

// ── Empty state ───────────────────────────────────────────────────────────────

class _EmptyState extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          // Hardware-adjacent icon: a miniature display outline with a + glyph
          Container(
            width: 64,
            height: 64,
            decoration: BoxDecoration(
              border: Border.all(
                color: AppColors.darkBorder,
                width: 1.5,
              ),
              borderRadius: AppRadius.smBr,
            ),
            child: Center(
              child: Container(
                width: 40,
                height: 10,
                decoration: BoxDecoration(
                  border: Border.all(
                    color: AppColors.accent.withOpacity(0.4),
                    width: 1,
                  ),
                  borderRadius: BorderRadius.circular(2),
                ),
                child: Center(
                  child: Icon(
                    Icons.add,
                    size: 8,
                    color: AppColors.accent.withOpacity(0.5),
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(height: AppSpacing.md),
          Text(
            'No images yet',
            style: theme.textTheme.titleSmall?.copyWith(
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: AppSpacing.xs),
          Text(
            'PNG, JPG or GIF — shown behind zone content\non the 640×48 display.',
            textAlign: TextAlign.center,
            style: theme.textTheme.bodySmall?.copyWith(
              color: AppColors.textMuted,
            ),
          ),
        ],
      ),
    );
  }
}
