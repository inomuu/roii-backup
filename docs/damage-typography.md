# Arena damage typography — 2026-09-09

All damage numbers now use the bundled SIL OFL Crimson Text Semibold Italic SDF:
ordinary outgoing hits, damage received by the player, individual multi-hits,
multi-hit totals, and the amount above BREAK. This approximates the smooth italic
serif numerals in the Octopath reference; it is not the original game font.
Status/heal labels and ordinary HUD text retain Pixel Operator.

ArenaHud.NumberLabel uses a separate shared numeric material, a 1.3 optical size
factor, a charcoal outline (0.12), face dilate 0.035, and a small dark underlay.
The upper face is slightly lighter than the existing semantic color. Existing
white/red/yellow damage categories and popup timing/placement are preserved.
Pooled labels reset font, material, gradient and transform state on reuse.

## Verification

Compiled and reviewed in the master Unity project with no console errors.
DamageTypographyReview.Run() is an editor-only Play Mode specimen using the same
SpawnDamage and FeedMultiHit methods as combat, without applying simulation damage.
It temporarily pauses and then restores the driver, waits for a death overlay to
clear, and captures ordinary 29, received 128, four-digit 9999 and multi-hit total
100 (24 + 35 + 41). Capture timing is approximate; the image includes fading hits.
Unity was left in Play Mode. No standalone build or mobile visual check in this pass.

![Damage font specimen](art-review-break/2026-09-09/damage-font/damage-typography.png)
