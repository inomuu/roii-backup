using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
 public sealed class ExcavationButtonFrame : RawImage {
  protected override void OnPopulateMesh(VertexHelper vh) {
   vh.Clear();
   var left=new List<Vector2>{new Vector2(12,0),new Vector2(40,0),new Vector2(40,64),new Vector2(12,64)};
   Arc(left,new Vector2(12,52),90,180);Arc(left,new Vector2(12,12),180,270);
   Region(vh,left,0,40,1186,64);
   Region(vh,new List<Vector2>{new Vector2(40,0),new Vector2(208,0),new Vector2(208,64),new Vector2(40,64)},40,208,1262,8);
   var right=new List<Vector2>();foreach(var p in left)right.Add(new Vector2(248-p.x,p.y));right.Reverse();Region(vh,right,208,248,1515,64);
  }
  static void Arc(List<Vector2> path,Vector2 center,float start,float end) {
   for(int i=1;i<=16;i++){float a=Mathf.Lerp(start,end,i/16f)*Mathf.Deg2Rad;path.Add(center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*12);}
  }
  void Region(VertexHelper vh,List<Vector2> path,float min,float max,float sourceX,float sourceWidth) {
   int start=vh.currentVertCount;Add(vh,new Vector2((min+max)*.5f,32),min,max,sourceX,sourceWidth);
   foreach(var p in path)Add(vh,p,min,max,sourceX,sourceWidth);
   for(int i=0;i<path.Count;i++)vh.AddTriangle(start,start+i+1,start+(i+1)%path.Count+1);
  }
  void Add(VertexHelper vh,Vector2 p,float min,float max,float sourceX,float sourceWidth) {
   var r=rectTransform.rect;var position=new Vector2(r.xMin+p.x/248*r.width,r.yMax-p.y/64*r.height);
   var uv=new Vector2((sourceX+(p.x-min)/(max-min)*sourceWidth)/2048f,(1149-1004-p.y/64*103)/1149f);
   vh.AddVert(position,color,uv);
  }
 }
}
