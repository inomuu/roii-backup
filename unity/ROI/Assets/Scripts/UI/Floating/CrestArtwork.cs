using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // A nested canvas isolates moving artwork from the inventory's text and item grid.
    // No scene lights, particles, render textures, sim ticks or full-window post effects.
    public sealed class CrestArtwork : MonoBehaviour
    {
        RawImage _crest,_magic;
        RectTransform _orbit,_floating;
        Material _sky,_magicMaterial;
        readonly Texture2D[] _atlases=new Texture2D[5];
        int _group=-1,_tier=-1;
        static readonly int MotionTime=Shader.PropertyToID("_MotionTime");
        static readonly int SparkleTime=Shader.PropertyToID("_SparkleTime");
        // Crest Tier identity (not equipment grade): pearl, ice, jade, gold, rose.
        static readonly Color[] TierAccents={new(.96f,.90f,.77f),new(.35f,.72f,1f),new(.46f,.88f,.71f),new(1f,.68f,.22f),new(1f,.40f,.67f)};
        // Visible alpha bounds measured from the approved 1254px sheets (bottom-left origin).
        // Do not center the untrimmed quadrant: its uneven transparent margins move the Crest.
        static readonly Rect[] T3Bounds={new(68,627,524,616),new(655,627,543,579),new(61,42,523,582),new(654,49,545,567)};
        static readonly Rect[] T4Bounds={new(70,627,515,604),new(681,629,496,564),new(87,59,462,568),new(674,65,509,535)};
        static readonly Rect[] T5Bounds={new(67,629,525,610),new(654,627,540,582),new(56,47,534,578),new(650,54,550,573)};
        static readonly Rect[] T6Bounds={new(64,627,529,612),new(652,627,543,593),new(52,48,541,578),new(648,53,553,574)};
        static readonly Rect[] T7Bounds={new(59,627,537,615),new(643,627,563,592),new(45,48,552,579),new(640,49,570,578)};
        static readonly Rect[][] BoundsByTier={T3Bounds,T4Bounds,T5Bounds,T6Bounds,T7Bounds};
        public bool HasArtwork(int tier)=>tier>=3&&tier<=7&&_atlases[tier-3]!=null;
        public void Build()
        {
            gameObject.AddComponent<Canvas>();
            var background=ForgeUi.Panel(transform,"CelestialBackdrop",0,0,300,232,Color.white);
            background.raycastTarget=false;
            var shader=Resources.Load<Shader>("Crests/CrestAtmosphere");
            if(shader!=null)
            {
                _sky=new Material(shader){name="Crest ambient motion"};
                _sky.SetFloat("_Aspect",300f/232f);
                var nebula=Resources.Load<Texture2D>("Crests/NebulaBackdrop");
                if(nebula!=null){_sky.SetTexture("_NebulaTex",nebula);_sky.SetFloat("_UseNebulaTexture",1);}
                background.material=_sky;
            }
            else background.color=new Color(.075f,.09f,.14f);
            _floating=ForgeUi.Rect(transform,"FloatingCrest",150,116,0,0);
            _floating.pivot=new Vector2(.5f,.5f);
            _orbit=ForgeUi.Rect(_floating,"Orbit",0,0,226,226);
            _orbit.pivot=new Vector2(.5f,.5f);
            var ring=_orbit.gameObject.AddComponent<CrestOrbit>();ring.raycastTarget=false;
            var magicTexture=Resources.Load<Texture2D>("Crests/MagicCircle");
            var glow=Resources.Load<Shader>("Crests/WhiteCircleGlow");
            if(magicTexture!=null&&glow!=null&&glow.isSupported)
            {
                // Luminance mask: black becomes zero opacity in the glow shader. Keep all
                // four outer terminals inside the stage even while the circle rotates.
                var magicRect=ForgeUi.Rect(_floating,"MagicCircle",0,0,248,248);
                magicRect.pivot=new Vector2(.5f,.5f);
                _magic=magicRect.gameObject.AddComponent<RawImage>();_magic.raycastTarget=false;
                _magic.texture=magicTexture;_magic.color=new Color(1f,1f,1f,.56f);
                _magicMaterial=new Material(glow){name="Crest tier magic-circle glow"};_magic.material=_magicMaterial;
                _orbit.gameObject.SetActive(false);
            }
            var rect=ForgeUi.Rect(_floating,"Artwork",0,0,158,158);
            rect.pivot=new Vector2(.5f,.5f);
            _crest=rect.gameObject.AddComponent<RawImage>();_crest.raycastTarget=false;
            for(int i=0;i<_atlases.Length;i++)_atlases[i]=Resources.Load<Texture2D>("Crests/T"+(i+3)+"-Atlas");
            BuildFrame();
        }
        void BuildFrame()
        {
            // Stationary inset mount anchors the floating illustration to the detail panel.
            // Retained UI lines stay crisp at every companion scale and never receive clicks.
            var frame=ForgeUi.Rect(transform,"CrestMount",0,0,300,232);
            UiTheme.Border(frame,new Color(.48f,.43f,.33f,.72f),1).raycastTarget=false;
            var inner=ForgeUi.Rect(frame,"Inset",4,4,292,224);
            UiTheme.Border(inner,new Color(.50f,.51f,.53f,.16f),1).raycastTarget=false;
            var gold=new Color(.77f,.67f,.46f,.75f);
            for(int i=0;i<4;i++)
            {
                bool right=(i&1)!=0,bottom=(i&2)!=0;
                float x=right?292:8,y=bottom?224:8;
                ForgeUi.Panel(frame,"CornerHorizontal"+i,right?x-19:x,bottom?y-1:y,19,1,gold).raycastTarget=false;
                ForgeUi.Panel(frame,"CornerVertical"+i,right?x-1:x,bottom?y-19:y,1,19,gold).raycastTarget=false;
            }
            // Small central clasps keep the frame ornamental without enclosing the art in a box.
            for(int i=0;i<2;i++)
            {
                var mark=ForgeUi.Panel(frame,"Clasp"+i,150,i==0?4:228,4,4,gold);
                mark.raycastTarget=false;mark.rectTransform.pivot=new Vector2(.5f,.5f);
                mark.rectTransform.localRotation=Quaternion.Euler(0,0,45);
            }
        }
        public void Bind(int group,int tier)
        {
            if(_group==group&&_tier==tier)return;
            _group=group;_tier=tier;
            int tierIndex=Mathf.Clamp(tier-3,0,4);
            _crest.texture=_atlases[tierIndex];
            _crest.enabled=_crest.texture!=null;
            var bounds=BoundsByTier[tierIndex][Mathf.Clamp(group,0,3)];
            // Two source pixels preserve the antialiased edge without bleeding adjacent cells.
            float cellX=group%2*627,cellY=group<2?627:0;
            float x=Mathf.Max(cellX,bounds.xMin-2),y=Mathf.Max(cellY,bounds.yMin-2);
            float width=Mathf.Min(cellX+627,bounds.xMax+2)-x,height=Mathf.Min(cellY+627,bounds.yMax+2)-y;
            _crest.uvRect=new Rect(x/1254,y/1254,width/1254,height/1254);
            float scale=158/Mathf.Max(width,height);
            _crest.rectTransform.sizeDelta=new Vector2(width*scale,height*scale);
            var tierColor=TierAccents[Mathf.Clamp(tier-3,0,4)];
            var accent=tierColor;accent.a=.29f;
            _orbit.GetComponent<CrestOrbit>().color=accent;
            if(_magicMaterial!=null)_magicMaterial.SetColor("_GlowColor",tierColor);
            if(_sky!=null)_sky.SetColor("_Accent",tierColor);
        }
        void Update()
        {
            if(_crest==null)return;
            float t=Time.unscaledTime;
            // The mount and circle keep a fixed center; only the illustration levitates.
            _crest.rectTransform.anchoredPosition=new Vector2(0,Mathf.Sin(t*.8f)*2.5f);
            _orbit.localRotation=Quaternion.Euler(0,0,t*1.1f);
            if(_magic!=null)_magic.rectTransform.localRotation=Quaternion.Euler(0,0,-t*.9f);
            if(_magicMaterial!=null)_magicMaterial.SetFloat(MotionTime,t);
            if(_sky!=null){_sky.SetFloat(MotionTime,t*.6f);_sky.SetFloat(SparkleTime,t);}
        }
        void OnDestroy(){if(_sky!=null)Destroy(_sky);if(_magicMaterial!=null)Destroy(_magicMaterial);}
    }
}
