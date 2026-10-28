using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
// Sparse background starlight only. No orbit diagrams or per-frame work.
[RequireComponent(typeof(CanvasRenderer))]
public sealed class CelestialCardMotif : MaskableGraphic {
 public static void Add(Transform parent) {
  var go=new GameObject("BackgroundStarlight",typeof(RectTransform));go.transform.SetParent(parent,false);
  go.transform.SetAsFirstSibling();
  var g=go.AddComponent<CelestialCardMotif>();g.raycastTarget=false;
  var r=g.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;
 }
 protected override void OnPopulateMesh(VertexHelper v){
  v.Clear();var r=rectTransform.rect;
  Vector2[] points={new Vector2(.28f,.25f),new Vector2(.39f,.15f),new Vector2(.47f,.39f),new Vector2(.58f,.22f),new Vector2(.65f,.46f),new Vector2(.35f,.52f),new Vector2(.56f,.58f)};
  for(int i=0;i<points.Length;i++){
   var c=new Vector2(r.xMin+points[i].x*r.width,r.yMin+points[i].y*r.height);
   float size=i%3==0?2.3f:1.1f;
   var ink=new Color(.86f,.88f,.82f,i%3==0?.20f:.12f);
   int n=v.currentVertCount;
   v.AddVert(c+Vector2.up*size,ink,Vector2.zero);v.AddVert(c+Vector2.right*size*.5f,ink,Vector2.zero);
   v.AddVert(c-Vector2.up*size,ink,Vector2.zero);v.AddVert(c-Vector2.right*size*.5f,ink,Vector2.zero);
   v.AddTriangle(n,n+1,n+2);v.AddTriangle(n,n+2,n+3);
  }
 }
}}
