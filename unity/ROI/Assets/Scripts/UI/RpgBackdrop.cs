using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // WuWa page ground: charcoal lit from the upper left, a faint diagonal light band, and a set
    // of thin concentric "resonance" rings on the right that sit far below the content.
    // This graphic is the static gradient; the band, rings and drifting motes are sibling layers
    // that RpgBackdropMotion animates (transform/alpha only, off with reduced motion/companion).
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class RpgBackdrop : MaskableGraphic
    {
        public static void Create(Transform parent)
        {
            var go=new GameObject("TacticalBackdrop",typeof(RectTransform));
            go.transform.SetParent(parent,false);
            var rt=(RectTransform)go.transform;Stretch(rt);
            var overlay=new GameObject("Atmosphere",typeof(RectTransform));overlay.transform.SetParent(go.transform,false);
            Stretch((RectTransform)overlay.transform);
            overlay.AddComponent<RpgBackdrop>().raycastTarget=false;

            var band=Layer(go.transform,"LightBand",RpgBackdropLayer.Kind.Band);
            var rings=Layer(go.transform,"ResonanceRings",RpgBackdropLayer.Kind.Rings);
            var rr=rings.rectTransform;
            rr.anchorMin=rr.anchorMax=new Vector2(.84f,.42f);rr.sizeDelta=Vector2.zero;rr.anchoredPosition=Vector2.zero;
            rings.SizeSource=rt;
            MenuAtmosphere.Create(go.transform);   // drifting motes, same cadence/gate as the inventory
            Layer(go.transform,"FootShade",RpgBackdropLayer.Kind.Vignette);

            var motion=go.AddComponent<RpgBackdropMotion>();
            motion.Root=rt;motion.Band=band.rectTransform;motion.Rings=rr;
            motion.BandGraphic=band;motion.RingsGraphic=rings;
        }
        static RpgBackdropLayer Layer(Transform parent,string name,RpgBackdropLayer.Kind kind)
        {
            var go=new GameObject(name,typeof(RectTransform));go.transform.SetParent(parent,false);
            Stretch((RectTransform)go.transform);
            var layer=go.AddComponent<RpgBackdropLayer>();layer.Layer=kind;layer.raycastTarget=false;
            return layer;
        }
        static void Stretch(RectTransform r){r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;}
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            if(r.width<=0||r.height<=0)return;
            var lit=UiTheme.BackdropTop;var deep=UiTheme.BackdropBottom;var mid=Color.Lerp(lit,deep,.55f);
            // bottom-left, top-left, top-right, bottom-right
            WuwaMesh.Quad(vh,new Vector2(r.xMin,r.yMin),new Vector2(r.xMin,r.yMax),new Vector2(r.xMax,r.yMax),new Vector2(r.xMax,r.yMin),mid,lit,mid,deep);
        }
    }
}
