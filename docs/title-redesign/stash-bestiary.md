# Stash and Bestiary reference layout

2026-10-28: narrow left icon rail, central responsive grid, right inspector separated by a thin rule, and bottom controls.

Bestiary uses existing All/Recorded/Undiscovered filters in the icon rail; search and filter menu move to the footer. The grid has six columns at the desktop fixture width and five at compact width. Discovery gating, portraits, drops and milestone reward claims remain native to Bestiary. Unknown creatures stay unknown.

Stash uses icon-only categories, selected-item name/artwork in the right inspector, grade ordering at the lower left, and Withdraw at the lower right when available. Empty-state instructions occupy the grid area only.

Validation: C# compilation succeeds and both desktop/compact galleries were refreshed. Existing Unity Editor layout stabilization errors still prevent a clean overall test result. Current default fixtures have an empty Stash and undiscovered Bestiary records.
