using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // One continuous seam bows at the selected category, with a soft inner reflection.
    // Static UI geometry: rebuilt only when selection or layout changes.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ShopSelectionArc : MaskableGraphic
    {
        static readonly float[] Offsets = { -6f, -3f, -1.5f, -.8f, -.35f, 0f, .35f, .8f, 1.5f, 3f, 6f };
        static readonly float[] Alpha = { 0f, .025f, .09f, .3f, .78f, 1f, .78f, .3f, .09f, .025f, 0f };
        float _selectedOffset = 26f;
        float _normalizedOffset = -1f;

        public void SelectNormalized(float offsetFromTop)
        {
            offsetFromTop = Mathf.Clamp01(offsetFromTop);
            if (Mathf.Approximately(_normalizedOffset, offsetFromTop)) return;
            _normalizedOffset = offsetFromTop;
            SetVerticesDirty();
        }

        public void Select(int index)
        {
            float offset = index * 64f + 26f;
            _normalizedOffset = -1f;
            if (Mathf.Approximately(offset, _selectedOffset)) return;
            _selectedOffset = offset;
            SetVerticesDirty();
        }

        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var r = rectTransform.rect;
            if (r.height <= 0f) return;
            float center = r.yMax - (_normalizedOffset >= 0 ? r.height * _normalizedOffset : _selectedOffset);
            float seamX = r.xMax - 17f;
            DrawGlow(vh, r, seamX, center);
            DrawSeam(vh, r, seamX, center, true);
            DrawSeam(vh, r, seamX, center, false);
        }

        static float Bend(float d)
        {
            // The upper shoulder is longer; the lower end turns back sooner.
            float u = d / (d < 0f ? 29f : 19f);
            return Mathf.Exp(-u * u);
        }

        void DrawSeam(VertexHelper vh, Rect r, float x, float center, bool reflection)
        {
            int rows = Mathf.CeilToInt(r.height / 1.5f);
            int start = vh.currentVertCount;
            for (int i = 0; i <= rows; i++)
            {
                float y = Mathf.Lerp(r.yMax, r.yMin, i / (float)rows);
                float d = center - y;
                float bend = Bend(d);
                float offset = reflection ? 3.2f : 8.5f;
                float derivative = offset * (Bend(d + .1f) - Bend(d - .1f)) / .2f;
                var normal = new Vector2(1f, derivative).normalized;
                float edgeFade = Mathf.Clamp01(Mathf.Min(r.yMax - y, y - r.yMin) / 18f);
                float strength = reflection ? .25f * bend : .065f + .935f * Mathf.Pow(bend, .65f);
                var tint = reflection ? new Color(.89f, .84f, .67f, 1f)
                    : Color.Lerp(new Color(.55f, .59f, .63f, 1f), new Color(1f, .97f, .83f, 1f), Mathf.Pow(bend, .45f));
                float width = reflection ? 1.2f : Mathf.Lerp(.38f, .9f, bend);
                for (int j = 0; j < Offsets.Length; j++)
                {
                    var c = tint;
                    c.a = Alpha[j] * strength * edgeFade * color.a;
                    vh.AddVert(new Vector2(x + offset * bend, y) + normal * Offsets[j] * width, c, Vector2.zero);
                    if (i == 0 || j == 0) continue;
                    int n = start + i * Offsets.Length + j;
                    vh.AddTriangle(n - Offsets.Length - 1, n - Offsets.Length, n);
                    vh.AddTriangle(n - Offsets.Length - 1, n, n - 1);
                }
            }
        }

        void DrawGlow(VertexHelper vh, Rect r, float x, float center)
        {
            const int cols = 20, rows = 56;
            int start = vh.currentVertCount;
            float top = Mathf.Min(r.yMax, center + 78f), bottom = Mathf.Max(r.yMin, center - 70f);
            for (int i = 0; i <= rows; i++)
            {
                float y = Mathf.Lerp(top, bottom, i / (float)rows);
                float d = center - y;
                float vertical = Mathf.Exp(-Mathf.Pow(d / 32f, 2f));
                for (int j = 0; j <= cols; j++)
                {
                    float px = x - 28f + j * 48f / cols;
                    float spread = Mathf.Exp(-Mathf.Pow((px - x - 3f) / 11f, 2f));
                    float edge = Mathf.Sin(j * Mathf.PI / cols) * Mathf.Sin(i * Mathf.PI / rows);
                    var tint = new Color(.84f, .84f, .75f, .15f * vertical * spread * Mathf.Max(0f, edge) * color.a);
                    vh.AddVert(new Vector2(px, y), tint, Vector2.zero);
                    if (i == 0 || j == 0) continue;
                    int n = start + i * (cols + 1) + j;
                    vh.AddTriangle(n - cols - 2, n - cols - 1, n);
                    vh.AddTriangle(n - cols - 2, n, n - 1);
                }
            }
        }
    }
}
