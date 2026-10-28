using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Resolution-independent engraving, kept separate from the live text and hit targets.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeCelestialArt : MaskableGraphic
    {
        public enum Motif { Compass, Sigil, Chart, Rule, BattleFrame }
        public Motif Kind;
        public override Material defaultMaterial => HomeIconSet.Material ?? base.defaultMaterial;
        static readonly Color Gold = UiTheme.Accent;
        Color Tint(float alpha) => new Color(Gold.r,Gold.g,Gold.b,alpha*color.a*(Kind==Motif.Chart?.24f:1));

        protected override void OnPopulateMesh(VertexHelper v)
        {
            v.Clear();var r=rectTransform.rect;var c=r.center;
            float radius=Mathf.Min(r.width,r.height)*.43f;
            if(Kind==Motif.Compass) Compass(v,c,radius);
            else if(Kind==Motif.Sigil) {
                float w=Mathf.Min(r.width,r.height*.72f);
                var o=c+Vector2.up*r.height*.08f;
                Arc(v,o,w*.43f,18,342,1,Tint(.75f));
                Arc(v,o,w*.38f,120,440,.65f,Tint(.43f));
                Line(v,new Vector2(c.x,r.yMin+4),new Vector2(c.x,r.yMax-3),.8f,Tint(.75f));
                Star(v,o,w*.50f,w*.115f,Tint(.95f),Tint(.4f),true);
                Star(v,o-Vector2.up*w*.32f,w*.23f,w*.085f,Tint(.8f),Tint(.3f),true);
                Line(v,o+Vector2.left*w*.54f,o+Vector2.right*w*.54f,.8f,Tint(.85f));
                Arc(v,o-Vector2.up*w*.10f,w*.28f,190,350,.7f,Tint(.7f));
                Diamond(v,new Vector2(c.x,r.yMin+10),4,7,Tint(.9f));
            } else if(Kind==Motif.Chart) {
                c=new Vector2(r.xMin+r.width*.13f,r.yMin+r.height*.52f);
                radius=r.height*.47f;
                for(int ring=0;ring<3;ring++) {
                    float rad=radius+ring*7;
                    ArcClipped(v,r,c,rad,-65,88,.75f,Tint(ring==1?.19f:.27f));
                }
                ArcClipped(v,r,c,radius*.83f,-65,92,.65f,Tint(.10f));
                for(int i=-60;i<88;i+=3) {
                    var dir=Direction(i);var a=c+dir*(radius+9);
                    var b=c+dir*(radius+(i%15==0?23:15));
                    if(r.Contains(a)&&r.Contains(b))Line(v,a,b,.65f,Tint(i%15==0?.30f:.18f));
                }
                for(int i=0;i<4;i++) {
                    float angle=20+i*22;var p=c+Direction(angle)*radius*.88f;
                    if(r.Contains(p))Star(v,p,i==2?13:4,2,Tint(.30f),Tint(.12f));
                }
            } else if(Kind==Motif.Rule) {
                Line(v,new Vector2(r.xMin,c.y),new Vector2(r.xMax,c.y),.85f,Tint(.68f));
                Star(v,c,8,2.4f,Tint(.95f),Tint(.55f));
                Diamond(v,c+Vector2.left*13,2,2,Tint(.7f));
                Diamond(v,c+Vector2.right*13,2,2,Tint(.7f));
            } else if(Kind==Motif.BattleFrame) {
                var points=CutCorners(r,Mathf.Min(29,r.height*.33f),1.5f);
                for(int i=0;i<points.Length;i++)Line(v,points[i],points[(i+1)%points.Length],.75f,Tint(.9f));
                var inner=CutCorners(r,Mathf.Min(27,r.height*.31f),5);
                for(int i=0;i<inner.Length;i++)Line(v,inner[i],inner[(i+1)%inner.Length],.4f,Tint(.12f));
                Compass(v,new Vector2(r.xMax-r.height*.48f,c.y),r.height*.31f);
                Star(v,new Vector2(r.xMin+r.height*.33f,c.y),5,1.4f,Tint(.8f),Tint(.25f));
            }
        }
        void Compass(VertexHelper v,Vector2 c,float radius)
        {
            Arc(v,c,radius*.83f,0,360,.8f,Tint(.72f));
            Arc(v,c,radius*.73f,0,360,.7f,Tint(.55f));
            Arc(v,c,radius*.93f,17,163,.65f,Tint(.48f));
            Arc(v,c,radius*.93f,197,343,.65f,Tint(.48f));
            for(int i=0;i<360;i+=10) {
                var d=Direction(i);
                Line(v,c+d*radius*.76f,c+d*radius*(i%30==0?.94f:.85f),.65f,Tint(.60f));
            }
            Line(v,c-Vector2.right*radius*1.12f,c+Vector2.right*radius*1.25f,.7f,Tint(.75f));
            Line(v,c-Vector2.up*radius*1.10f,c+Vector2.up*radius*1.12f,.7f,Tint(.75f));
            Star(v,c,radius*.83f,radius*.17f,Tint(.97f),Tint(.38f));
            Diamond(v,c+Vector2.right*radius*1.22f,2,3,Tint(.85f));
        }
        static Vector2 Direction(float degrees) => new Vector2(Mathf.Cos(degrees*Mathf.Deg2Rad),Mathf.Sin(degrees*Mathf.Deg2Rad));
        static void Star(VertexHelper v,Vector2 c,float radius,float inner,Color light,Color dark,bool spear=false)
        {
            int count=spear?4:8;
            for(int i=0;i<count;i++) {
                float angle=90+i*360f/count;
                float length=radius*(spear?(i%2==0?1.45f:.52f):(i%2==0?1:.65f));
                var tip=c+Direction(angle)*length;
                Triangle(v,c,c+Direction(angle-180f/count)*inner,tip,light);
                Triangle(v,c,tip,c+Direction(angle+180f/count)*inner,dark);
            }
        }
        static void Diamond(VertexHelper v,Vector2 c,float x,float y,Color color)
        { Triangle(v,c+Vector2.left*x,c+Vector2.up*y,c+Vector2.right*x,color);Triangle(v,c+Vector2.left*x,c+Vector2.right*x,c+Vector2.down*y,color); }
        static void Triangle(VertexHelper v,Vector2 a,Vector2 b,Vector2 c,Color color)
        { int n=v.currentVertCount;v.AddVert(a,color,Vector2.zero);v.AddVert(b,color,Vector2.zero);v.AddVert(c,color,Vector2.zero);v.AddTriangle(n,n+1,n+2); }
        static void Arc(VertexHelper v,Vector2 c,float radius,float from,float to,float width,Color color)
        { for(float a=from;a<to;a+=2)Line(v,c+Direction(a)*radius,c+Direction(Mathf.Min(to,a+2))*radius,width,color); }
        static void ArcClipped(VertexHelper v,Rect r,Vector2 c,float radius,float from,float to,float width,Color color)
        { for(float a=from;a<to;a+=2){var p=c+Direction(a)*radius;var q=c+Direction(Mathf.Min(to,a+2))*radius;if(r.Contains(p)&&r.Contains(q))Line(v,p,q,width,color);} }
        // Transparent fringes keep sub-pixel gold lines continuous at phone resolution.
        static void Line(VertexHelper v,Vector2 a,Vector2 b,float width,Color color)
        {
            var d=(b-a).normalized;var normal=new Vector2(-d.y,d.x);
            float[] offsets={-width*.5f-1,-width*.5f,width*.5f,width*.5f+1};
            int n=v.currentVertCount;
            for(int i=0;i<4;i++) {
                var tint=color;if(i==0||i==3)tint.a=0;
                v.AddVert(a+normal*offsets[i],tint,Vector2.zero);v.AddVert(b+normal*offsets[i],tint,Vector2.zero);
            }
            for(int i=0;i<3;i++){int k=n+i*2;v.AddTriangle(k,k+1,k+3);v.AddTriangle(k,k+3,k+2);}
        }
        internal static Vector2[] CutCorners(Rect r,float cut,float inset)
        {
            float l=r.xMin+inset,b=r.yMin+inset,t=r.yMax-inset,h=r.xMax-inset;
            return new[]{new Vector2(l+cut,b),new Vector2(h-cut,b),new Vector2(h,b+cut),new Vector2(h,t-cut),new Vector2(h-cut,t),new Vector2(l+cut,t),new Vector2(l,t-cut),new Vector2(l,b+cut)};
        }
    }

    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeBattleBackdrop : MaskableGraphic
    {
        static Texture2D _texture;
        static Material _material;
        public override Texture mainTexture => _texture!=null?_texture:(_texture=Resources.Load<Texture2D>("HomeUI/EclipseCitadel"));
        public override Material defaultMaterial {
            get {
                if(_material==null)_material=new Material(Resources.Load<Shader>("HomeUI/CurrencyPaint")){name="Home battle artwork",hideFlags=HideFlags.HideAndDontSave};
                return _material;
            }
        }
        protected override void OnPopulateMesh(VertexHelper v)
        {
            v.Clear();var r=rectTransform.rect;
            var points=HomeCelestialArt.CutCorners(r,Mathf.Min(29,r.height*.33f),1.5f);
            // A darker text area on the left fades into the illustrated destination on the right.
            var center=r.center;Add(v,center,r);
            foreach(var p in points)Add(v,p,r);
            for(int i=0;i<points.Length;i++)v.AddTriangle(0,i+1,(i+1)%points.Length+1);
        }
        void Add(VertexHelper v,Vector2 p,Rect r)
        {
            float x=(p.x-r.xMin)/r.width,y=(p.y-r.yMin)/r.height;
            float shade=Mathf.Lerp(.055f,.52f,Mathf.Pow(x,3));
            v.AddVert(p,new Color(shade*.83f,shade*.94f,shade,color.a),new Vector2(.3f+x*.7f,.4f+y*.4f));
        }
    }
}
