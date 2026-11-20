# ThemePicker

Mode and color theme in Settings, Appearance: a segmented control for System, Light, Dark and a row of four theme swatches.

## Themes
| Theme | Neutrals | Accent | For |
|---|---|---|---|
| Mint (default) | charcoal from `#1F1F24` | mint `#98E0BF` | the brand look |
| Lilac | violet-tinted | lilac `#BCAAF7` | a softer, cooler room |
| Sand | warm paper | teal `#86CADB` | long reading in daylight |
| Contrast | pure black and white | white or black, yellow or blue focus | low vision, bright rooms, projectors |

Each theme has a light and a dark set; Mode picks which one. The eight sets are the eight token themes (`mint-dark` … `contrast-light`).

## Rules
- A swatch draws the theme itself: its panel, canvas strip, two text bars and an accent button. The name sits under it; the chosen one gets an accent ring and a check.
- Status colors (working, needs you, ready, error) keep their hue in every theme, so status reads the same everywhere. Only Contrast shifts them for more contrast.
- The brand mint and the mascot never change with the theme.
- A theme applies at once, including open terminals when "Terminal colors" is on.
- Every text and status token holds 4.5:1 on canvas, panel, raised and hover in all eight sets; control borders and focus hold 3:1.
