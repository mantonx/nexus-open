import 'dart:io';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';

/// Full-screen onboarding shown on first launch.
/// Steps: Welcome → Connect → Done.
class OnboardingOverlay extends StatefulWidget {
  const OnboardingOverlay({super.key});

  @override
  State<OnboardingOverlay> createState() => _OnboardingOverlayState();
}

class _OnboardingOverlayState extends State<OnboardingOverlay> {
  int _step = 0;
  static const int _totalSteps = 3;

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
      case 2: return _DoneStep(onFinish: _finish);
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
  });

  final IconData icon;
  final Color? iconColor;
  final String title;
  final String body;
  final String buttonLabel;
  final VoidCallback onNext;

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
              color: AppColors.accent.withValues(alpha: 0.10),
              borderRadius: AppRadius.smBr,
              border: Border.all(
                color: AppColors.accent.withValues(alpha: 0.25),
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

class _ConnectStep extends StatefulWidget {
  const _ConnectStep({required this.onNext});
  final VoidCallback onNext;

  @override
  State<_ConnectStep> createState() => _ConnectStepState();
}

class _ConnectStepState extends State<_ConnectStep> {
  static const _udevRulePath =
      '/usr/lib/udev/rules.d/99-corsair-nexus.rules';

  bool get _udevInstalled => File(_udevRulePath).existsSync();

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsState>();
    final connected = settings.deviceConnected;
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final udev = _udevInstalled;

    // Auto-advance when device connects — no tap needed.
    if (connected) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) widget.onNext();
      });
    }

    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.xl, AppSpacing.xxl, AppSpacing.xl, AppSpacing.lg),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Icon
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: (connected
                      ? cs.success
                      : AppColors.accent)
                  .withValues(alpha: 0.10),
              borderRadius: AppRadius.smBr,
              border: Border.all(
                color: (connected
                        ? cs.success
                        : AppColors.accent)
                    .withValues(alpha: 0.25),
              ),
            ),
            child: Icon(
              connected ? Icons.usb : Icons.usb_off_outlined,
              size: AppIconSize.xl,
              color: connected ? cs.success : AppColors.accent,
            ),
          ),
          const SizedBox(height: AppSpacing.lg),

          Text(
            'CONNECT DEVICE',
            style: theme.textTheme.labelSmall?.copyWith(
              color: AppColors.accent,
              letterSpacing: 1.5,
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Text('Connect your device',
              style: theme.textTheme.headlineMedium),
          const SizedBox(height: AppSpacing.md),

          // Contextual instructions
          if (!connected) ...[
            if (!udev) ...[
              // First-time setup — udev not installed yet
              const _Instruction(
                step: '1',
                text: 'Run this once in a terminal:',
              ),
              const _CodeBlock('sudo nexus-open --setup-udev'),
              const SizedBox(height: AppSpacing.sm),
              const _Instruction(
                step: '2',
                text: 'Unplug and replug your iCUE Nexus.',
              ),
              const SizedBox(height: AppSpacing.sm),
              const _Instruction(
                step: '3',
                text: 'This screen will update automatically.',
              ),
            ] else ...[
              // udev is installed — just needs plugging in
              const _Instruction(
                step: '1',
                text: 'Plug your iCUE Nexus into a USB port.',
              ),
              const SizedBox(height: AppSpacing.sm),
              const _Instruction(
                step: '2',
                text: 'This screen will update automatically.',
              ),
            ],
            const SizedBox(height: AppSpacing.md),
            Row(children: [
              const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: AppColors.accent,
                ),
              ),
              const SizedBox(width: AppSpacing.sm),
              Text(
                'Waiting for device…',
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: cs.onSurfaceVariant),
              ),
            ]),
          ] else ...[
            Text(
              'iCUE Nexus found and connected.',
              style: theme.textTheme.bodyLarge
                  ?.copyWith(color: cs.onSurfaceVariant, height: 1.6),
            ),
          ],

          const SizedBox(height: AppSpacing.md),
          NexusStatusBadge(
            status: connected ? NexusStatus.ok : NexusStatus.warning,
            label:
                connected ? 'Device connected' : 'No device detected',
          ),

          const Spacer(),

          NexusButton.primary(
            label: connected ? 'Continue' : 'Skip for now',
            onPressed: widget.onNext,
            expand: true,
          ),
        ],
      ),
    );
  }
}

class _Instruction extends StatelessWidget {
  const _Instruction({required this.step, required this.text});
  final String step;
  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 20,
          height: 20,
          margin: const EdgeInsets.only(top: 2),
          decoration: BoxDecoration(
            color: AppColors.accent.withValues(alpha: 0.15),
            shape: BoxShape.circle,
          ),
          child: Center(
            child: Text(
              step,
              style: theme.textTheme.labelSmall?.copyWith(
                color: AppColors.accent,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
        ),
        const SizedBox(width: AppSpacing.sm),
        Expanded(
          child: Text(
            text,
            style: theme.textTheme.bodyMedium
                ?.copyWith(color: cs.onSurfaceVariant, height: 1.5),
          ),
        ),
      ],
    );
  }
}

class _CodeBlock extends StatelessWidget {
  const _CodeBlock(this.command);
  final String command;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(left: 28, top: AppSpacing.xs),
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.md, vertical: AppSpacing.sm),
      decoration: BoxDecoration(
        color: Colors.black.withValues(alpha: 0.3),
        borderRadius: AppRadius.smBr,
        border: Border.all(color: AppColors.darkBorder),
      ),
      child: Text(
        command,
        style: theme.textTheme.bodySmall?.copyWith(
          fontFamily: 'monospace',
          color: AppColors.accent,
        ),
      ),
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
          'Use the settings panel to customise plugins, colors, and more.',
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
      ..color = Colors.white.withValues(alpha: 0.035)
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
