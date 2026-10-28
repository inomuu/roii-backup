using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeDepth : MaskableGraphic
    {
        public static void Create(Transform parent)
        {
            var go=new GameObject("LobbyLighting",typeof(RectTransform));go.transform.SetParent(parent,false);
            var g=go.AddComponent<HomeDepth>();g.raycastTarget=false;
            var r=g.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            Glow(vh,r,new Vector2(.41f,.55f),new Vector2(.30f,.49f),new Color(.52f,.70f,.86f,.22f));
            Glow(vh,r,new Vector2(.30f,.77f),new Vector2(.22f,.28f),new Color(.94f,.79f,.49f,.15f));
            Shade(vh,r,0,.21f,true,new Color(.012f,.026f,.048f,.80f));
            Shade(vh,r,.58f,1,false,new Color(.012f,.026f,.048f,.84f));
        }
        static void Shade(VertexHelper vh,Rect r,float a,float b,bool left,Color ink)
        {
            int i=vh.currentVertCount;var clear=ink;clear.a=0;
            vh.AddVert(new Vector2(r.xMin+a*r.width,r.yMin),left?ink:clear,Vector2.zero);
            vh.AddVert(new Vector2(r.xMin+a*r.width,r.yMax),left?ink:clear,Vector2.zero);
            vh.AddVert(new Vector2(r.xMin+b*r.width,r.yMax),left?clear:ink,Vector2.zero);
            vh.AddVert(new Vector2(r.xMin+b*r.width,r.yMin),left?clear:ink,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
        static void Glow(VertexHelper vh,Rect r,Vector2 at,Vector2 scale,Color ink)
        {
            Vector2 centre=new Vector2(r.xMin+at.x*r.width,r.yMin+at.y*r.height);
            int start=vh.currentVertCount;vh.AddVert(centre,ink,Vector2.zero);var edge=ink;edge.a=0;
            for(int i=0;i<=48;i++) {
                float a=i*Mathf.PI/24;
                vh.AddVert(centre+new Vector2(Mathf.Cos(a)*scale.x*r.width,Mathf.Sin(a)*scale.y*r.height),edge,Vector2.zero);
                if(i>0)vh.AddTriangle(start,start+i,start+i+1);
            }
        }
    }
    public sealed class HomeParallax : MonoBehaviour
    {
        public RectTransform Landscape,Hero;
        Vector2 _offset;float _policyAt;bool _allowed;
        void LateUpdate()
        {
            if(Time.unscaledTime>=_policyAt){_policyAt=Time.unscaledTime+.25f;_allowed=UiEntrance.MotionAllowed;}
            if(Hero==null||Landscape==null)return;
            var entrance=Hero.GetComponent<UiEntrance>();if(entrance!=null&&!entrance.Complete)return;
            Vector2 target=Vector2.zero;
            if(_allowed&&Mouse.current!=null&&Application.isFocused) {
                var pointer=Mouse.current.position.ReadValue();
                target=new Vector2(Mathf.Clamp(pointer.x/Mathf.Max(Screen.width,1)-.5f,-.5f,.5f),Mathf.Clamp(pointer.y/Mathf.Max(Screen.height,1)-.5f,-.5f,.5f));
            }
            var next=_allowed?Vector2.Lerp(_offset,target,1-Mathf.Exp(-Time.unscaledDeltaTime*5)):Vector2.zero;
            if((next-_offset).sqrMagnitude<.000001f&&next==_offset)return;
            _offset=next;Landscape.anchoredPosition=-_offset*8;Hero.anchoredPosition=_offset*14;
        }
        void OnDisable(){_offset=Vector2.zero;if(Landscape!=null)Landscape.anchoredPosition=Vector2.zero;if(Hero!=null)Hero.anchoredPosition=Vector2.zero;}
    }
}
