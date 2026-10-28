# Reign of Idlers — futuristic fantasy UI

## Current direction — 2026-10-10

The user rejected generated gold frames, requested Arknights + Wuthering Waves inspiration, then clarified futuristic fantasy plus UX improvements. The current native Unity UI uses deep blue-black surfaces, white icon silhouettes, cyan selection accents, thin rules and plain high-contrast primary buttons. No generated panel, icon-frame or button textures remain in active Assets. Rejected sources are archived outside the project at `D:/roii/ui-rejected-mobile-art`.

## Shared rules

- Keep the existing character art and gameplay routes. Do not import assets from reference games.
- UiTheme owns the neutral palette, Noto UI typography and quiet bevels. Preserve rarity and element colors.
- Home keeps character-centered composition with open white icon navigation and a translucent bottom strip.
- Management screens use a static charcoal gradient, with static fine-line octagonal arcane geometry and sparse diagonal circuitry; no bitmap frame decoration.
- Character information sits on a dark translucent side panel. Preserve the prototype-art disclosure.
- Use white filled primary actions, quiet secondary surfaces and visible keyboard focus.
- Preserve localized labels, hotkeys, touch targets and existing game actions.
- Inventory capacity remains visible. Render owned items only; zero items shows localized guidance. Empty Stash provides an Inventory action. Long grids scroll above the fixed footer.
- Home groups use a titled modal, dim backdrop with click-to-dismiss, explicit up/down keyboard navigation, and focus return on close.
- Shop retains a responsive 2–6 column grid with existing item artwork, prices and actions.
- UI UX Pro Max was used earlier for contrast, hierarchy and scroll-affordance guidance. Its dashboard/marketing suggestions are not the visual target for this native game.

## Validation scope

Use UiGalleryProbe.After and Compact for isolated fixtures at 1440×900 and 1024×768. Inspect screenshots and test scrolling, back navigation and character pages. This fixture does not log in, write saves or validate live server transactions. Missing environment packs remain separate from UI work.

## UI UX Pro Max evidence for this revision

Executed the local skill search script; this is not a hosted API.
- `gaming futuristic fantasy immersive --design-system`: gaming result, but feature-rich marketing showcase and hyperrealism recommendations do not fit native Unity menus; not adopted wholesale.
- Narrowed `HUD sci fi futuristic --domain style`: verified top match `hud-sci-fi-fui`. Adopt thin cyan lines, angular details, dark/translucent surfaces and technical geometry. Omit blinking, scanning and heavy glow to protect legibility.
- `empty state guidance --domain ux`: top match Feedback / Empty States. Applied helpful message plus next action.
- `navigation progressive disclosure --domain ux` was off-target web guidance; narrowed to `menu active state --domain ux`, yielding Navigation / Active State and Interaction / Active States. Preserve selected/focus indicators.

Native adaptation: retained uGUI meshes and TMP; no CSS, ARIA or web routing introduced. No newly generated images. No decorative Update animation.
