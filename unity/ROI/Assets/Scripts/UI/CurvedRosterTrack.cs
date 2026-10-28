using UnityEngine;
using UnityEngine.UI;
using UnityEngine.EventSystems;

namespace Roi.UI
{
    // A retained 180-degree roster arc. The hero-facing edge is a true semicircle.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class CurvedRosterTrack : MaskableGraphic
    {
        // A half-circle needs one radius of width, not a full diameter. This shared geometry
        // keeps the portrait centers on the visible track at both reference resolutions.
        public static void Geometry(Rect rect,out Vector2 center,out float radius)
        {
            center=new Vector2(rect.xMin+24f,rect.center.y+25f);
            radius=Mathf.Max(28f,Mathf.Min(rect.width-78f,rect.height*.5f-80f));
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var r=GetPixelAdjustedRect();
            const int segments=96;
            Geometry(r,out var center,out float radius);
            var fill=new Color(.045f,.065f,.07f,.36f)*color;
            var rim=new Color(.78f,.70f,.48f,.34f)*color;
            float band=3f;
            for(int i=0;i<segments;i++)
            {
                float a0=-Mathf.PI*.5f+i*Mathf.PI/segments;
                float a1=-Mathf.PI*.5f+(i+1)*Mathf.PI/segments;
                var a=center+new Vector2(Mathf.Cos(a0),Mathf.Sin(a0))*(radius-band);
                var b=center+new Vector2(Mathf.Cos(a0),Mathf.Sin(a0))*(radius+band);
                var c=center+new Vector2(Mathf.Cos(a1),Mathf.Sin(a1))*(radius+band);
                var d=center+new Vector2(Mathf.Cos(a1),Mathf.Sin(a1))*(radius-band);
                int n=vh.currentVertCount;
                vh.AddVert(a,fill,Vector2.zero);vh.AddVert(b,fill,Vector2.zero);
                vh.AddVert(c,fill,Vector2.zero);vh.AddVert(d,fill,Vector2.zero);
                vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
                Line(vh,a,d,.8f,rim);Line(vh,b,c,.8f,rim);
            }
            // Fine end caps define the open side of the half-circle.
            Line(vh,center+Vector2.down*(radius-band),center+Vector2.down*(radius+band),1f,rim);
            Line(vh,center+Vector2.up*(radius-band),center+Vector2.up*(radius+band),1f,rim);
        }
        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color ink)
        {
            var d=(b-a).normalized;var n=new Vector2(-d.y,d.x)*width*.5f;int i=vh.currentVertCount;
            vh.AddVert(a-n,ink,Vector2.zero);vh.AddVert(a+n,ink,Vector2.zero);
            vh.AddVert(b+n,ink,Vector2.zero);vh.AddVert(b-n,ink,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }

    // Stencil alpha for clean circular portrait crops, independent of sprite dimensions.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class RosterPortraitMaskGraphic : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();var center=r.center;
            float radius=Mathf.Min(r.width,r.height)*.5f;
            const int segments=40;
            vh.AddVert(center,color,Vector2.zero);
            for(int i=0;i<segments;i++)
            {
                float a=(i/(float)segments)*Mathf.PI*2;
                vh.AddVert(center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius,color,Vector2.zero);
            }
            for(int i=0;i<segments;i++)vh.AddTriangle(0,i+1,(i+1)%segments+1);
        }
    }

    // Restrained circle stroke, gold for the selected portrait and slate for other members.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class RosterMedallionRing : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=GetPixelAdjustedRect();var center=r.center;
            float radius=Mathf.Min(r.width,r.height)*.5f-1.5f;
            const int segments=48;float inner=radius-1.35f;
            for(int i=0;i<segments;i++)
            {
                float a=i/(float)segments*Mathf.PI*2,b=(i+1)/(float)segments*Mathf.PI*2;
                var outerA=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius;
                var innerA=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*inner;
                var outerB=center+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*radius;
                var innerB=center+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*inner;
                int n=vh.currentVertCount;
                vh.AddVert(innerA,color,Vector2.zero);vh.AddVert(outerA,color,Vector2.zero);
                vh.AddVert(outerB,color,Vector2.zero);vh.AddVert(innerB,color,Vector2.zero);
                vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
            }
        }
    }

    // One step per wheel gesture or completed swipe; only the visible window changes.
    // Selecting a portrait remains an ordinary native Button action.
    public sealed class RosterArcInput : MonoBehaviour,IScrollHandler,IBeginDragHandler,IDragHandler,IEndDragHandler
    {
        public System.Action<int> Move;
        Vector2 _start;
        float _lastWheel;
        public void OnScroll(PointerEventData e)
        {
            if(!isActiveAndEnabled||Mathf.Abs(e.scrollDelta.y)<.01f||Time.unscaledTime-_lastWheel<.12f)return;
            _lastWheel=Time.unscaledTime;Move?.Invoke(e.scrollDelta.y<0?1:-1);
        }
        public void OnBeginDrag(PointerEventData e){_start=e.position;}
        public void OnDrag(PointerEventData e){}
        public void OnEndDrag(PointerEventData e)
        {
            if(!isActiveAndEnabled)return;
            float delta=e.position.y-_start.y;
            if(Mathf.Abs(delta)>24f)Move?.Invoke(delta>0?1:-1);
        }
    }
}
