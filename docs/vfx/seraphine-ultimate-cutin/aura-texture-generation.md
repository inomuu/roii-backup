# Aura Texture Generation

Generated using the built-in `image_gen` tool. No API/CLI fallback was used.
The character illustration is unchanged.

## Asset

- Runtime source: `unity/ROI/Assets/Resources/Cutin/SeraphineAuraFlow-v1.png`
- Output: 1254x1254 RGB, copied unchanged from the generated output.
- Unity: imported at up to 1024px, linear intensity (sRGB off), mipmaps,
  trilinear filtering, mirrored wrapping, no alpha and no texture compression.
- The requested seamless tiling is not assumed perfect: opposite-edge mean
  grayscale differences are 8.03 horizontally and 9.04 vertically (0-255).
  Mirrored wrapping avoids value jumps without modifying the generated bitmap.
  Two overlapping, warped Cartesian samples reduce obvious reflected repetition
  and avoid the seam from polar UV coordinates.
- Shader: the mask modulates wave density, crest intensity and contour shape.
  Cyan coloring and animation remain shader-controlled. Neon lines are independent.
- `_AuraDetail = 0` restores the procedural-only waves for A/B inspection.

## Exact Prompt

```text
Use case: stylized-concept.
Asset type: seamless square grayscale VFX emission mask texture for a real-time Unity water-aura shader, not a finished scene.
Create one high-quality 1024x1024 flat texture: broad translucent flowing sheets of liquid light, irregular rippling water folds, wispy torn trailing edges and soft luminous crests. Predominantly horizontal flow with gently curling eddies and layered thick-to-thin variation. Painterly fantasy water energy, organic and fluid, with a few crisp crests but mostly broad soft masses. Pure grayscale, white/gray flowing material over pure black negative space; black covers about half the image. Distributed pattern across the entire square, no central focal point, seamless tileable left/right and top/bottom edges. Moderate contrast with rich midtones, no clipped white areas. This is a raw material intensity mask which will be tinted cyan and animated through UV distortion in-engine. No character, no ring, no shield, no objects, no landscape, no perspective, no border, no text, no watermark, no thin neon lines, no sparks, no starfield, no checkerboard, no baked blue color.
```
