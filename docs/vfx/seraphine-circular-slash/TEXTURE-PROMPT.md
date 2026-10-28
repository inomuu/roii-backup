# Static Slash Mask v1

Generated with the built-in image_gen tool, not the CLI/API fallback.
Asset: `unity/ROI/Assets/Resources/Effects/SeraphineSlashMask-v1.png`.
The tool returned a 1254 x 1254 opaque PNG; Unity imports it at 1024 with linear sampling,
clamped UVs, mipmaps and no compression. The shader uses red-channel coverage, not alpha.
No post-generation painting or image editing was applied.

## Final Prompt

Use case: stylized-concept. Asset type: production grayscale VFX opacity mask for a UV-unwrapped sword-slash ribbon mesh in a game, single static texture, NOT a flipbook. Generate a square 1024x1024 image on an absolutely pure black opaque background. Inside the central horizontal band, draw THREE separated elongated white blade-like energy streak silhouettes flowing horizontally from left to right. Each streak starts in a very fine long needle tail on the left, widens asymmetrically toward the right, then ends in a razor-sharp hooked spear point. They are gently undulating and elongated horizontally, NOT circular crescents or rings. One dominant broader streak and two thinner shorter companion streaks offset above and below, with clearly visible black negative space between them. Add a few long sharp black slit cutouts toward the tails, like hand-painted stylized anime ice/water slash VFX. Solid white interiors, crisply antialiased borders only. Overall shapes span x=8% to 92%, y=18% to 82%; keep the entire outer margin pure black. This is an UNLIT flat technical texture mask, the game shader supplies all color, HDR light, bloom and mesh curvature. No gradients, blur, soft glow, lighting, shadows, noise, spark dots, text, watermark, frame, grid, character, weapon, perspective or 3D rendering.

## Use

This is a flat UV mask, not a circular sprite or flipbook. Three streaks share one curved
ribbon. Geometry supplies the sweep; the grayscale mask supplies sharp cutouts. The existing
water-flow texture perturbs a directional erosion threshold as the strike ends. The approved
solid primary blade stays independent, with its original width coefficient 0.16.
