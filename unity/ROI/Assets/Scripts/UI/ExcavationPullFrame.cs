using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
 // Samples only the supplied ornamental skin; the title and price are live localized UI.
 public sealed class ExcavationPullFrame : RawImage {
  public bool Ten;
  Material _eclipseMaterial;
  protected override void Awake() {
   base.Awake();
   var shader=Resources.Load<Shader>("ExcavationUI/EclipsePull");
   if(shader!=null)material=_eclipseMaterial=new Material(shader){hideFlags=HideFlags.HideAndDontSave};
  }
  protected override void OnDestroy() {
   base.OnDestroy();
   if(_eclipseMaterial!=null)Destroy(_eclipseMaterial);
  }
  protected override void OnPopulateMesh(VertexHelper vh) {
   vh.Clear();
   Quad(vh,0,0,429,39,0,0,429,39);
   Quad(vh,0,39,132,66,0,39,132,66);
   Quad(vh,324,39,105,66,324,39,105,66);
   Quad(vh,0,105,429,38,0,105,429,38);
   if(Ten) { Quad(vh,132,39,192,41,310.5f,39,0,41); Quad(vh,132,80,192,25,310.5f,80,0,0); } else Quad(vh,132,39,192,66,132.5f,39,0,66);
  }
  void Quad(VertexHelper vh,float x,float y,float w,float h,float sx,float sy,float sw,float sh) {
   int start=vh.currentVertCount;var r=rectTransform.rect;float offset=Ten?430:0;
   Vector2[] p={new Vector2(x,y),new Vector2(x+w,y),new Vector2(x+w,y+h),new Vector2(x,y+h)};
   Vector2[] uv={new Vector2(sx,sy),new Vector2(sx+sw,sy),new Vector2(sx+sw,sy+sh),new Vector2(sx,sy+sh)};
   for(int i=0;i<4;i++)vh.AddVert(new Vector2(r.xMin+p[i].x/429*r.width,r.yMax-p[i].y/143*r.height),color,new Vector2((offset+uv[i].x)/859,1-uv[i].y/143));
   vh.AddTriangle(start,start+1,start+2);vh.AddTriangle(start,start+2,start+3);
  }
 }
}
