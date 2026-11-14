# Bundled fonts

Compiled into the binary so the strip looks the same on every system.

| File | Role | Source | Licence |
|---|---|---|---|
| `IBMPlexSans.ttf` | text choice (default), variable weight | google/fonts `ofl/ibmplexsans` | SIL OFL 1.1 |
| `AtkinsonHyperlegible-Regular.ttf` | text choice | google/fonts `ofl/atkinsonhyperlegible` | SIL OFL 1.1 |
| `Roboto.ttf` | text choice, variable weight | google/fonts `ofl/roboto` | SIL OFL 1.1 |
| `NotoSans-Regular.ttf` | text choice | Arch `noto-fonts` | SIL OFL 1.1 |
| `Lato-Regular.ttf` | text choice | google/fonts `ofl/lato` | SIL OFL 1.1 |
| `DejaVuSans.ttf` | fallback for characters the chosen font lacks | Arch `ttf-dejavu` | Bitstream Vera / public domain |
| `FontAwesome-Solid.ttf` | icons | Font Awesome Free | SIL OFL 1.1 (fonts) |

Licence texts are in `licenses/`.

Every text choice must be a TrueType font with hinting instructions
(`fpgm` and `prep` tables). The renderer relies on the font's own hinting
for crisp 10 px text.
