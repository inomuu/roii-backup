# Reference-inspired presentation pass — 2026-10-28

Keep the existing navy, violet, ivory and gold palette. Borrow hierarchy and quieter surfaces from the supplied examples, not their artwork or game systems.

Implemented:
- Shared menu and dialog surfaces now have one thin neutral edge; primary actions use a rounded violet silhouette and gold outline, without side diamonds or nested gold frames.
- Management-page backdrops use a quiet navy/violet gradient instead of the citadel texture, crest, rings and ambient particles, keeping foreground character/item artwork prominent.
- Inventory detail/card backdrops use the same violet palette. Equipment picker tiles have rounded corners and a quieter inspector divider.
- Daily/weekly task rows have more vertical breathing room and larger reward rows; progress and claim behavior remain unchanged.
- Existing category navigation, item grids, detail inspectors and gameplay controls remain functional. No reference-game mechanics or data were introduced.

Validation: C# compilation succeeded. Refresh desktop/compact and dialog galleries. The existing Unity MainToolbarWindow layout errors contaminate capture error counts, so these captures do not establish a clean test pass or online connectivity.
