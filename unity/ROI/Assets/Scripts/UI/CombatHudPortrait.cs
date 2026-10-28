using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Display the original transparent illustration, with a feather on the crop rather than a
    // rectangular photo edge. This changes only UI vertices; the source artwork stays intact.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class CombatHudPortrait : MaskableGraphic
    {
        Texture _texture;
        public Rect Crop = new Rect(.29f, .70f, .43f, .27f);
        public bool FeatherAllEdges;
        public bool PreserveAspect;
        // Optional source ratio when a legacy power-of-two import resized the texture unevenly.
        public float SourceAspect;
        public float SideFeather = .07f;
        public float BottomFeather = .09f;
        public override Texture mainTexture => _texture != null ? _texture : Texture2D.whiteTexture;
        public void SetTexture(Texture texture) { _texture = texture; SetAllDirty(); }

        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var rect = GetPixelAdjustedRect();
            if (PreserveAspect && _texture != null && Crop.height > 0)
            {
                float aspect = (SourceAspect>0?SourceAspect:(float)_texture.width/_texture.height)*Crop.width/Crop.height;
                var centre = rect.center;
                if (rect.width > rect.height * aspect) rect.width = rect.height * aspect;
                else rect.height = rect.width / aspect;
                rect.center = centre;
            }
            if (FeatherAllEdges)
            {
                // Equipment illustrations fade into the panel on every side. Retained geometry
                // keeps the centre crisp and needs no render texture or per-frame blur pass.
                const int steps = 24;
                for (int y = 0; y <= steps; y++)
                    for (int x = 0; x <= steps; x++)
                    {
                        float u = x / (float)steps, v = y / (float)steps;
                        var c = color;
                        c.a *= Mathf.SmoothStep(0, 1, Mathf.Min(u, 1-u) / .20f)
                             * Mathf.SmoothStep(0, 1, Mathf.Min(v, 1-v) / .20f);
                        vh.AddVert(new Vector3(rect.x+u*rect.width, rect.y+v*rect.height), c,
                            new Vector2(Crop.x+u*Crop.width, Crop.y+v*Crop.height));
                    }
                for (int y = 0; y < steps; y++)
                    for (int x = 0; x < steps; x++)
                    {
                        int a = y*(steps+1)+x;
                        vh.AddTriangle(a, a+steps+1, a+1);
                        vh.AddTriangle(a+1, a+steps+1, a+steps+2);
                    }
                return;
            }
            float[] xs = { 0f, SideFeather, 1f-SideFeather, 1f };
            float[] ys = { 0f, BottomFeather, 1f };
            for (int y = 0; y < ys.Length; y++)
                for (int x = 0; x < xs.Length; x++)
                {
                    var c = color;
                    if (x == 0 || x == xs.Length - 1 || y == 0) c.a = 0f;
                    vh.AddVert(new Vector3(rect.x + xs[x] * rect.width, rect.y + ys[y] * rect.height),
                               c, new Vector2(Crop.x + xs[x] * Crop.width, Crop.y + ys[y] * Crop.height));
                }
            for (int y = 0; y < ys.Length - 1; y++)
                for (int x = 0; x < xs.Length - 1; x++)
                {
                    int a = y * xs.Length + x;
                    vh.AddTriangle(a, a + xs.Length, a + 1);
                    vh.AddTriangle(a + 1, a + xs.Length, a + xs.Length + 1);
                }
        }
    }
}
