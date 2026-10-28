using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Small vector icons: no font-glyph dependencies, textures or decorative updates.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class TitleGlyph : MaskableGraphic
    {
        public enum Symbol { Settings, News, Support, Account, Globe, Chevron, Close, Bag, Book, Shop, Star, Mail, Party, Flag }
        public Symbol Kind;
        public bool Ring;
        public static TitleGlyph Add(Transform parent, Symbol kind, bool ring = false)
        {
            var go = new GameObject(kind.ToString(), typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var icon = go.AddComponent<TitleGlyph>();
            icon.Kind = kind; icon.Ring = ring; icon.raycastTarget = false;
            icon.color = UiTheme.TextHigh;
            return icon;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            if (Ring) Arc(vh, Vector2.zero, 29, 29, 0, 360, .8f);
            switch (Kind)
            {
                case Symbol.Bag:
                    Path(vh,new Vector2(-16,-16),new Vector2(-14,9),new Vector2(14,9),new Vector2(16,-16),new Vector2(-16,-16));
                    Arc(vh,new Vector2(0,9),7,8,0,180);break;
                case Symbol.Book:
                    Path(vh,new Vector2(0,-16),new Vector2(-18,-12),new Vector2(-18,16),new Vector2(0,12),new Vector2(18,16),new Vector2(18,-12),new Vector2(0,-16),new Vector2(0,12));break;
                case Symbol.Shop:
                    Path(vh,new Vector2(-18,5),new Vector2(-12,17),new Vector2(12,17),new Vector2(18,5),new Vector2(-18,5));
                    Path(vh,new Vector2(-13,5),new Vector2(-13,-16),new Vector2(13,-16),new Vector2(13,5));
                    Path(vh,new Vector2(-4,-16),new Vector2(-4,-3),new Vector2(4,-3),new Vector2(4,-16));break;
                case Symbol.Star:
                    Path(vh,new Vector2(0,21),new Vector2(5,5),new Vector2(21,0),new Vector2(5,-5),new Vector2(0,-21),new Vector2(-5,-5),new Vector2(-21,0),new Vector2(-5,5),new Vector2(0,21));break;
                case Symbol.Mail:
                    Path(vh,new Vector2(-19,13),new Vector2(19,13),new Vector2(19,-13),new Vector2(-19,-13),new Vector2(-19,13),new Vector2(0,-2),new Vector2(19,13));break;
                case Symbol.Party:
                    Arc(vh,new Vector2(-10,9),6,6,0,360);Arc(vh,new Vector2(10,9),6,6,0,360);
                    Arc(vh,new Vector2(-10,-13),11,13,0,180);Arc(vh,new Vector2(10,-13),11,13,0,180);break;
                case Symbol.Flag:
                    Path(vh,new Vector2(-12,-20),new Vector2(-12,18),new Vector2(17,18),new Vector2(10,7),new Vector2(17,-3),new Vector2(-12,-3));break;
                case Symbol.Settings:
                    Arc(vh, Vector2.zero, 7, 7, 0, 360);
                    for (int i = 0; i < 32; i++)
                    {
                        float a = i * Mathf.PI / 16, b = (i + 1) * Mathf.PI / 16;
                        float r1 = i % 4 < 2 ? 17 : 13, r2 = (i + 1) % 4 < 2 ? 17 : 13;
                        Line(vh, new Vector2(Mathf.Cos(a), Mathf.Sin(a)) * r1,
                            new Vector2(Mathf.Cos(b), Mathf.Sin(b)) * r2);
                    }
                    break;
                case Symbol.News:
                    Path(vh, new Vector2(-13,17),new Vector2(13,17),new Vector2(13,-17),new Vector2(-13,-17),new Vector2(-13,17));
                    for(int y = -9; y <= 9; y += 9) Line(vh,new Vector2(-7,y),new Vector2(7,y));
                    break;
                case Symbol.Support:
                    Arc(vh, new Vector2(0,1), 15, 15, 0, 180);
                    Path(vh,new Vector2(-15,2),new Vector2(-15,-10),new Vector2(-9,-10),new Vector2(-9,2),new Vector2(-15,2));
                    Path(vh,new Vector2(15,2),new Vector2(15,-10),new Vector2(9,-10),new Vector2(9,2),new Vector2(15,2));
                    Path(vh,new Vector2(15,-10),new Vector2(12,-17),new Vector2(3,-17));
                    break;
                case Symbol.Account:
                    Arc(vh,new Vector2(0,9),8,8,0,360);
                    Arc(vh,new Vector2(0,-14),16,14,0,180);
                    Line(vh,new Vector2(-16,-14),new Vector2(16,-14));
                    break;
                case Symbol.Globe:
                    Arc(vh,Vector2.zero,22,22,0,360); Arc(vh,Vector2.zero,10,22,0,360);
                    Line(vh,new Vector2(-22,0),new Vector2(22,0));
                    Line(vh,new Vector2(-19,10),new Vector2(19,10));
                    Line(vh,new Vector2(-19,-10),new Vector2(19,-10));
                    break;
                case Symbol.Chevron:
                    Path(vh,new Vector2(-12,5),new Vector2(0,-7),new Vector2(12,5));
                    break;
                case Symbol.Close:
                    Line(vh,new Vector2(-10,-10),new Vector2(10,10));
                    Line(vh,new Vector2(-10,10),new Vector2(10,-10));
                    break;
            }
        }
        void Path(VertexHelper vh, params Vector2[] points)
        { for(int i=1;i<points.Length;i++) Line(vh,points[i-1],points[i]); }
        void Arc(VertexHelper vh,Vector2 center,float rx,float ry,float from,float to,float stroke=1.6f)
        {
            int n = Mathf.CeilToInt((to-from)/8);
            for(int i=0;i<n;i++)
            {
                float a=Mathf.Lerp(from,to,(float)i/n)*Mathf.Deg2Rad;
                float b=Mathf.Lerp(from,to,(float)(i+1)/n)*Mathf.Deg2Rad;
                Line(vh,center+new Vector2(Mathf.Cos(a)*rx,Mathf.Sin(a)*ry),
                    center+new Vector2(Mathf.Cos(b)*rx,Mathf.Sin(b)*ry),stroke);
            }
        }
        void Line(VertexHelper vh,Vector2 a,Vector2 b,float stroke=1.6f)
        {
            var r=rectTransform.rect; float scale=Mathf.Min(r.width,r.height)/64f;
            a=r.center+a*scale; b=r.center+b*scale;
            var d=(b-a).normalized; var normal=new Vector2(-d.y,d.x);
            var n=normal*stroke*scale*.5f;
            var outer=n+normal*.65f;
            var clear=color; clear.a=0;
            Quad(vh,a-n,a+n,b+n,b-n,color,color);
            Quad(vh,a+n,a+outer,b+outer,b+n,color,clear);
            Quad(vh,a-outer,a-n,b-n,b-outer,clear,color);
        }
        static void Quad(VertexHelper vh,Vector2 a,Vector2 b,Vector2 c,Vector2 d,Color inner,Color outer)
        {
            int at=vh.currentVertCount;
            vh.AddVert(a,inner,Vector2.zero); vh.AddVert(b,outer,Vector2.zero);
            vh.AddVert(c,outer,Vector2.zero); vh.AddVert(d,inner,Vector2.zero);
            vh.AddTriangle(at,at+1,at+2); vh.AddTriangle(at,at+2,at+3);
        }
    }
}
