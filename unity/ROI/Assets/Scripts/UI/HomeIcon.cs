using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // One authored celestial family shared by lobby, navigation, headings and selected states.
    // The source is a white-on-black mask. Unity imports luminance as alpha; the UI owns colour.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeIcon : MaskableGraphic
    {
        public string Kind;
        static Texture2D _atlas;
        static Texture2D Atlas => _atlas!=null ? _atlas : _atlas=Resources.Load<Texture2D>("HomeUI/CelestialEmblems-62");
        public override Texture mainTexture => Atlas;
        public override Material defaultMaterial => HomeIconSet.Material!=null ? HomeIconSet.Material : base.defaultMaterial;

        // Pixel-space ink bounds, top-left origin. Trim the generated sheet's unequal gutters
        // without resampling the artwork or clipping detached comet / star details.
        static readonly Vector4[] InkBounds = {
            new(36,42,209,210),
            new(240,34,407,209),
            new(439,56,606,189),
            new(648,62,809,202),
            new(846,35,1029,203),
            new(1078,35,1213,219),
            new(58,230,180,405),
            new(239,249,407,401),
            new(441,230,605,396),
            new(642,230,817,397),
            new(851,241,1024,396),
            new(1058,264,1224,375),
            new(43,435,201,600),
            new(232,435,408,601),
            new(434,424,603,605),
            new(660,430,794,599),
            new(846,437,1022,600),
            new(1060,421,1221,604),
            new(32,625,208,793),
            new(255,630,380,795),
            new(432,627,604,808),
            new(640,644,821,794),
            new(855,628,1020,793),
            new(1060,631,1225,800),
            new(33,824,207,974),
            new(239,824,391,991),
            new(447,823,597,990),
            new(632,831,824,976),
            new(838,839,1036,976),
            new(1060,828,1220,988),
            new(35,1013,210,1188),
            new(226,1009,414,1198),
            new(437,1015,606,1194),
            new(640,1008,816,1190),
            new(836,1025,1026,1174),
            new(1049,1024,1229,1174)
        };

        static int Index(string kind) => kind switch {
            "Character" or "Exemplar" or "Exemplars" or "Attributes" => 0,
            "Party" => 1, "Inventory" => 2, "Stash" => 3,
            "Workshop" or "Secondary1" => 4, "Equipment" => 5,
            "Apothecary" or "Potions" or "Consumable" => 6,
            "Provisions" or "Secondary0" => 7,
            "Store" or "Shop" or "Secondary2" => 8,
            "Trade" => 9, "Crystal" => 10, "Mail" => 11,
            "Daily" or "Mission" => 12, "Weekly" => 13, "Pass" => 14,
            "Commander" => 15, "Outpost" => 16,
            "Record" or "Achievement" or "Secondary3" => 17,
            "Story" => 18, "Album" => 19, "Bestiary" or "TartarusWolf" => 20,
            "Map" => 21, "Dungeon" => 22, "Cards" => 23, "Regalia" => 24,
            "Skills" or "Potential" => 25, "Paths" => 26, "Apex" => 27,
            "Resonance" => 28, "Options" => 29, "Events" or "Event" => 30,
            "Summon" or "Excavation" => 31, "Combat" => 32,
            "Info" => 33, "Probability" => 34, _ => 35
        };

        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var texture=Atlas;if(texture==null)return;
            var r=rectTransform.rect;float size=Mathf.Min(r.width,r.height);if(size<=0)return;
            int index=Index(Kind);
            // At badge size, retain the signature star/orbit and omit the long comet cluster.
            if(index==31 && size*(canvas!=null?canvas.scaleFactor:1)<22)index=35;
            var b=InkBounds[index];float width=b.z-b.x,height=b.w-b.y;
            float scale=size*.94f/Mathf.Max(width,height);
            var half=new Vector2(width,height)*scale*.5f;
            Vector2 lo=r.center-half,hi=r.center+half;
            Vector2 uvLo=new(b.x/texture.width,1-b.w/texture.height),uvHi=new(b.z/texture.width,1-b.y/texture.height);
            Color top=color,bottom=new(color.r*.82f,color.g*.82f,color.b*.82f,color.a);
            vh.AddVert(new Vector2(lo.x,lo.y),bottom,new Vector2(uvLo.x,uvLo.y));
            vh.AddVert(new Vector2(lo.x,hi.y),top,new Vector2(uvLo.x,uvHi.y));
            vh.AddVert(new Vector2(hi.x,hi.y),top,new Vector2(uvHi.x,uvHi.y));
            vh.AddVert(new Vector2(hi.x,lo.y),bottom,new Vector2(uvHi.x,uvLo.y));
            vh.AddTriangle(0,1,2);vh.AddTriangle(2,3,0);
        }
    }
}
