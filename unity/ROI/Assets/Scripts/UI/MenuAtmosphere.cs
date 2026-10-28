using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // A small, retained UI mesh: no particles, allocations per frame or gameplay RNG.
    // Lives behind content and never receives pointer input.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class MenuAtmosphere : MaskableGraphic
    {
        float _phase, _nextFrame, _nextPolicy;
        bool _motion;
        public static void Create(Transform parent)
        {
            var go = new GameObject("AmbientLight", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var effect = go.AddComponent<MenuAtmosphere>();
            effect.raycastTarget = false;
            var r = effect.rectTransform;
            r.anchorMin = Vector2.zero; r.anchorMax = Vector2.one;
            r.offsetMin = r.offsetMax = Vector2.zero;
        }
        protected override void OnEnable() { base.OnEnable(); _nextPolicy = 0; }
        void Update()
        {
            float now = Time.unscaledTime;
            if (now >= _nextPolicy)
            {
                _nextPolicy = now + .25f;
                bool allowed = UiEntrance.MotionAllowed;
                if (_motion != allowed) { _motion = allowed; _phase = 0; SetVerticesDirty(); }
            }
            if (!_motion || now < _nextFrame) return;
            _nextFrame = now + 1f / 20f;
            _phase = now;
            SetVerticesDirty();
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear(); var r = rectTransform.rect;
            // Broad, faint atmospheric light stays on the right, away from reading columns.
            Glow(vh, new Vector2(r.xMin+r.width*(.79f+.025f*Mathf.Sin(_phase*.065f)),
                r.yMin+r.height*.60f), new Vector2(r.width*.31f,r.height*.53f),
                new Color(.34f,.53f,.70f,.13f));
            Glow(vh, new Vector2(r.xMin+r.width*.92f,r.yMin+r.height*(.31f+.035f*Mathf.Sin(_phase*.08f))),
                new Vector2(r.width*.18f,r.height*.30f),new Color(.81f,.69f,.43f,.075f));
            if (!_motion) return;
            for (int i=0;i<18;i++)
            {
                float x=.55f+Mathf.Repeat(i*.173f, .43f);
                float y=Mathf.Repeat(i*.217f+_phase*(.0025f+i*.00009f),1);
                float alpha=Mathf.Sin(y*Mathf.PI)*(.09f+.07f*Mathf.Sin(_phase*.35f+i));
                Glow(vh,new Vector2(r.xMin+x*r.width,r.yMin+y*r.height),
                    new Vector2(1.4f,2.1f),new Color(.9f,.83f,.65f,alpha));
            }
        }
        static void Glow(VertexHelper vh,Vector2 centre,Vector2 radius,Color ink)
        {
            int start=vh.currentVertCount; vh.AddVert(centre,ink,Vector2.zero);
            var edge=ink; edge.a=0;
            const int sides=24;
            for(int i=0;i<=sides;i++) {
                float a=i*Mathf.PI*2/sides;
                vh.AddVert(centre+new Vector2(Mathf.Cos(a)*radius.x,Mathf.Sin(a)*radius.y),edge,Vector2.zero);
                if(i>0)vh.AddTriangle(start,start+i,start+i+1);
            }
        }
    }
}

