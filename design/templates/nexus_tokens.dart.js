// Style Dictionary formatter — generates ui/lib/src/theme/nexus_tokens.g.dart
// Do not edit the output file directly; edit design/tokens.json instead.

export function format({ dictionary }) {
  const colors = dictionary.allTokens.filter(t => t.path[0] === 'nexus' && t.path[1] === 'color');
  const grid   = dictionary.allTokens.filter(t => t.path[0] === 'nexus' && t.path[1] === 'grid');
  const type   = dictionary.allTokens.filter(t => t.path[0] === 'nexus' && t.path[1] === 'type');

  // nexus.color.screenBg → screenBg  (drop 'nexus' + group, keep camelCase)
  const fieldName = t => {
    const parts = t.path.slice(2);
    return parts[0] + parts.slice(1).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join('');
  };

  const desc = t => t.$description || '';

  const colorField = t => {
    const hex = (t.$value || '').replace('#', '');
    const r = hex.slice(0,2), g = hex.slice(2,4), b = hex.slice(4,6);
    return `  /// ${desc(t)}\n  static const Color ${fieldName(t)} = Color(0xff${r}${g}${b});`;
  };

  const numField = t => {
    const v = t.$value;
    const isInt = Number.isInteger(v);
    const typeName = isInt ? 'int' : 'double';
    return `  /// ${desc(t)}\n  static const ${typeName} ${fieldName(t)} = ${v};`;
  };

  return `// GENERATED — do not edit. Source: design/tokens.json
// Regenerate: cd design && npm run build

import 'package:flutter/material.dart';

/// Hardware display color tokens.
///
/// These are the only color values permitted in ZonePainter and all
/// Widgetbook use cases. Plugins never choose colors — the host maps
/// Severity → color using this table.
///
/// Two namespaces must never mix:
///   NexusColors — what is painted to the 640×48 hardware panel
///   AppColors   — the Flutter settings UI chrome
class NexusColors {
${colors.map(colorField).join('\n\n')}
}

/// Hardware display grid constants (native 640×48 pixel coordinate space).
class NexusGrid {
${grid.map(numField).join('\n\n')}
}

/// Hardware display type scale (font sizes in pixels).
class NexusTypeScale {
${type.map(numField).join('\n\n')}
}
`;
}
