using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Resolution-independent, chamfered menu surfaces. No image frames or per-frame work.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class GameMenuSurface : MaskableGraphic
    {
        public bool Primary;
        public bool Opaque;
        public static GameMenuSurface Add(Transform parent,bool primary=false)
        {
            var go=new GameObject("CutGlass",typeof(RectTransform));go.transform.SetParent(parent,false);
            go.transform.SetAsFirstSibling();var g=go.AddComponent<GameMenuSurface>();g.Primary=primary;g.raycastTarget=false;
            var r=g.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;return g;
        }
        protected override void OnPopulateMesh(VertexHelper vh) =>
            CodexFrameMesh.Draw(vh, rectTransform.rect, color, Primary:Primary,
                DoubleFrame:rectTransform.rect.height>100, Opaque:Opaque);
    }
}
