# Built-in image generation prompts

Generated with the built-in image_gen tool, not the CLI/API fallback. No generated pixels were
repainted or replaced procedurally. The source PNG alpha is preserved.

## Generation 1

Style reference inputs: the supplied Sword of Convallaria screenshots taken at 07.37.09 and
07.37.23 on 2026-09-24. These are visual references, not evidence of the source game's engine.

```text
Use case: stylized-concept.
Asset type: production VFX flipbook texture atlas for a Unity sword spin attack, not an illustration of a spritesheet.
Transform the circular blue-white sword energy effect shown in the supplied gameplay screenshots into an isolated hand-painted animation sprite sheet. The screenshots are STYLE references only: use the broad razor-white hooked blade edge, cyan inner rim, translucent cobalt energy sheet, layered sweeping ribbons and tiny luminous fragments. Remove ALL characters, scenery, UI, browser, text and objects.
Output one square 2048x2048 texture atlas, exactly 4 columns by 4 rows of equally sized square cells, chronological left-to-right then top-to-bottom, 16 different consecutive frames of ONE continuous nonlooping 0.4-second sword spin. Absolutely no grid lines, gutters, labels, frame numbers or watermark. Background throughout must be pure solid RGB black (0,0,0) for additive blending, including empty center.
IMPORTANT CAMERA: orthographic TOP DOWN, looking straight down onto the horizontal circular slash plane. The circumference is circular, NOT a foreshortened ellipse; Unity will provide perspective later. The exact center stays at the center of each cell. Fixed camera, fixed scale. Main ring radius about 36% of cell width; black hole radius about 25%. Every streak/spark fully contained within each cell with at least 6% black margin. Nothing crosses between cells.
Temporal storyboard, with geometrically coherent forward motion:
Row 1: frame1 faint short cyan arc starting at 9 o'clock; frame2 sharp white curved blade grows to 90 degrees clockwise; frame3 extends to 180 degrees; frame4 to 270 degrees with a strong white cutting edge and blue trailing sheet.
Row 2: frames5-8 peak almost-full 330 degree slash. Broad tapering white outer crescent with 2 or 3 asymmetrical hooked spear-like tips and an inset cyan blade. The short opening and leading tip advance smoothly clockwise by about 25 degrees per frame. This is a sword cut, not fire, not a liquid splash, not an electric portal. White blade occupies about 8% of cell width at its thickest. Strong crisp solid white geometric silhouettes, painted blue interior wakes, delicate glow outside only. Keep center empty.
Row 3: frames9-12 main white blade breaks progressively into 3 then 5 tapering curved fragments along the SAME circumference while blue sheets follow clockwise. White intensity progressively decreases, radius grows only slightly (under 8%). Small cyan sparks detach outward, sparse not confetti.
Row 4: frames13-16 dissipate continuously from thin blue arc fragments and a few white slivers into faint blue traces and finally only a couple extremely faint sparks. No restart or new flash.
Style: premium HD-2D JRPG attack VFX, hand-painted animation silhouettes, hard luminous razor edges with subtle organic variation frame-to-frame, restrained blue glow. Smooth raster artwork, NOT pixelated or blocky. Temporal continuity is more important than making sixteen independent impressive icons. No smoke cloud, no huge radial explosion, no lens flare, no background gradient.
```

## Refinement 2

Edit input: generation 1. The requested padding correction was only partially followed;
faint cell-boundary artifacts remain in the source. The runtime shader has a documented edge guard.

```text
Edit this VFX animation atlas. Keep exactly the existing 4 by 4 equally sized cell layout and all 16 chronological animation phases, the white/cyan razor-like sword crescents, central pivot, top-down camera and blue palette. This is a technical texture-atlas cleanup, NOT a redesign. Inside EVERY cell, shrink the entire original effect about its cell center to 76% of the current size, leaving a generous completely empty 10-12% margin along all four cell edges. ALL wisps and ALL glow must fit inside this margin; no content may touch a cell boundary. Keep every cell the same scale relative to the other frames. Remove every straight grid-edge artifact. Background must be uniform PURE BLACK RGB 0 0 0, no gradient, no lines or checkerboard. Preserve the sharp white blade details, do not blur them. Keep the 16-frame build-up/peak/breakup/fade chronology exactly. The last frame stays almost empty. Output square PNG, preferably 2048x2048. No labels, numbers, borders or text.
```
