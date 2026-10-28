using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained UI geometry: no particles, lights, allocations or textures per frame.
    // The sim owns readiness; the reservoir and the ready shimmer are presentation only.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class CombatUltimateGauge : MaskableGraphic
    {
        const int Segments = 96;
        float _charge, _nextFrame;
        bool _ready, _down;
        readonly Vector2[] _clipInput = new Vector2[4], _clipOutput = new Vector2[6];
        public void SetState(float charge, bool ready, bool down)
        {
            charge = Mathf.Clamp01(charge);
            if (Mathf.Abs(charge - _charge) < .0005f && ready == _ready && down == _down) return;
            _charge = charge; _ready = ready; _down = down;
            SetVerticesDirty();
        }
        void Update()
        {
            if ((_charge <= 0f && !_ready) || _down || Time.unscaledTime < _nextFrame) return;
            _nextFrame = Time.unscaledTime + 1f / 30f;
            SetVerticesDirty();
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            float time = Time.unscaledTime;
            bool lit = _ready && !_down;
            float pulse = .78f + .22f * Mathf.Sin(time * 2.4f);
            var clear = new Color(0, 0, 0, 0);
            if (lit)
            {
                // A small readiness accent, without the glass-orb halo.
                Ring(vh, .456f, .477f, new Color(.21f, .72f, .78f, .20f * pulse), clear);
            }
            var track = new Color(.025f,.045f,.065f,.72f);
            Ring(vh, 0, .413f, track, track);
            // A gently moving liquid surface, clipped to the reservoir at every column.
            // No bubbles: two shallow waves and a thin meniscus carry the motion.
            // One waterline spans the inner disc AND both frame bands.
            float top = -.46f + .92f * _charge;
            if (_charge > .0001f)
            {
                const int slices = 80;
                for (int i = 0; i < slices; i++)
                {
                    float x0 = Mathf.Lerp(-.392f, .392f, i / (float)slices);
                    float x1 = Mathf.Lerp(-.392f, .392f, (i + 1f) / slices);
                    float edge0 = Mathf.Sqrt(Mathf.Max(0, .392f * .392f - x0 * x0));
                    float edge1 = Mathf.Sqrt(Mathf.Max(0, .392f * .392f - x1 * x1));
                    float surface0 = top + Wave(x0, time);
                    float surface1 = top + Wave(x1, time);
                    float y0 = Mathf.Clamp(surface0, -edge0, edge0);
                    float y1 = Mathf.Clamp(surface1, -edge1, edge1);
                    if (y0 <= -edge0 && y1 <= -edge1) continue;
                    // Nearly flat colour; only the surface moves, not a shaded interior vortex.
                    Quad(vh,new Vector2(x0,-edge0),new Vector2(x1,-edge1),new Vector2(x1,y1),new Vector2(x0,y0),
                        FillColor((.392f-edge0)/.784f,lit),FillColor((.392f-edge1)/.784f,lit),
                        FillColor((y1+.392f)/.784f,lit),FillColor((y0+.392f)/.784f,lit));
                    if (_charge < .985f && surface0 < edge0 && surface1 < edge1)
                    {
                        var sheen=new Color(.83f,1,1,.25f);
                        var soft=new Color(.60f,.96f,1,0);
                        Quad(vh,new Vector2(x0,Mathf.Max(-edge0,y0-.012f)),new Vector2(x1,Mathf.Max(-edge1,y1-.012f)),new Vector2(x1,y1),new Vector2(x0,y0),soft,soft,sheen,sheen);
                    }
                }
            }
            // Two broad, flat dark bands. Their submerged sections use saturated colour,
            // not a white outline; both share the exact level of the central fill.
            var outer=new Color(.19f,.23f,.26f,.92f);
            var inner=new Color(.11f,.16f,.18f,.88f);
            Ring(vh,.411f,.46f,outer,outer);
            Ring(vh,.383f,.404f,inner,inner);
            if (_charge > 0)
            {
                FilledRing(vh,.411f,.46f,top,_down ? inner : new Color(.08f,.47f,.54f,.98f));
                FilledRing(vh,.383f,.404f,top,_down ? inner : new Color(.17f,.54f,.60f,.96f));
            }
            Ring(vh,.46f,.466f,outer,new Color(outer.r,outer.g,outer.b,0));
            if (lit)
            {
                Spark(vh, 115, .46f, .030f, .65f*pulse);
                Spark(vh, 288, .46f, .020f, .35f*(1.56f-pulse));
            }
        }
        void FilledRing(VertexHelper vh,float inner,float outer,float top,Color tint)
        {
            var r=GetPixelAdjustedRect();float scale=Mathf.Min(r.width,r.height);
            for(int i=0;i<Segments;i++)
            {
                var p=Direction(i*360f/Segments);var q=Direction((i+1)*360f/Segments);
                _clipInput[0]=p*inner;_clipInput[1]=p*outer;
                _clipInput[2]=q*outer;_clipInput[3]=q*inner;
                int count=0;
                var previous=_clipInput[3];bool prevInside=previous.y<=top;
                for(int j=0;j<4;j++)
                {
                    var current=_clipInput[j];bool inside=current.y<=top;
                    if(inside!=prevInside)
                        _clipOutput[count++]=Vector2.Lerp(previous,current,(top-previous.y)/(current.y-previous.y));
                    if(inside)_clipOutput[count++]=current;
                    previous=current;prevInside=inside;
                }
                int start=vh.currentVertCount;
                for(int j=0;j<count;j++)vh.AddVert(r.center+_clipOutput[j]*scale,tint,Vector2.zero);
                for(int j=1;j<count-1;j++)vh.AddTriangle(start,start+j,start+j+1);
            }
        }
        float Wave(float x, float time)
        {
            if (_down) return 0f;
            float fade = Mathf.Sin(Mathf.PI * _charge);
            return (Mathf.Sin(x * 12f + time * 1.5f) * .004f
                  + Mathf.Sin(x * 21f - time * 1.0f) * .002f) * fade;
        }
        Color FillColor(float y, bool ready)
        {
            if (_down) return new Color(.28f,.34f,.37f,.4f);
            return ready ? Color.Lerp(new Color(.24f,.60f,.68f,.95f),new Color(.29f,.65f,.73f,.95f),y)
                         : Color.Lerp(new Color(.22f,.65f,.73f,.85f),new Color(.28f,.71f,.78f,.85f),y);
        }
        void Ring(VertexHelper vh, float inner, float outer, Color a, Color b)
        {
            for (int i=0;i<Segments;i++)
            {
                var p=Direction(i*360f/Segments); var q=Direction((i+1)*360f/Segments);
                Quad(vh,p*inner,p*outer,q*outer,q*inner,a,b,b,a);
            }
        }
        void Spark(VertexHelper vh,float angle,float radius,float size,float alpha)
        {
            var p=Direction(angle)*radius; var c=new Color(1,.97f,.83f,alpha);
            Quad(vh,p+Vector2.up*size,p+Vector2.right*size*.26f,p-Vector2.up*size,p-Vector2.right*size*.26f,c,c,c,c);
            Quad(vh,p+Vector2.right*size,p+Vector2.up*size*.22f,p-Vector2.right*size,p-Vector2.up*size*.22f,c,c,c,c);
        }
        static Vector2 Direction(float angle)=>new Vector2(Mathf.Cos(angle*Mathf.Deg2Rad),Mathf.Sin(angle*Mathf.Deg2Rad));
        void Quad(VertexHelper vh,Vector2 a,Vector2 b,Vector2 c,Vector2 d,Color ca,Color cb,Color cc,Color cd)
        {
            var r=GetPixelAdjustedRect(); float s=Mathf.Min(r.width,r.height); var p=r.center;
            int n=vh.currentVertCount;
            vh.AddVert(p+a*s,ca,Vector2.zero); vh.AddVert(p+b*s,cb,Vector2.zero);
            vh.AddVert(p+c*s,cc,Vector2.zero); vh.AddVert(p+d*s,cd,Vector2.zero);
            vh.AddTriangle(n,n+1,n+2); vh.AddTriangle(n,n+2,n+3);
        }
    }
}
