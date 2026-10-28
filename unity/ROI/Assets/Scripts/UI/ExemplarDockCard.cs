using UnityEngine;
using UnityEngine.UI;
using UnityEngine.EventSystems;

namespace Roi.UI
{
    // Retained portrait dock artwork. Missing art is a neutral silhouette, never another unit.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarDockCard : MaskableGraphic
    {
        bool _selected;
        public void Select(bool selected)
        {
            if (_selected == selected) return;
            _selected = selected; SetVerticesDirty();
        }
        public static ExemplarDockCard Create(RectTransform parent, string unit)
        {
            var art = ExemplarUi.Rect(parent,"DockArtwork",0,0,1,1);
            var mask = art.gameObject.AddComponent<ExemplarDockMask>();mask.raycastTarget=false;
            art.gameObject.AddComponent<Mask>().showMaskGraphic=true;
            var faceRect=ExemplarUi.Rect(art,"Face",.02f,.02f,.98f,.98f);
            var texture=ExemplarPortrait.Illustration(unit);
            if(texture!=null)
            {
                var face=faceRect.gameObject.AddComponent<CombatHudPortrait>();face.raycastTarget=false;
                face.Crop=new Rect(.35f,.765f,.32f,.18f);face.PreserveAspect=true;
                face.SourceAspect=unit=="seraphine"?5151f/5816f:0;
                face.SideFeather=.01f;face.BottomFeather=.03f;face.SetTexture(texture);
            }
            else
            {
                var sprite=ExemplarPortrait.Avatar(unit);
                if(sprite!=null){var face=faceRect.gameObject.AddComponent<Image>();face.sprite=sprite;face.preserveAspect=true;face.raycastTarget=false;}
                else {var face=faceRect.gameObject.AddComponent<ExemplarDockSilhouette>();face.raycastTarget=false;}
            }
            var shade=ExemplarUi.Rect(art,"CaptionShade",0,0,1,.44f).gameObject.AddComponent<ExemplarDockShade>();shade.raycastTarget=false;
            var frame=ExemplarUi.Rect(parent,"DockFrame",0,0,1,1).gameObject.AddComponent<ExemplarDockCard>();frame.raycastTarget=false;
            return frame;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();r.xMin+=2;r.yMin+=2;r.xMax-=2;r.yMax-=2;
            var ink=(_selected?UiTheme.Accent:new Color(.47f,.55f,.57f,.65f))*color;
            var p=ExemplarDockMask.Corners(r,7);
            for(int i=0;i<p.Length;i++)Line(vh,p[i],p[(i+1)%p.Length],_selected?1.6f:1,ink);
            if(!_selected)return;
            var center=new Vector2(r.xMin+15,r.yMax-15);
            var diamond=new[]{center+Vector2.up*6,center+Vector2.right*6,center+Vector2.down*6,center+Vector2.left*6};
            for(int i=0;i<4;i++)Line(vh,diamond[i],diamond[(i+1)%4],1.4f,ink);
            Line(vh,center-Vector2.up*2,center+Vector2.up*2,2,ink);
        }
        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color ink)
        {
            int i=vh.currentVertCount;var d=(b-a).normalized;var n=new Vector2(-d.y,d.x)*width*.5f;
            vh.AddVert(a-n,ink,Vector2.zero);vh.AddVert(a+n,ink,Vector2.zero);vh.AddVert(b+n,ink,Vector2.zero);vh.AddVert(b-n,ink,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarDockMask : MaskableGraphic
    {
        internal static Vector2[] Corners(Rect r,float c)=>new[]{
            new Vector2(r.xMin+c,r.yMin),new Vector2(r.xMin,r.yMin+c),new Vector2(r.xMin,r.yMax-c),new Vector2(r.xMin+c,r.yMax),
            new Vector2(r.xMax-c,r.yMax),new Vector2(r.xMax,r.yMax-c),new Vector2(r.xMax,r.yMin+c),new Vector2(r.xMax-c,r.yMin)};
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();r.xMin+=3;r.xMax-=3;r.yMin+=3;r.yMax-=3;
            var ink=new Color(.065f,.085f,.10f,.97f);vh.AddVert(r.center,ink,Vector2.zero);
            foreach(var p in Corners(r,6))vh.AddVert(p,ink,Vector2.zero);
            for(int i=0;i<8;i++)vh.AddTriangle(0,i+1,(i+1)%8+1);
        }
    }
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarDockShade : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();var ink=new Color(.018f,.027f,.033f,.96f);
            vh.AddVert(new Vector2(r.xMin,r.yMin),ink,Vector2.zero);vh.AddVert(new Vector2(r.xMax,r.yMin),ink,Vector2.zero);
            ink.a=0;vh.AddVert(new Vector2(r.xMax,r.yMax),ink,Vector2.zero);vh.AddVert(new Vector2(r.xMin,r.yMax),ink,Vector2.zero);
            vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);
        }
    }
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarDockSilhouette : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();var ink=new Color(.20f,.24f,.29f,1);
            // A neutral hooded bust belongs to no named unit; actual artwork replaces it automatically.
            var hood=new[]{new Vector2(.50f,.98f),new Vector2(.35f,.83f),new Vector2(.30f,.47f),new Vector2(.20f,.25f),new Vector2(.80f,.25f),new Vector2(.70f,.47f),new Vector2(.65f,.83f)};
            int start=vh.currentVertCount;vh.AddVert(r.min+Vector2.Scale(new Vector2(.5f,.55f),r.size),ink,Vector2.zero);
            foreach(var point in hood)vh.AddVert(r.min+Vector2.Scale(point,r.size),ink,Vector2.zero);
            for(int i=0;i<hood.Length;i++)vh.AddTriangle(start,start+i+1,start+(i+1)%hood.Length+1);
            Oval(vh,r,new Vector2(.5f,.64f),new Vector2(.17f,.31f),ink);
            Oval(vh,r,new Vector2(.5f,.03f),new Vector2(.38f,.36f),ink);
            Oval(vh,r,new Vector2(.5f,.34f),new Vector2(.085f,.20f),ink);
            Oval(vh,r,new Vector2(.5f,.66f),new Vector2(.105f,.21f),new Color(.12f,.15f,.19f,1));
        }
        static void Oval(VertexHelper vh,Rect r,Vector2 center,Vector2 radius,Color ink)
        {
            int first=vh.currentVertCount;Vector2 c=r.min+Vector2.Scale(center,r.size);vh.AddVert(c,ink,Vector2.zero);
            for(int i=0;i<32;i++){float a=i*Mathf.PI*2/32;vh.AddVert(c+Vector2.Scale(new Vector2(Mathf.Cos(a),Mathf.Sin(a)),Vector2.Scale(radius,r.size)),ink,Vector2.zero);}
            for(int i=0;i<32;i++)vh.AddTriangle(first,first+1+i,first+1+(i+1)%32);
        }
    }
    public sealed class ExemplarDockWheel : MonoBehaviour, IScrollHandler
    {
        public System.Action<float> Scroll;
        public void OnScroll(PointerEventData e)
        {
            if(UiConfirm.BlocksNavigation)return;
            var delta=e.scrollDelta;
            Scroll?.Invoke(-(Mathf.Abs(delta.x)>Mathf.Abs(delta.y)?delta.x:delta.y));
        }
    }
}
