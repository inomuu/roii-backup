# Area 6+ map placeholder (2026-09-11)

Area 6 and later currently reuse the authored Area 1–5 environment in
`Zone_r1_Painterly_3Rooms`. Their old generated terrain/dressing is no longer
selected while this placeholder is enabled.

`ArenaView.UseEarlyAreaMapPlaceholder` defaults to true. The presentation-only
`EnvironmentRegionOfZone` resolves later regions to baked region 1 before the
ground rebuild decision. Actual zone IDs, enemies, area labels, save progression,
band transitions and the three-room camera march continue using the real area.
No backend deployment or save migration is needed.

Disable the flag when replacement area maps are ready. The legacy builders/assets
remain in source for development scenes; they are not instantiated by this path.

Verified in the idle Unity Editor: no compilation errors; areas 1, 5, 6, 10, 11
and 30 resolve to the authored template for all three rooms. This verifies map
selection; a full traversal of every area has not been play-tested.
