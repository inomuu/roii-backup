using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Shared prism currency atlas. Plus/Arrow remain lightweight native controls.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeCurrencyIcon : MaskableGraphic
    {
        public static readonly Color GemColor = Color.white;
        public static readonly Color TicketColor = Color.white;
        public string Kind;

        static Texture2D _atlas;
        // Four alpha-separated cells, sampled without modifying the approved artwork.
        static readonly Rect[] Ink = {new Rect(0.01966717f,0.23581337f,0.23499748f,0.51071879f),
            new Rect(0.28643469f,0.22068096f,0.18305598f,0.59646910f),
            new Rect(0.51336359f,0.23455233f,0.19364599f,0.53341740f),
            new Rect(0.74230963f,0.23707440f,0.24004034f,0.50819672f)};
        static Material _paintMaterial;
        int ItemIndex => Kind switch {"Gil"=>0,"Soul"=>1,"Gem"=>2,"Ticket"=>3,_=>-1};
        Texture2D ItemTexture
        {
            get
            {
                int i=ItemIndex;
                if(i<0)return null;
                if(_atlas==null)_atlas=Resources.Load<Texture2D>("HomeUI/Currency81/PrismAtlas");
                return _atlas;
            }
        }
        public override Texture mainTexture => ItemTexture!=null ? ItemTexture : Texture2D.whiteTexture;
        public override Material defaultMaterial
        {
            get
            {
                if(ItemIndex<0)return HomeIconSet.Material;
                if(_paintMaterial==null)
                {
                    var shader=Resources.Load<Shader>("HomeUI/CurrencyPaint");
                    if(shader!=null)_paintMaterial=new Material(shader){name="Painted currency items",hideFlags=HideFlags.HideAndDontSave};
                }
                return _paintMaterial!=null ? _paintMaterial : base.defaultMaterial;
            }
        }
        void PaintItem(Texture2D texture)
        {
            var uv=Ink[ItemIndex];var rect=rectTransform.rect;
            var pixels=new Vector2(uv.width*texture.width,uv.height*texture.height);
            float scale=Mathf.Min(rect.width/pixels.x,rect.height/pixels.y);
            var size=pixels*scale;var min=rect.center-size*.5f;var max=rect.center+size*.5f;
            _mesh.AddVert(new Vector3(min.x,min.y),color,new Vector2(uv.xMin,uv.yMin));
            _mesh.AddVert(new Vector3(min.x,max.y),color,new Vector2(uv.xMin,uv.yMax));
            _mesh.AddVert(new Vector3(max.x,max.y),color,new Vector2(uv.xMax,uv.yMax));
            _mesh.AddVert(new Vector3(max.x,min.y),color,new Vector2(uv.xMax,uv.yMin));
            _mesh.AddTriangle(0,1,2);_mesh.AddTriangle(2,3,0);
        }
        VertexHelper _mesh;
        float _feather;

        Vector2 Point(Vector2 p)
        {
            var r=rectTransform.rect;
            var size=Kind=="Ticket" ? r.size : Vector2.one*Mathf.Min(r.width,r.height);
            return r.center+Vector2.Scale(p-new Vector2(.5f,.5f),size);
        }
        Color Shade(float brightness,float alpha=1)
            => new Color(color.r*brightness,color.g*brightness,color.b*brightness,color.a*alpha);
        void Polygon(Color tint,params Vector2[] points)
        {
            int start=_mesh.currentVertCount;
            foreach(var p in points)_mesh.AddVert(Point(p),tint,Vector2.zero);
            for(int i=1;i<points.Length-1;i++)_mesh.AddTriangle(start,start+i,start+i+1);
        }
        void Line(Vector2 a,Vector2 b,float width,Color tint)
        {
            var normal=new Vector2(-(b-a).y,(b-a).x).normalized;
            var n=normal*width*.5f;
            Polygon(tint,a-n,a+n,b+n,b-n);
            Feather(a-n,b-n,-normal*_feather,-normal*_feather,tint);
            Feather(a+n,b+n,normal*_feather,normal*_feather,tint);
        }
        void Feather(Vector2 a,Vector2 b,Vector2 outsideA,Vector2 outsideB,Color tint)
        {
            int start=_mesh.currentVertCount;var clear=tint;clear.a=0;
            _mesh.AddVert(Point(a),tint,Vector2.zero);_mesh.AddVert(Point(b),tint,Vector2.zero);
            _mesh.AddVert(Point(b+outsideB),clear,Vector2.zero);_mesh.AddVert(Point(a+outsideA),clear,Vector2.zero);
            _mesh.AddTriangle(start,start+1,start+2);_mesh.AddTriangle(start,start+2,start+3);
        }
        void Outline(float width,Color tint,params Vector2[] points)
        {
            var normals=new Vector2[points.Length];
            for(int i=0;i<points.Length;i++) {
                var incoming=(points[i]-points[(i+points.Length-1)%points.Length]).normalized;
                var outgoing=(points[(i+1)%points.Length]-points[i]).normalized;
                var a=new Vector2(-incoming.y,incoming.x);var b=new Vector2(-outgoing.y,outgoing.x);
                var miter=(a+b).normalized;
                normals[i]=miter/Mathf.Max(.2f,Vector2.Dot(miter,b));
            }
            for(int i=0;i<points.Length;i++) {
                int next=(i+1)%points.Length;
                StrokeSegment(points[i],points[next],normals[i],normals[next],width,tint);
            }
        }
        void StrokeSegment(Vector2 a,Vector2 b,Vector2 na,Vector2 nb,float width,Color tint)
        {
            var oa=na*width*.5f;var ob=nb*width*.5f;
            Polygon(tint,a-oa,a+oa,b+ob,b-ob);
            Feather(a-oa,b-ob,-na*_feather,-nb*_feather,tint);
            Feather(a+oa,b+ob,na*_feather,nb*_feather,tint);
        }
        protected override void OnPopulateMesh(VertexHelper mesh)
        {
            mesh.Clear();_mesh=mesh;
            var item=ItemTexture;
            if(item!=null){PaintItem(item);return;}
            _feather=.7f/Mathf.Max(1,Mathf.Min(rectTransform.rect.width,rectTransform.rect.height));
            if(Kind=="Plus"||Kind=="Arrow")
            {
                Outline(.05f,Shade(.72f),new(.30f,.08f),new(.70f,.08f),new(.92f,.30f),new(.92f,.70f),
                    new(.70f,.92f),new(.30f,.92f),new(.08f,.70f),new(.08f,.30f));
                Line(new(.28f,.5f),new(.72f,.5f),.09f,Shade(1));
                if(Kind=="Plus")Line(new(.5f,.28f),new(.5f,.72f),.09f,Shade(1));
                else {Line(new(.52f,.70f),new(.72f,.5f),.08f,Shade(1));Line(new(.52f,.30f),new(.72f,.5f),.08f,Shade(1));}
                return;
            }
        }
    }
}
