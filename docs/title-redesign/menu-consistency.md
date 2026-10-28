# Shared menu treatment

2026-10-28: use the Shop menu as the reference for suitable management pages.

- UiWindow retains the shared back/icon/title/context header and now uses the same opaque navy surface as standalone management screens.
- Exemplar pages use quiet icon-and-label category buttons with a gold active marker on the content-facing edge. Existing page routes and deployment gates are unchanged.
- CollectionBrowser (equipment selection) uses the same header surface and selected category treatment.
- MenuNavigationButton shares subdued violet interaction feedback; inactive category icons are muted, selected icons are gold, and disabled state stays distinguishable.
- Combat, Home, and small dialogs retain their purpose-specific layouts.

Validation: C# compilation succeeded. Desktop and compact gallery captures were refreshed. Unity Editor MainToolbarWindow reports a recurring layout stabilization error, so gallery runs are diagnostic captures, not clean test passes. Online account behavior was not exercised.
