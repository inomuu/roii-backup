using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Keep only the supplied frame and ornaments; the center contains live UI.
    public sealed class ExcavationDialogFrame : RawImage
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            Quad(vh,0,0,756,156,0,0,756,156,false);
            Quad(vh,0,156,100,159,0,156,100,159,false);
            Quad(vh,656,156,100,159,656,156,100,159,false);
            Quad(vh,0,315,756,45,0,315,756,45,false);
            Quad(vh,100,156,556,64,200.5f,156,0,64,false);
            Quad(vh,100,220,556,95,100.5f,220,0,95,false);
        }
        void Quad(VertexHelper vh,float x,float y,float w,float h,float sx,float sy,float sw,float sh,bool fill)
        {
            int start=vh.currentVertCount;var r=rectTransform.rect;
            Vector2[] p={new Vector2(x,y),new Vector2(x+w,y),new Vector2(x+w,y+h),new Vector2(x,y+h)};
            Vector2[] uv={new Vector2(sx,sy),new Vector2(sx+sw,sy),new Vector2(sx+sw,sy+sh),new Vector2(sx,sy+sh)};
            for(int i=0;i<4;i++) {
                var tint=color;if(fill)tint*=i<2?new Color(.91f,.91f,.94f):new Color(.54f,.55f,.63f);
                vh.AddVert(new Vector2(r.xMin+p[i].x/756*r.width,r.yMax-p[i].y/360*r.height),tint,
                    new Vector2(uv[i].x/756,1-uv[i].y/360));
            }
            vh.AddTriangle(start,start+1,start+2);vh.AddTriangle(start,start+2,start+3);
        }
    }
}
