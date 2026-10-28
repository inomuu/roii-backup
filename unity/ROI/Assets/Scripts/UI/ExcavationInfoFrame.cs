using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
 // Retained vector frame: smooth corners and fine strokes at every canvas scale.
 [RequireComponent(typeof(CanvasRenderer))]
 public sealed class ExcavationInfoFrame : MaskableGraphic {
  protected override void OnPopulateMesh(VertexHelper vh) {
   vh.Clear();var r=rectTransform.rect;const int steps=16;const float radius=22;
   var points=new Vector2[4*(steps+1)];int n=0;
   for(int corner=0;corner<4;corner++) {
    var center=new Vector2(corner==0||corner==3?r.xMax-radius:r.xMin+radius,corner<2?r.yMax-radius:r.yMin+radius);
    for(int j=0;j<=steps;j++){float a=(corner*90+j*90f/steps)*Mathf.Deg2Rad;points[n++]=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius;}
   }
   Color Fill(Vector2 p)=>Color.Lerp(new Color(.026f,.040f,.071f,1),new Color(.13f,.17f,.23f,1),Mathf.Clamp01((p.y-r.yMin)/r.height)*Mathf.Clamp01(1-(p.x-r.xMin)/r.width));
   vh.AddVert(r.center,Fill(r.center),Vector2.zero);
   foreach(var p in points)vh.AddVert(p,Fill(p),Vector2.zero);
   for(int i=0;i<n;i++)vh.AddTriangle(0,1+i,1+(i+1)%n);
   int start=vh.currentVertCount;
   for(int i=0;i<n;i++) {
    var p=points[i];var prev=points[(i+n-1)%n];var next=points[(i+1)%n];var tangent=(next-prev).normalized;var inward=new Vector2(-tangent.y,tangent.x);
    vh.AddVert(p-inward*.5f,new Color(.80f,.75f,.59f,0),Vector2.zero);
    vh.AddVert(p+inward*.4f,new Color(.80f,.75f,.59f,.9f),Vector2.zero);
    vh.AddVert(p+inward*1.4f,new Color(.80f,.75f,.59f,0),Vector2.zero);
   }
   for(int i=0;i<n;i++)for(int band=0;band<2;band++){int a=start+i*3+band,b=start+((i+1)%n)*3+band;vh.AddTriangle(a,b,a+1);vh.AddTriangle(a+1,b,b+1);}
  }
 }
}
