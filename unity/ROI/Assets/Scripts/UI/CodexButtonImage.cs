using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI
{
    // Image compatibility preserves existing selected-state setters and Button tinting.
    public sealed class CodexButtonImage : Image
    {
        public bool Primary, Focused, Raised, NoticeStyle;
        public static CodexButtonImage Create(Transform parent, string name, bool primary=false)
        {
            var go=new GameObject(name,typeof(RectTransform));go.transform.SetParent(parent,false);
            var g=go.AddComponent<CodexButtonImage>();g.Primary=primary;g.color=UiTheme.Surface;return g;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            if(NoticeStyle){base.OnPopulateMesh(vh);return;}
            bool active=color==UiTheme.SurfaceActive || color==new Color(.34f,.30f,.21f);
            CodexFrameMesh.Draw(vh,rectTransform.rect,new Color(1,1,1,color.a),active||Focused,Primary,
                DoubleFrame:rectTransform.rect.height>100,Raised:Raised);
        }
    }
}
