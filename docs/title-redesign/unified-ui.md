# Shared UI theme revision

2026-10-28: unified the shared menu material with the title UI: midnight navy surfaces, muted silver frame edges and restrained warm gold selection. Existing display/body typography and cut-corner geometry are retained. Character art, rarity/element colors and gameplay state colors are unchanged.

UiTheme now owns the frame top/bottom, primary/selected fill and normal/raised edge colors. CodexFrameMesh consumes these values across GameMenuSurface-backed menus, dialogs, cards and buttons. Ghost interaction colors use the same surface palette; Exemplar navigation and roster shades use shared colors instead of independent literals. Title dialogs already consume DialogSurface and the same theme.

Validation uses the existing UiGalleryProbe presentation fixtures for main screens, compact screens, dialogs, system pages and online UI states. These captures do not authenticate, purchase or mutate an account. Results are recorded in D:/roii/unified-ui-validation.txt; captures are under D:/roii/idle_mmorpg/ui-gallery/. An online fixture passing does not establish live-server functionality. Source assets unavailable for Suzu can still produce placeholders; this theme change does not provide missing character artwork.
