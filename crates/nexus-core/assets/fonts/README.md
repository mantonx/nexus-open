# Bundled fonts

Compiled into the binary so the strip looks the same on every system.

| File | Role | Source | Licence |
|---|---|---|---|
| `Rubik.ttf` | every numeral, and the default words face (`DESIGN.md`), variable weight 300–900 | google/fonts `ofl/rubik`, v2.300 | SIL OFL 1.1 |
| `IBMPlexSans.ttf` | words choice, variable weight | google/fonts `ofl/ibmplexsans` | SIL OFL 1.1 |
| `AtkinsonHyperlegible-Regular.ttf` | words choice | google/fonts `ofl/atkinsonhyperlegible` | SIL OFL 1.1 |
| `Roboto.ttf` | words choice, variable weight | google/fonts `ofl/roboto` | SIL OFL 1.1 |
| `NotoSans-Regular.ttf` | words choice | Arch `noto-fonts` | SIL OFL 1.1 |
| `Lato-Regular.ttf` | words choice | google/fonts `ofl/lato` | SIL OFL 1.1 |
| `DejaVuSans.ttf` | fallback for characters the chosen font lacks | Arch `ttf-dejavu` | Bitstream Vera / public domain |
| `Phosphor-Fill.ttf`, `Phosphor-Bold.ttf` | every icon (`src/icons.rs`) | `@phosphor-icons/web` 2.1.2 on npm, subset to the 67 glyphs `DESIGN.md` lists (fontTools `pyftsubset`) | MIT |

Licence texts are in `licenses/`.

The `font` setting chooses the words face only: labels, titles, key
labels. Numerals are always Rubik, so a reading keeps its shape and width
whatever face is chosen. Rubik ships without TrueType hinting programs
(no `fpgm`); the strip's smallest text is 15 px (`theme::LABEL`), where its
outlines draw cleanly without them.

The other words choices must be TrueType fonts with hinting instructions
(`fpgm` and `prep` tables); the renderer relies on the font's own hinting
for crisp small text.
