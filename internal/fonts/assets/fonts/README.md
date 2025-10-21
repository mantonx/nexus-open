# Embedded Fonts

This directory contains embedded TrueType fonts for the Nexus Open display.

## Adding Fonts

To embed fonts in the binary, place `.ttf` files here. The application will try to load these fonts first before falling back to system fonts.

### Recommended Fonts

For the best display quality on the 640x48 pixel display, use monospace fonts:

1. **DejaVu Sans Mono** (Recommended)
   - Download: https://dejavu-fonts.github.io/
   - File: `DejaVuSansMono.ttf`
   - License: Free (public domain)

2. **Liberation Mono**
   - Included in most Linux distributions
   - File: `LiberationMono-Regular.ttf`
   - License: SIL Open Font License

3. **Ubuntu Mono**
   - Download: https://design.ubuntu.com/font/
   - File: `UbuntuMono-R.ttf`
   - License: Ubuntu Font License

## Installation

```bash
# Download DejaVu fonts
cd /tmp
wget https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.tar.bz2
tar xjf dejavu-fonts-ttf-2.37.tar.bz2

# Copy to project (optional - will use system fonts if not present)
cp dejavu-fonts-ttf-2.37/ttf/DejaVuSansMono.ttf internal/display/assets/fonts/
```

## Font Loading Order

The application tries to load fonts in this order:

1. **Embedded fonts** (this directory)
2. **System fonts** (various paths in /usr/share/fonts/)
3. **Fallback** (built-in bitmap font)

If no TrueType fonts are found, the application will fall back to the basic bitmap font.

## Licensing

Make sure any fonts you embed are licensed for distribution. All fonts mentioned above are free and open source.
