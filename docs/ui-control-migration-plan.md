# UI Control Migration Plan — every `_draw` panel → Controls in a CanvasLayer

> Decided 2026-07-20 after the Exemplar-screen debugging ordeal (see §6 lessons).
> Goal: ONE input/render model for all UI. `_draw` remains ONLY for the world layer
> (arena, projectiles/FX, animators) per the CLAUDE.md UI rule. Status legend:
> ☐ pending · ◐ in progress · ☑ done.

## 1. End state

- Every screen/window/popup = **Control nodes inside a CanvasLayer** (retained,
  cached, auto-translated via `tr()`, GUI hit-testing).
- One shared **window base** so every screen behaves identically (dim, centering,
  Esc/hotkey close, HudBar sync).
- The world keeps `_draw`: `arena.gd`, `arena_fx.gd`, `field_fx.gd`,
  `player_animator.gd`, `multi_hit_popup.gd`, sprites. **Out of scope forever.**
- Leaf widgets MAY still paint with `_draw` *inside* a Control (custom painting is
  retained-friendly); what's banned is `_draw`-based screens doing their own
  hit-testing/input.

## 2. Inventory (what must move)

| Screen | Lines | Effort | Notes |
|---|---|---|---|
| sidebar_view | 1100 | XL | split into sub-scenes while migrating (§4 P4) |
| skill_tree_view | 722 | L | node buttons + custom-drawn edges leaf |
| album_view | 626 | L | needs CardButton (wraps card_render) |
| overlay_view | 622 | L | ALWAYS-ON → biggest perf win |
| modals_view | 551 | M | mostly dissolves (tooltip layer + refine popup) |
| enchant_view | 535 | M | |
| steam_market_view | 482 | M | |
| card_panel_view | 411 | M | CardButton reuse |
| companion_view | 399 | M | desktop widget; special-case, LAST |
| creation_view | 337 | M | |
| refine_view | 300 | M | |
| options_view | 261 | S | mostly buttons/sliders — easy win |
| paths_view | 257 | M | web layout like skill tree |
| shop_view | 235 | S | pilot candidate |
| party_view | 190 | S | pilot candidate |
| stash_view | 187 | S | pilot candidate (validates ItemSlotButton) |
| debug_view | 154 | S | |
| dps_view | 108 | S | |
| inventory_picker | 94 | S | becomes a shared Control picker |
| item_slot_display | 36 | S | absorbed by ItemSlotButton |

Already Control (align to the base in passing): hud_bar, craft, mailbox, market,
item_market, chat, advancement, world_map, exemplar, mode_select, loading,
session_ended, sync_banner, hover_tooltip_layer, bestiary_view.

## 3. Phase 0 — Foundations (BEFORE any screen moves)

1. **`views/ui_window.gd` — the window base (CanvasLayer).** Provides: full-rect
   dim (STOP), centered PanelContainer, `open()/close()/toggle()`,
   `visibility_changed` → layer sync (HudBar sets `.visible` directly — must keep
   working), Esc handling, viewport-resize re-center. Exemplar_view becomes the
   first consumer (it already does all this ad-hoc — extract, don't invent).
2. **Layer map (constants in ui_window.gd):** 10 = windows · 20 = sub-popups
   (pickers/confirms) · 30 = hover tooltips · 128+ = system (mode badge 128,
   loading 129, session-ended 130). Move exemplar (64) + hover_tooltip_layer into
   the scheme.
3. **`views/item_slot_button.gd` — leaf Control wrapping the existing
   `ItemSlot.draw_slot` painting** (custom `_draw` inside a Button-like Control:
   keeps the pixel-perfect look, gains real hover/click/tooltip). Same for
   **CardButton** wrapping `card_render.gd` when the album moves.
4. **HudBar `_toggle_window` stays `.visible`-based** — the base's visibility sync
   makes that safe for every window (lesson: never bypass; never add
   click-outside-close).
5. **Retained rules (enforced in review):** no `state_updated` → rebuild; diff or
   fingerprint before touching children; build once in `_ready`; no
   ScrollContainer unless the content truly scrolls (it eats clicks via
   drag-capture — if needed, test clicks explicitly).

## 4. Phases 1-5 — screens in order

- **P1 pilots (S-size, prove the base + ItemSlotButton):** stash → shop → party →
  options → dps → debug → inventory_picker. One screen = one commit = one
  in-game click test by the user (I cannot see the screen — §5).
- **P2 always-on:** overlay_view (perf: kills the unconditional every-frame
  redraw). Top bar = party frames (Buttons) + band picker grid.
- **P3 heavy screens, one per slice:** album (+CardButton) → card_panel →
  enchant → refine → creation → steam_market → paths → skill_tree.
- **P4 sidebar (XL):** split per one-scene-one-responsibility while migrating:
  `paperdoll_panel` / `stats_panel` / `inventory_grid` / `skill_bar` sub-scenes
  under one window. modals_view dissolves here: tooltips → hover_tooltip_layer,
  refine confirm → its own ui_window.
- **P5 cleanup:** delete dead `_draw` helpers + `ItemSlot.draw_slot`'s screen
  callers (keep it for ItemSlotButton's internal paint), drop per-frame
  `queue_redraw` loops, sweep `Loc`-bypass strings met along the way (drawn
  strings become `tr()`-auto-translated Labels — i18n bonus).

## 5. Working agreements (risk control)

- **One screen per commit**, tests green, then STOP for the user's in-game click
  test before the next (new .gd flows need a human eye — memory rule).
- **Shared worktree:** check `git status` before touching a file the other
  session may hold; never `git commit -a`.
- **No behavior changes while migrating** — same actions, same sim_driver calls,
  same Loc keys. Refactor and feature never mix in one slice.
- **Debug tool of first resort** when clicks misbehave:
  `get_viewport().gui_get_hovered_control()` (null = blocked/not registered;
  foreign node = something stealing; huge instance id = churn).
- Companion (always-on-top widget) and creation flow are fragile/rare — migrate
  LAST, after the pattern is boring.

## 6. Why (lessons that forced this — 2026-07-20)

1. Mixed input models hide global breakage: `_draw` screens bypass the GUI stage,
   so a screen-wide GUI blocker went unnoticed until the first real Control
   screen hit it.
2. `_draw` habits are toxic in retained UI: a 10 Hz rebuild loop (from the
   `queue_redraw`-every-tick reflex) freed Buttons mid-press — unclickable UI.
3. Controls parented under plain Node2D don't anchor/hit-test reliably —
   CanvasLayer is the correct host; visibility must sync via `visibility_changed`.
