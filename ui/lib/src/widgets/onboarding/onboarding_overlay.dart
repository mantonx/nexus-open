import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/settings_state.dart';
import '../../theme/app_tokens.dart';
import '../common/common.dart';
import '../settings/tabs/location_tab.dart';

/// Full-screen onboarding shown on first launch (when the backend has no
/// existing config file). Steps: Welcome → Connect → Location → Done.
class OnboardingOverlay extends StatefulWidget {
  const OnboardingOverlay({super.key});

  @override
  State<OnboardingOverlay> createState() => _OnboardingOverlayState();
}

class _OnboardingOverlayState extends State<OnboardingOverlay> {
  int _step = 0;
  static const int _totalSteps = 4;

  void _next() => setState(() => _step++);

  void _finish() {
    // Mark first run done so the overlay doesn't show again this session.
    context.read<SettingsState>().dismissFirstRun();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      backgroundColor: theme.colorScheme.surface,
      body: SafeArea(
        child: Column(
          children: [
            // Progress indicator
            LinearProgressIndicator(
              value: (_step + 1) / _totalSteps,
              color: theme.colorScheme.tertiary,
              backgroundColor:
                  theme.colorScheme.tertiary.withOpacity(0.15),
            ),

            Expanded(
              child: AnimatedSwitcher(
                duration: const Duration(milliseconds: 250),
                child: KeyedSubtree(
                  key: ValueKey(_step),
                  child: _buildStep(context),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildStep(BuildContext context) {
    switch (_step) {
      case 0:
        return _WelcomeStep(onNext: _next);
      case 1:
        return _ConnectStep(onNext: _next);
      case 2:
        return _LocationStep(onNext: _next);
      case 3:
        return _DoneStep(onFinish: _finish);
      default:
        return const SizedBox.shrink();
    }
  }
}

// ── Step widgets ─────────────────────────────────────────────────────────────

class _WelcomeStep extends StatelessWidget {
  final VoidCallback onNext;
  const _WelcomeStep({required this.onNext});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return _StepShell(
      icon: Icons.display_settings,
      title: 'Welcome to Nexus Open',
      body: 'Open-source control for your Corsair iCUE Nexus display.\n\n'
          'This quick setup will get you connected and configured in under '
          'a minute.',
      buttonLabel: 'Get started',
      onNext: onNext,
      theme: theme,
    );
  }
}

class _ConnectStep extends StatelessWidget {
  final VoidCallback onNext;
  const _ConnectStep({required this.onNext});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final settings = context.watch<SettingsState>();

    return _StepShell(
      icon: settings.isConnected ? Icons.usb : Icons.usb_off,
      iconColor: settings.isConnected ? Colors.green : Colors.orange,
      title: 'Connect your device',
      body: settings.isConnected
          ? 'iCUE Nexus found and connected. '
          : 'Plug in your iCUE Nexus via USB.\n\n'
              'If you see a permission error, run:\n'
              '  sudo scripts/setup-udev.sh\n'
              'then unplug and replug the device.',
      buttonLabel: settings.isConnected ? 'Continue' : 'Skip for now',
      onNext: onNext,
      theme: theme,
    );
  }
}

class _LocationStep extends StatelessWidget {
  final VoidCallback onNext;
  const _LocationStep({required this.onNext});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final settings = context.watch<SettingsState>();

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 32, 24, 0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.location_on,
                  size: 48, color: theme.colorScheme.tertiary),
              const SizedBox(height: 16),
              Text('Choose your location',
                  style: theme.textTheme.headlineSmall),
              const SizedBox(height: 8),
              Text(
                'Used for the weather module. You can change this later.',
                style: theme.textTheme.bodyMedium,
              ),
              const SizedBox(height: 16),
            ],
          ),
        ),
        Expanded(
          child: LocationTab(
            onLocationSelected: (loc) {
              settings.updateConfig(location: loc);
            },
            initialLocation: settings.location,
          ),
        ),
        Padding(
          padding: AppSpacing.paddingLg,
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
  final VoidCallback onFinish;
  const _DoneStep({required this.onFinish});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return _StepShell(
      icon: Icons.check_circle_outline,
      iconColor: AppColors.success,
      title: 'All set!',
      body: 'Nexus Open is ready. Your display should be updating now.\n\n'
          'Use the settings panel to customise modules, colors, and more.',
      buttonLabel: 'Open settings',
      onNext: onFinish,
      theme: theme,
    );
  }
}

// ── Shared shell ─────────────────────────────────────────────────────────────

class _StepShell extends StatelessWidget {
  final IconData icon;
  final Color? iconColor;
  final String title;
  final String body;
  final String buttonLabel;
  final VoidCallback onNext;
  final ThemeData theme;

  const _StepShell({
    required this.icon,
    required this.title,
    required this.body,
    required this.buttonLabel,
    required this.onNext,
    required this.theme,
    this.iconColor,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: AppSpacing.paddingXl,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon,
              size: AppIconSize.hero,
              color: iconColor ?? AppColors.accent),
          const SizedBox(height: AppSpacing.lg),
          Text(title, style: theme.textTheme.headlineMedium),
          const SizedBox(height: AppSpacing.md),
          Text(body, style: theme.textTheme.bodyLarge),
          const SizedBox(height: AppSpacing.xl),
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
