using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained decorative mesh. No Update, animation, camera, render target or post effect.
    // WuWa page ground: charcoal lit from the upper left, a faint diagonal light band, and a set
    // of thin concentric "resonance" rings on the right that sit far below the content.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class RpgBackdrop : MaskableGraphic
    {
        public static void Create(Transform parent)
        {
            var go=new GameObject("TacticalBackdrop",typeof(RectTransform));
            go.transform.SetParent(parent,false);
            var rt=(RectTransform)go.transform;rt.anchorMin=Vector2.zero;rt.anchorMax=Vector2.one;rt.offsetMin=rt.offsetMax=Vector2.zero;
            var overlay=new GameObject("Atmosphere",typeof(RectTransform));overlay.transform.SetParent(go.transform,false);
            var or=(RectTransform)overlay.transform;or.anchorMin=Vector2.zero;or.anchorMax=Vector2.one;or.offsetMin=or.offsetMax=Vector2.zero;
            overlay.AddComponent<RpgBackdrop>().raycastTarget=false;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            if(r.width<=0||r.height<=0)return;
            var lit=UiTheme.BackdropTop;var deep=UiTheme.BackdropBottom;var mid=Color.Lerp(lit,deep,.55f);
            // bottom-left, top-left, top-right, bottom-right
            WuwaMesh.Quad(vh,new Vector2(r.xMin,r.yMin),new Vector2(r.xMin,r.yMax),new Vector2(r.xMax,r.yMax),new Vector2(r.xMax,r.yMin),mid,lit,mid,deep);

            // A soft diagonal band of light.
            var glow=new Color(1f,.96f,.86f,.028f);
            float w=r.width*.18f;
            WuwaMesh.Quad(vh,new Vector2(r.xMin+r.width*.20f,r.yMin),new Vector2(r.xMin+r.width*.48f,r.yMax),
                new Vector2(r.xMin+r.width*.48f+w,r.yMax),new Vector2(r.xMin+r.width*.20f+w,r.yMin),Color.clear,glow,Color.clear,Color.clear);

            // Resonance rings, low on the right.
            var centre=new Vector2(r.xMax-r.width*.16f,r.yMin+r.height*.42f);
            float baseR=r.height*.30f;
            var ring=new Color(.93f,.86f,.66f,.045f);
            WuwaMesh.Ring(vh,centre,baseR,1f,ring,96);
            WuwaMesh.Ring(vh,centre,baseR*1.32f,1f,UiTheme.WithAlpha(ring,.03f),96);
            WuwaMesh.Ring(vh,centre,baseR*2.0f,1f,UiTheme.WithAlpha(ring,.022f),128);
            for(int i=0;i<48;i++)
            {
                float a=i*Mathf.PI*2/48;var d=new Vector2(Mathf.Cos(a),Mathf.Sin(a));
                float len=i%4==0?10:4;
                WuwaMesh.Line(vh,centre+d*(baseR*1.32f-len),centre+d*baseR*1.32f,1f,UiTheme.WithAlpha(ring,i%4==0?.06f:.035f));
            }
            WuwaMesh.DiamondOutline(vh,centre,baseR*.18f,1f,UiTheme.WithAlpha(ring,.06f));

            // Vignette along the foot so the bottom bar sits on darkness.
            var shade=new Color(0,0,0,.35f);
            WuwaMesh.Quad(vh,new Vector2(r.xMin,r.yMin),new Vector2(r.xMin,r.yMin+r.height*.25f),new Vector2(r.xMax,r.yMin+r.height*.25f),new Vector2(r.xMax,r.yMin),shade,Color.clear,Color.clear,shade);
        }
    }
}
