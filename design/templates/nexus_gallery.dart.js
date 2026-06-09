// Style Dictionary formatter — generates ui/lib/src/theme/nexus_gallery.g.dart
// A scrollable token catalog widget for in-app reference (no Widgetbook needed).

export function format({ dictionary }) {
    const colors = dictionary.allTokens.filter(t => t.path[0] === 'nexus' && t.path[1] === 'color');

    const fieldName = token => {
      const parts = token.path.slice(2);
      return parts[0] + parts.slice(1).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join('');
    };

    const colorRows = colors.map(t => {
      const name = fieldName(t);
      const comment = t.$description || name;
      return `          const _ColorRow(label: '${name}', comment: '${comment}', color: NexusColors.${name}),`;
    }).join('\n');

    return `// GENERATED — do not edit. Source: design/tokens.json
// Regenerate: cd design && npm run build

import 'package:flutter/material.dart';
import 'nexus_tokens.g.dart';

/// Scrollable catalog of all NexusColors tokens.
/// Mount this widget during development to visually verify the token palette.
/// Usage: Navigator.push(context, MaterialPageRoute(builder: (_) => const NexusTokenGallery()));
class NexusTokenGallery extends StatelessWidget {
  const NexusTokenGallery({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: NexusColors.screenBg,
      appBar: AppBar(
        backgroundColor: NexusColors.bezel,
        title: const Text('Nexus token gallery', style: TextStyle(color: Color(0xFFECECEC), fontSize: 14)),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: const [
          _SectionHeader(label: 'Colors'),
${colorRows}
          _SectionHeader(label: 'Grid'),
          _GridSection(),
          _SectionHeader(label: 'Type scale'),
          _TypeSection(),
        ],
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String label;
  const _SectionHeader({required this.label});
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 20, bottom: 8),
    child: Text(label, style: const TextStyle(color: Color(0xFF9A9A9A), fontSize: 10, letterSpacing: 1.2)),
  );
}

class _ColorRow extends StatelessWidget {
  final String label;
  final String comment;
  final Color color;
  const _ColorRow({required this.label, required this.comment, required this.color});

  @override
  Widget build(BuildContext context) {
    final hex = '#\${color.toARGB32().toRadixString(16).padLeft(8, '0').substring(2).toUpperCase()}';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(children: [
        Container(width: 24, height: 24, decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(4), border: Border.all(color: const Color(0xFF2A2A2A)))),
        const SizedBox(width: 12),
        Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(label, style: const TextStyle(color: Color(0xFFECECEC), fontSize: 12)),
          Text(comment, style: const TextStyle(color: Color(0xFF6F6F6F), fontSize: 10)),
        ])),
        Text(hex, style: const TextStyle(color: Color(0xFF6F6F6F), fontSize: 10, fontFamily: 'monospace')),
      ]),
    );
  }
}

class _GridSection extends StatelessWidget {
  const _GridSection();
  @override
  Widget build(BuildContext context) => const Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
    _NumRow(label: 'displayWidth',   value: '\${NexusGrid.displayWidth}px'),
    _NumRow(label: 'displayHeight',  value: '\${NexusGrid.displayHeight}px'),
    _NumRow(label: 'slotCount',      value: '\${NexusGrid.slotCount}'),
    _NumRow(label: 'slotWidth',      value: '\${NexusGrid.slotWidth}px'),
    _NumRow(label: 'contentPadLeft', value: '\${NexusGrid.contentPadLeft}px'),
    _NumRow(label: 'labelBaselineY', value: '\${NexusGrid.labelBaselineY}px'),
    _NumRow(label: 'valueBaselineY', value: '\${NexusGrid.valueBaselineY}px'),
  ]);
}

class _TypeSection extends StatelessWidget {
  const _TypeSection();
  @override
  Widget build(BuildContext context) => Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
    _TypeRow(label: 'label',        size: NexusTypeScale.sizeLabel.toDouble()),
    _TypeRow(label: 'value',        size: NexusTypeScale.sizeValue.toDouble()),
    _TypeRow(label: 'valueDetail',  size: NexusTypeScale.sizeValueDetail.toDouble()),
    _TypeRow(label: 'unit',         size: NexusTypeScale.sizeUnit.toDouble()),
    _TypeRow(label: 'caption',      size: NexusTypeScale.sizeCaption.toDouble()),
  ]);
}

class _NumRow extends StatelessWidget {
  final String label;
  final String value;
  const _NumRow({required this.label, required this.value});
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 3),
    child: Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
      Text(label, style: const TextStyle(color: Color(0xFF9A9A9A), fontSize: 11)),
      Text(value,  style: const TextStyle(color: Color(0xFFECECEC), fontSize: 11, fontFamily: 'monospace')),
    ]),
  );
}

class _TypeRow extends StatelessWidget {
  final String label;
  final double size;
  const _TypeRow({required this.label, required this.size});
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 4),
    child: Row(children: [
      SizedBox(width: 90, child: Text(label, style: const TextStyle(color: Color(0xFF9A9A9A), fontSize: 10))),
      Text('Aa', style: TextStyle(color: NexusColors.value, fontSize: size)),
      const SizedBox(width: 8),
      Text('\${size.toInt()}px', style: const TextStyle(color: Color(0xFF6F6F6F), fontSize: 10, fontFamily: 'monospace')),
    ]),
  );
}
`;
}
