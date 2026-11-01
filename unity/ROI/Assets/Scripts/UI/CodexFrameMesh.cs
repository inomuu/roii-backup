using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI
{
    // Shared retained geometry in the WuWa idiom: a dark glass slab with its top-left and
    // bottom-right corners cut, a hairline edge, and gold only where something is chosen.
    // A PRIMARY action is the ivory slab with dark text. No texture sampling or per-frame work.
    internal static class CodexFrameMesh
    {
        internal static void Draw(VertexHelper vh, Rect r, Color color, bool Selected=false,
            bool Primary=false, bool DoubleFrame=false, bool Insignia=false, bool Ribbon=false, bool Opaque=false, bool Raised=false)
        {
            vh.Clear();
            if (Ribbon)
            {
                vh.AddVert(new Vector2(r.xMin,r.yMin),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMin,r.yMax),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMax,r.yMax),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMax-r.height,r.yMin),UiTheme.Accent*color,Vector2.zero);
                vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);return;
            }
            r.xMin += 1; r.xMax -= 1; r.yMin += 1; r.yMax -= 1;
            if (r.width <= 0 || r.height <= 0) return;
            float cut = Mathf.Clamp(r.height * .22f, 4f, DoubleFrame ? 16f : 10f);
            var p = WuwaMesh.Cut(r, cut);

            Color top, bottom;
            if (Primary) { top = UiTheme.Ivory; bottom = UiTheme.IvoryShade; }
            else if (Selected) { top = UiTheme.FrameSelected; bottom = UiTheme.FrameBottom; }
            else if (Raised) { top = UiTheme.SurfaceHover; bottom = UiTheme.Surface; }
            else { top = UiTheme.FrameTop; bottom = UiTheme.FrameBottom; }
            if (Opaque) { top.a = 1; bottom.a = 1; }
            WuwaMesh.Fill(vh, p, r, bottom * color, top * color);

            if (Primary)
            {
                // A thin inner rule a few pixels in: WuWa's ivory buttons read as a plate in a frame.
                var inner = new Rect(r.xMin + 3, r.yMin + 3, r.width - 6, r.height - 6);
                if (inner.width > 8 && inner.height > 8)
                    WuwaMesh.Outline(vh, WuwaMesh.Cut(inner, cut * .7f), 1f, new Color(.25f,.22f,.16f,.35f) * color);
                WuwaMesh.Outline(vh, p, 1f, new Color(1f,.97f,.88f,.9f) * color);
                return;
            }

            var edge = Selected ? UiTheme.Accent : Raised ? UiTheme.FrameRaisedEdge : UiTheme.FrameEdge;
            WuwaMesh.Outline(vh, p, Selected ? 1.4f : 1f, edge * color);
            // Top highlight: one pixel of light along the upper edge makes glass of a flat slab.
            WuwaMesh.Line(vh, new Vector2(r.xMin + cut + 2, r.yMax - 1.5f), new Vector2(r.xMax - 2, r.yMax - 1.5f), 1f,
                new Color(1f, 1f, 1f, Selected ? .16f : .07f) * color);
            if (Selected)
            {
                // The WuWa selection tick: a short gold bar on the left edge and a soft warm wash.
                float h = Mathf.Min(r.height - cut * 2, 28f);
                WuwaMesh.Rect(vh, new Rect(r.xMin + 1, r.center.y - h * .5f, 3, h), UiTheme.Accent * color);
                WuwaMesh.Quad(vh, new Vector2(r.xMin, r.yMin), new Vector2(r.xMin, r.yMax - cut), new Vector2(r.xMin + r.width * .45f, r.yMax - cut), new Vector2(r.xMin + r.width * .45f, r.yMin),
                    UiTheme.WithAlpha(UiTheme.AccentGlow, .18f) * color, UiTheme.WithAlpha(UiTheme.AccentGlow, .18f) * color, Color.clear, Color.clear);
            }
            if (DoubleFrame)
                WuwaMesh.Brackets(vh, r, 9, 1.2f, UiTheme.WithAlpha(UiTheme.Accent, Selected ? .8f : .35f) * color, 4);
            if (Selected && Insignia)
                WuwaMesh.Diamond(vh, new Vector2(r.xMax - 16, r.yMax - 16), 4f, UiTheme.Accent * color);
        }
    }
}
