# Reign of Idlers — UI design system (MASTER)

Updated 2026-11-02. The code is the source of truth: `unity/ROI/Assets/Scripts/UI/UiTheme.cs`
(tokens), `WuwaStyle.cs` (panel / divider / bracket meshes), `DialogSurface.cs` (every modal's
skin). This file records the values and the rules behind them; if they disagree, fix this file.

## Direction

Futuristic fantasy in a Wuthering Waves idiom: near-black glass with a faint blue cast, cut
corners (top-left and bottom-right) as the one silhouette, low-alpha hairlines, and ONE warm
gold accent spent only on what matters. Confirm actions are ivory plates. No bitmap frames, no
assets from reference games, no decorative Update animation (companion mode must stay cheap).

The 2026-10-10 "cyan accent" direction below is superseded by the gold/ivory palette.

## Colour tokens (`UiTheme`)

| Token | Hex | Use |
|---|---|---|
| `Panel` | `#0F1012` | Window ground |
| `Surface` | `#1B1C1F` | Raised: title bar, filled slot, button at rest |
| `SurfaceHover` | `#2A2B2E` | Hover |
| `SurfaceActive` | `#37352F` | Selected cell (one step up; selection is carried by the gold edge, not the fill) |
| Dialog glass | `#121316` @ 97% | `NoticeDialogStyle.Glass` |
| Dialog row | `#1D1E21` @ 90% | Setting rows, input fields inside dialogs |
| `Accent` | `#EDD48F` | The one warm accent: focus ring, selected edge, key numbers |
| `Ivory` / `IvoryShade` | `#EDE8D9` / `#CCC4B0` | Primary (confirm) plate, top→bottom gradient |
| `OnAccent` | `#1A1A1C` | Text on ivory / accent |
| `TextHigh` | `#F5F5F0` | Titles, button labels |
| `TextMid` | `#C7C7C4` | Body copy |
| `TextLow` | `#A8A8A8` | Captions, hints (must stay ≥ 4.5:1 on every surface) |
| `Warning` | `#F2786B` | Destructive titles, errors |
| `Line` / `LineStrong` | `#DBDBD6` @ 13% / 30% | Hairlines / button outlines |
| `Scrim` | `#030304` @ 76% | Behind a modal; blocks clicks, tap = cancel |

Rarity and element colours are semantic and are never repainted by a theme pass
(`NoticeDialogStyle.Semantic`: saturation > .3 and value > .35 keeps its colour).

### Measured contrast (dialog glass over scrim)

TextHigh 16.9:1 · TextMid 10.9:1 · TextLow 7.8:1 · Accent 12.7:1 · Warning 6.8:1 ·
OnAccent on Ivory 14.3:1. All text passes WCAG AA; keep new pairs at ≥ 4.5:1 (body) and
≥ 3:1 (large text, focus indicators, component boundaries).

## Typography

- Faces: Marcellus (`UiTheme.Display`) for window/dialog titles and names; Bai Jamjuree
  (`EclipseUI SDF`) for all readable UI, Latin and Thai. Thai titles fall back to the UI face.
- Sizes are whole numbers only (fractional sizes blur glyphs):
  `TitleSize 21` window name · `HeadSize 16` section head / button · `BodySize 15` rows ·
  `CaptionSize 13` counts and footnotes.
- Dialogs: title 24 centred with a `——◇——` divider under it; body 16; plain buttons 17.
- One shared sharpened TMP material for every label (no per-label material copies).

## Spacing and size

- Scale: `Gap 8` · `PadSmall 12` · `Pad 16` · `PadLarge 24`.
- `TitleBarH 54`, `ButtonH 40`; touch target `UiLayout.Touch 44` (dialog buttons and the close
  icon are 44 tall/square minimum).
- Dialog: 24 padding, 12 between the title row and body (room for the divider), 16 above the
  button row, 8 between buttons.

## Components

### Dialog (`DialogSurface` / `NoticeDialogStyle`)
- Cut-corner glass (cut 14), hairline edge in Accent @ 32%, short gold accent along the top
  edge. No corner brackets on dialogs (removed 2026-11-02 at the user's request; cards keep them).
- Close ✕ top-right, 44×44 hit area, always last sibling.
- Escape and scrim tap both cancel. Initial focus goes to the SAFE choice (Cancel).

### Buttons in a dialog
- Primary (confirm): ivory plate, dark label, ◯ glyph when ≥ 130 wide. Chosen by, in order:
  a `UiPrimaryAction` marker (HudBar `primary:true`, ForgeUi `accent:true`), else the single
  filled `Btn_` beside a `Ghost_` cancel.
- Secondary / cancel: dark glass plate, `LineStrong` outline, 1 px top highlight, ✕ glyph.
- One primary per dialog. A destructive confirm still uses the ivory plate; the danger is
  carried by the Warning-coloured title and the wording.
- Focus: 2 px Accent ring 3 px outside the plate (visible on both plate types).
- Disabled: plate dimmed and label at 45% alpha. A disabled button must not look enabled.

### Copy
- Name the action on the confirm button ("Refund", "Move and equip"), not "Confirm", when a
  loc key exists for it. All strings go through `Loc` (`godot/i18n/strings.csv`).

## Validation

`UiGalleryProbe.Dialogs94Compact` / `More94Compact` / `System93` (via `.roi_cmd`) capture
disposable fixtures to `D:/roii/idle_mmorpg/ui-gallery/` — no login, saves or network. Inspect the
PNGs; a probe run is not evidence of keyboard behaviour or low power use.

## Known gaps (not yet fixed)

- Untitled confirms (FloatingInventoryHost `Confirm`: gem warning, crest disenchant) have no
  title and a generic "Confirm" label; they need loc keys for a title and a named action.
- Ghost-button outline (`LineStrong` on glass) is 2.3:1, under the 3:1 boundary guideline; the
  label carries the affordance today.
- The forced "labels 18–26 → 16" rule in `NoticeDialogStyle` flattens subheadings into body size.

## History — 2026-10-10 direction (superseded palette)

The user rejected generated gold frames, requested Arknights + Wuthering Waves inspiration, then
clarified futuristic fantasy plus UX improvements. Rules from that pass that still hold: keep the
character art and gameplay routes; home keeps a character-centred composition; management screens
use a static charcoal gradient with fine-line geometry; character info on a dark translucent side
panel; inventory capacity visible, empty states give guidance plus a next action; home group modals
dim, dismiss on click, support up/down keys and return focus on close; the shop keeps its
responsive 2–6 column grid. Rejected art is archived at `D:/roii/ui-rejected-mobile-art`.
UI UX Pro Max (local search script) informed contrast, hierarchy, empty-state and active-state
guidance; its marketing/dashboard suggestions are not the visual target for this native game.
