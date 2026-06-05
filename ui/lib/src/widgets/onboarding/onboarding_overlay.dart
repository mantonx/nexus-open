import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';
import '../settings/tabs/location_tab.dart';

/// Full-screen onboarding shown on first launch.
/// Steps: Welcome → Connect → Location → Done.
class OnboardingOverlay extends StatefulWidget {
  const OnboardingOverlay({super.key});

  @override
  State<OnboardingOverlay> createState() => _OnboardingOverlayState();
}

class _OnboardingOverlayState extends State<OnboardingOverlay> {
  int _step = 0;
  static const int _totalSteps = 4;

  void _next() => setState(() => _step++);

  void _finish() => context.read<SettingsState>().dismissFirstRun();

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      backgroundColor: cs.surfaceContainerLowest,
      body: Stack(
        children: [
          // Dot-grid background — matches settings page atmosphere
          Positioned.fill(child: CustomPaint(painter: _OnboardingGridPainter())),

          SafeArea(
            child: Column(
              children: [
                // Step progress bar
                _StepProgress(step: _step, total: _totalSteps),

                Expanded(
                  child: AnimatedSwitcher(
                    duration: AppDuration.normal,
                    child: KeyedSubtree(
                      key: ValueKey(_step),
                      child: _buildStep(context),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildStep(BuildContext context) {
    switch (_step) {
      case 0: return _WelcomeStep(onNext: _next);
      case 1: return _ConnectStep(onNext: _next);
      case 2: return _LocationStep(onNext: _next);
      case 3: return _DoneStep(onFinish: _finish);
      default: return const SizedBox.shrink();
    }
  }
}

// ── Step progress indicator ───────────────────────────────────────────────────

class _StepProgress extends StatelessWidget {
  const _StepProgress({required this.step, required this.total});
  final int step;
  final int total;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.lg, AppSpacing.md, AppSpacing.lg, 0),
      child: Row(
        children: [
          for (var i = 0; i < total; i++) ...[
            if (i > 0) const SizedBox(width: AppSpacing.xs),
            Expanded(
              child: AnimatedContainer(
                duration: AppDuration.normal,
                height: 3,
                decoration: BoxDecoration(
                  color: i <= step
                      ? AppColors.accent
                      : AppColors.darkBorder,
                  borderRadius: AppRadius.pillBr,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

// ── Shared step shell ─────────────────────────────────────────────────────────

class _StepShell extends StatelessWidget {
  const _StepShell({
    required this.icon,
    required this.title,
    required this.body,
    required this.buttonLabel,
    required this.onNext,
    this.iconColor,
    this.statusBadge,
  });

  final IconData icon;
  final Color? iconColor;
  final String title;
  final String body;
  final String buttonLabel;
  final VoidCallback onNext;
  final Widget? statusBadge;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.xl, AppSpacing.xxl, AppSpacing.xl, AppSpacing.lg),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Icon in a hardware-flavoured container
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: AppColors.accent.withOpacity(0.10),
              borderRadius: AppRadius.smBr,
              border: Border.all(
                color: AppColors.accent.withOpacity(0.25),
                width: 1,
              ),
            ),
            child: Icon(
              icon,
              size: AppIconSize.xl,
              color: iconColor ?? AppColors.accent,
            ),
          ),
          const SizedBox(height: AppSpacing.lg),

          // Step label
          Text(
            title.toUpperCase(),
            style: theme.textTheme.labelSmall?.copyWith(
              color: AppColors.accent,
              letterSpacing: 1.5,
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),

          // Title
          Text(title, style: theme.textTheme.headlineMedium),
          const SizedBox(height: AppSpacing.md),

          // Body
          Text(
            body,
            style: theme.textTheme.bodyLarge?.copyWith(
              color: cs.onSurfaceVariant,
              height: 1.6,
            ),
          ),

          if (statusBadge != null) ...[
            const SizedBox(height: AppSpacing.md),
            statusBadge!,
          ],

          const Spacer(),

          NexusButton.primary(
            label: buttonLabel,
            onPressed: onNext,
            expand: true,
          ),
        ],
      ),
    );
  }
}

// ── Step widgets ──────────────────────────────────────────────────────────────

class _WelcomeStep extends StatelessWidget {
  const _WelcomeStep({required this.onNext});
  final VoidCallback onNext;

  @override
  Widget build(BuildContext context) {
    return _StepShell(
      icon: Icons.display_settings_outlined,
      title: 'Welcome to Nexus Open',
      body: 'Open-source control for your Corsair iCUE Nexus display.\n\n'
          'This quick setup will get you connected and configured in '
          'under a minute.',
      buttonLabel: 'Get started',
      onNext: onNext,
    );
  }
}

class _ConnectStep extends StatelessWidget {
  const _ConnectStep({required this.onNext});
  final VoidCallback onNext;

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final connected = settings.isConnected;

    return _StepShell(
      icon: connected ? Icons.usb : Icons.usb_off_outlined,
      iconColor: connected
          ? Theme.of(context).colorScheme.success
          : AppColors.accent,
      title: 'Connect your device',
      body: connected
          ? 'iCUE Nexus found and connected.'
          : 'Plug in your iCUE Nexus via USB.\n\n'
              'If you see a permission error, run:\n'
              '  sudo scripts/setup-udev.sh\n'
              'then unplug and replug the device.',
      buttonLabel: connected ? 'Continue' : 'Skip for now',
      onNext: onNext,
      statusBadge: NexusStatusBadge(
        status: connected ? NexusStatus.ok : NexusStatus.warning,
        label: connected ? 'Device connected' : 'No device detected',
      ),
    );
  }
}

class _LocationStep extends StatelessWidget {
  const _LocationStep({required this.onNext});
  final VoidCallback onNext;

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.xl, AppSpacing.xxl, AppSpacing.xl, 0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                width: 56,
                height: 56,
                decoration: BoxDecoration(
                  color: AppColors.accent.withOpacity(0.10),
                  borderRadius: AppRadius.smBr,
                  border: Border.all(
                    color: AppColors.accent.withOpacity(0.25),
                    width: 1,
                  ),
                ),
                child: const Icon(Icons.location_on_outlined,
                    size: AppIconSize.xl, color: AppColors.accent),
              ),
              const SizedBox(height: AppSpacing.lg),
              Text(
                'LOCATION',
                style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: AppColors.accent,
                  letterSpacing: 1.5,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: AppSpacing.sm),
              Text('Choose your location',
                  style: Theme.of(context).textTheme.headlineMedium),
              const SizedBox(height: AppSpacing.sm),
              Text(
                'Used for the weather module. You can change this later.',
                style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: AppSpacing.md),
            ],
          ),
        ),
        Expanded(
          child: LocationTab(
            onLocationSelected: (loc) => settings.updateConfig(location: loc),
            initialLocation: settings.location,
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.xl, AppSpacing.sm, AppSpacing.xl, AppSpacing.lg),
          child: NexusButton.primary(
            label: 'Continue',
            onPressed: onNext,
            expand: true,
          ),
        ),
      ],
    );
  }
}

class _DoneStep extends StatelessWidget {
  const _DoneStep({required this.onFinish});
  final VoidCallback onFinish;

  @override
  Widget build(BuildContext context) {
    return _StepShell(
      icon: Icons.check_circle_outline,
      iconColor: Theme.of(context).colorScheme.success,
      title: 'All set!',
      body: 'Nexus Open is ready. Your display should be updating now.\n\n'
          'Use the settings panel to customise modules, colors, and more.',
      buttonLabel: 'Open settings',
      onNext: onFinish,
    );
  }
}

// ── Dot-grid background — matches settings page ───────────────────────────────

class _OnboardingGridPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.white.withOpacity(0.035)
      ..strokeWidth = 1;
    const step = 24.0;
    for (double x = step; x < size.width; x += step) {
      for (double y = step; y < size.height; y += step) {
        canvas.drawCircle(Offset(x, y), 1, paint);
      }
    }
  }

  @override
  bool shouldRepaint(_OnboardingGridPainter _) => false;
}
