using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Codex vector frames: selected card, quiet secondary action, gold upgrade action.
    // Static retained geometry; Button owns hover/press/disabled tinting.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class AlbumCodexSurface : MaskableGraphic
    {
        public bool Selected, Primary, DoubleFrame, Insignia, Ribbon;

        public static AlbumCodexSurface Create(Transform parent, string name, bool doubleFrame = false)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var surface = go.AddComponent<AlbumCodexSurface>();
            surface.DoubleFrame = doubleFrame;
            surface.raycastTarget = false;
            return surface;
        }

        protected override void OnPopulateMesh(VertexHelper vh) =>
            CodexFrameMesh.Draw(vh, rectTransform.rect, color, Selected, Primary, DoubleFrame, Insignia, Ribbon);
    }
}
