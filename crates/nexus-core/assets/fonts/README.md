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
| `Rubik.ttf` | the design system's text face (U2, `DESIGN.md`), variable weight 300–900 | google/fonts `ofl/rubik`, v2.300 | SIL OFL 1.1 |
| `Phosphor-Fill.ttf`, `Phosphor-Bold.ttf` | the design system's icons | `@phosphor-icons/web` 2.1.2 on npm, subset to the 66 glyphs `DESIGN.md` lists (fontTools `pyftsubset`) | MIT |

Licence texts are in `licenses/`.

Rubik and the Phosphor subsets are used by the design-system frames
(`examples/design_system.rs`) and are not yet a font choice. Rubik ships
without TrueType hinting programs (no `fpgm`), so it is used at 14 px and
above only; it needs checking on the panel before it becomes a choice.

Every text choice must be a TrueType font with hinting instructions
(`fpgm` and `prep` tables). The renderer relies on the font's own hinting
for crisp 10 px text.
