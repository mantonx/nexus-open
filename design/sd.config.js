import StyleDictionary  from 'style-dictionary';
import { format as goFormat }      from './templates/tokens.go.js';
import { format as dartFormat }    from './templates/nexus_tokens.dart.js';
import { format as galleryFormat } from './templates/nexus_gallery.dart.js';

// Register custom formats before building.
StyleDictionary.registerFormat({ name: 'nexus/go',           format: goFormat });
StyleDictionary.registerFormat({ name: 'nexus/dart',         format: dartFormat });
StyleDictionary.registerFormat({ name: 'nexus/dart-gallery', format: galleryFormat });

const sd = new StyleDictionary({
  source: ['tokens.json'],

  platforms: {
    go: {
      transformGroup: 'js',
      files: [{
        destination: '../internal/design/tokens.go',
        format: 'nexus/go',
      }],
    },

    dart: {
      transformGroup: 'js',
      files: [
        {
          destination: '../ui/lib/src/theme/nexus_tokens.g.dart',
          format: 'nexus/dart',
        },
        {
          destination: '../ui/lib/src/theme/nexus_gallery.g.dart',
          format: 'nexus/dart-gallery',
        },
      ],
    },
  },
});

await sd.buildAllPlatforms();
