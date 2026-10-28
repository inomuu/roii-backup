using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Shared gear artwork for inventory, worn items, pickers and Forge results.
    // Empty slots and unknown items retain white category symbols as a fallback.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarEquipmentIcon : MaskableGraphic
    {
        string _shape = "Sword";
        Texture2D _symbol;
        Rect _uv = new(0, 0, 1, 1);
        bool _artwork;
        static readonly Dictionary<string, Texture2D> Symbols = new();
        VertexHelper _mesh;
        Vector2 _center;
        float _unit;
        Color32 _ink;
        public string ShapeName => _shape;
        public override Texture mainTexture => _symbol != null ? _symbol : base.mainTexture;

        // Resources owns these textures. Clearing the lookup is enough; never destroy them.
        // This also picks up newly imported symbols when entering Play with domain reload disabled.
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.SubsystemRegistration)]
        static void ResetSymbolCache() => Symbols.Clear();

        protected override void OnEnable()
        {
            base.OnEnable();
            if (!_artwork) ApplyShape(_shape);
        }

        public static ExemplarEquipmentIcon Create(Transform parent, string slot)
        {
            var go = new GameObject("EquipmentCategory", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var icon = go.AddComponent<ExemplarEquipmentIcon>();
            var rt = icon.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = rt.offsetMax = Vector2.zero;
            icon.raycastTarget = false;
            icon.color = ExemplarUi.White;
            icon.SetItem(null, slot);
            return icon;
        }

        public void SetItem(ItemView item, string slot)
        {
            string shape = Resolve(item, slot);
            if (!EquipmentArtwork.TryGet(item, out var texture, out var uv))
            {
                ApplyShape(shape);
                return;
            }
            if (_artwork && _symbol == texture && _uv == uv && _shape == shape) return;
            bool textureChanged = _symbol != texture;
            _shape = shape; _symbol = texture; _uv = uv; _artwork = true;
            SetVerticesDirty();
            if (textureChanged) SetMaterialDirty();
        }

        void ApplyShape(string shape)
        {
            var uv = new Rect(0, 0, 1, 1);
            if (!GeneratedGearSymbols.TryGet(shape, out var texture, out uv) && !Symbols.TryGetValue(shape, out texture))
            {
                // Cache absent categories too, so fallback glyphs do not trigger repeated IO.
                texture = Resources.Load<Texture2D>("ExemplarUI/GearSymbols/" + shape);
                Symbols[shape] = texture;
            }
            if (!_artwork && _shape == shape && _symbol == texture && _uv == uv) return;
            bool textureChanged = _symbol != texture;
            _shape = shape; _symbol = texture; _artwork = false;
            // Generated slot symbols share an atlas; legacy weapon variants use full textures.
            _uv = uv;
            SetVerticesDirty();
            if (textureChanged) SetMaterialDirty();
        }

        static string Resolve(ItemView item, string slot)
        {
            if (string.IsNullOrEmpty(slot))
                slot = !string.IsNullOrEmpty(item?.slot) ? item.slot : item?.equip_slot;
            bool weapon = item?.kind == "Weapon" || slot == "Weapon";
            if (weapon || slot == "Shield" || slot == "OffHand" || slot == "Off-Hand")
            {
                // An off-hand dagger replaces the shield glyph, just as it replaces that cell.
                switch (item?.weapon_type)
                {
                    case "Sword1H": case "Sword2H": return "Sword";
                    case "Staff": return "Staff";
                    case "Bow": return "Bow";
                    case "Dagger": return "Dagger";
                    case "Mace": return "Mace";
                }
                return weapon ? "Sword" : "Shield";
            }
            return slot switch
            {
                "Helmet" or "Head" => "Helmet",
                "Gloves" => "Gloves",
                "Armor" => "Armor",
                "Earring" => "Earring",
                "Shoes" => "Boots",
                "Necklace" => "Necklace",
                "Bracer" => "Bracer",
                "Ring" => "Ring",
                _ => "Equipment"
            };
        }

        // Normalized geometry is cached once; rebuilding the retained mesh allocates no arrays.
        static readonly Vector2[] SwordBlade = { new(.40f,.51f), new(.48f,.43f), new(.82f,.80f), new(.87f,.91f), new(.76f,.86f) };
        static readonly Vector2[] DaggerBlade = { new(.39f,.47f), new(.58f,.43f), new(.66f,.68f), new(.59f,.91f), new(.52f,.67f) };
        static readonly Vector2[] MaceHead = { new(.46f,.65f), new(.49f,.82f), new(.65f,.94f), new(.85f,.80f), new(.80f,.60f), new(.64f,.55f) };
        static readonly Vector2[] ShieldEdge = { new(.19f,.81f), new(.50f,.90f), new(.81f,.81f), new(.77f,.43f), new(.65f,.24f), new(.50f,.12f), new(.35f,.24f), new(.23f,.43f) };
        static readonly Vector2[] CrownEdge = { new(.23f,.27f), new(.15f,.73f), new(.35f,.58f), new(.50f,.87f), new(.65f,.58f), new(.85f,.73f), new(.77f,.27f) };
        static readonly Vector2[] MaskEdge = { new(.12f,.67f), new(.29f,.74f), new(.50f,.65f), new(.71f,.74f), new(.88f,.67f), new(.79f,.37f), new(.61f,.29f), new(.50f,.43f), new(.39f,.29f), new(.21f,.37f) };
        static readonly Vector2[] ArmorEdge = { new(.25f,.86f), new(.39f,.83f), new(.42f,.70f), new(.58f,.70f), new(.61f,.83f), new(.75f,.86f), new(.87f,.66f), new(.73f,.55f), new(.71f,.18f), new(.50f,.12f), new(.29f,.18f), new(.27f,.55f), new(.13f,.66f) };
        static readonly Vector2[] CloakEdge = { new(.50f,.90f), new(.69f,.70f), new(.87f,.17f), new(.50f,.10f), new(.13f,.17f), new(.31f,.70f) };
        static readonly Vector2[] BootFront = { new(.46f,.80f), new(.73f,.80f), new(.70f,.43f), new(.87f,.30f), new(.88f,.15f), new(.43f,.15f), new(.40f,.27f) };
        static readonly Vector2[] BootBack = { new(.17f,.88f), new(.36f,.88f), new(.34f,.53f), new(.37f,.43f), new(.31f,.35f), new(.12f,.35f), new(.12f,.46f) };
        static readonly Vector2[] BowArc = BowPoints();
        static readonly Vector2[] HelmetEdge = { new(.21f,.24f), new(.20f,.61f), new(.29f,.79f), new(.50f,.89f), new(.71f,.79f), new(.80f,.61f), new(.79f,.24f), new(.64f,.14f), new(.64f,.48f), new(.36f,.48f), new(.36f,.14f) };
        static readonly Vector2[] GloveEdge = { new(.32f,.13f), new(.24f,.39f), new(.12f,.53f), new(.17f,.62f), new(.31f,.52f), new(.31f,.78f), new(.39f,.82f), new(.40f,.56f), new(.43f,.89f), new(.51f,.90f), new(.52f,.57f), new(.56f,.85f), new(.64f,.83f), new(.63f,.55f), new(.69f,.74f), new(.77f,.70f), new(.74f,.42f), new(.64f,.13f) };
        static readonly Vector2[] NecklaceChain = { new(.18f,.81f), new(.23f,.56f), new(.34f,.40f), new(.50f,.32f), new(.66f,.40f), new(.77f,.56f), new(.82f,.81f) };

        static Vector2[] BowPoints()
        {
            var points = new Vector2[17];
            for (int i = 0; i < points.Length; i++)
            {
                float a = i * Mathf.PI / (points.Length - 1);
                points[i] = new Vector2(.29f + .35f * Mathf.Sin(a), .50f + .38f * Mathf.Cos(a));
            }
            return points;
        }

        protected override void OnPopulateMesh(VertexHelper mesh)
        {
            mesh.Clear();
            var rect = rectTransform.rect;
            _unit = Mathf.Min(rect.width, rect.height);
            if (_unit <= 0) return;
            _mesh = mesh; _center = rect.center; _ink = color;
            if (_artwork) _ink = new Color(1, 1, 1, color.a);
            if (_symbol != null)
            {
                SymbolQuad(rect);
                _mesh = null;
                return;
            }
            switch (_shape)
            {
                case "Helmet":
                    Stroke(HelmetEdge,.048f);
                    Line(.50f,.84f,.50f,.60f,.038f);
                    Line(.28f,.59f,.72f,.59f,.045f);
                    break;
                case "Gloves":
                    Stroke(GloveEdge,.040f);
                    Line(.32f,.25f,.68f,.25f,.043f);
                    break;
                case "Earring":
                    Ring(.29f,.74f,.10f,.13f,.037f);
                    Ring(.71f,.74f,.10f,.13f,.037f);
                    Line(.29f,.62f,.29f,.43f,.038f);
                    Line(.71f,.62f,.71f,.43f,.038f);
                    Diamond(.29f,.30f,.12f,.17f);
                    Diamond(.71f,.30f,.12f,.17f);
                    break;
                case "Necklace":
                    Stroke(NecklaceChain,.041f,false);
                    Diamond(.50f,.24f,.10f,.14f);
                    break;
                case "Bracer":
                    Ring(.50f,.76f,.25f,.10f,.043f);
                    Ring(.50f,.22f,.19f,.08f,.043f);
                    Line(.25f,.76f,.31f,.22f,.043f);
                    Line(.75f,.76f,.69f,.22f,.043f);
                    Diamond(.50f,.48f,.075f,.11f);
                    break;
                case "Sword":
                    Fill(SwordBlade);
                    Line(.29f,.61f,.61f,.29f,.065f);
                    Line(.38f,.40f,.22f,.23f,.075f);
                    Diamond(.18f,.18f,.075f,.075f);
                    break;
                case "Dagger":
                    Fill(DaggerBlade);
                    Line(.29f,.43f,.68f,.36f,.063f);
                    Line(.46f,.35f,.42f,.16f,.073f);
                    Line(.34f,.13f,.50f,.10f,.060f);
                    break;
                case "Staff":
                    Line(.27f,.12f,.61f,.70f,.055f);
                    Ring(.66f,.79f,.12f,.12f,.045f);
                    Diamond(.66f,.79f,.057f,.072f);
                    Line(.46f,.58f,.58f,.51f,.038f);
                    Line(.23f,.11f,.33f,.07f,.038f);
                    break;
                case "Bow":
                    Stroke(BowArc, .055f, false);
                    Line(.29f,.88f,.29f,.12f,.026f);
                    Line(.13f,.50f,.85f,.50f,.036f);
                    Triangle(new(.88f,.50f),new(.75f,.59f),new(.75f,.41f));
                    break;
                case "Mace":
                    Line(.24f,.13f,.59f,.63f,.065f);
                    Stroke(MaceHead,.050f);
                    Line(.53f,.79f,.75f,.63f,.043f);
                    Line(.57f,.63f,.73f,.85f,.043f);
                    Line(.19f,.16f,.29f,.09f,.053f);
                    break;
                case "Shield":
                    Stroke(ShieldEdge,.055f);
                    Line(.50f,.77f,.50f,.29f,.046f);
                    Line(.34f,.61f,.50f,.49f,.040f);
                    Line(.66f,.61f,.50f,.49f,.040f);
                    break;
                case "Crown":
                    Stroke(CrownEdge,.050f);
                    Line(.25f,.38f,.75f,.38f,.036f);
                    Diamond(.50f,.52f,.055f,.070f);
                    break;
                case "Mask":
                    Stroke(MaskEdge,.046f);
                    Line(.25f,.55f,.39f,.52f,.050f);
                    Line(.61f,.52f,.75f,.55f,.050f);
                    break;
                case "Aura":
                    Ring(.50f,.48f,.35f,.17f,.037f);
                    Ring(.50f,.48f,.23f,.31f,.027f);
                    Diamond(.50f,.48f,.075f,.125f);
                    Diamond(.82f,.75f,.045f,.070f);
                    Diamond(.18f,.24f,.035f,.055f);
                    break;
                case "Armor":
                    Stroke(ArmorEdge,.046f);
                    Line(.35f,.55f,.50f,.47f,.038f);
                    Line(.50f,.47f,.65f,.55f,.038f);
                    Line(.33f,.29f,.67f,.29f,.036f);
                    break;
                case "Cloak":
                    Stroke(CloakEdge,.047f);
                    Ring(.50f,.69f,.105f,.10f,.040f);
                    Line(.39f,.52f,.29f,.21f,.031f);
                    Line(.61f,.52f,.71f,.21f,.031f);
                    break;
                case "Boots":
                    Stroke(BootBack,.037f);
                    Stroke(BootFront,.047f);
                    Line(.46f,.69f,.71f,.69f,.035f);
                    Line(.48f,.26f,.84f,.26f,.035f);
                    break;
                case "Ring":
                    Ring(.50f,.38f,.235f,.235f,.055f);
                    Diamond(.50f,.77f,.15f,.15f);
                    Line(.36f,.65f,.31f,.57f,.045f);
                    Line(.64f,.65f,.69f,.57f,.045f);
                    break;
                default:
                    Ring(.50f,.50f,.30f,.30f,.045f);
                    Diamond(.50f,.50f,.12f,.18f);
                    break;
            }
            _mesh = null;
        }

        void SymbolQuad(Rect rect)
        {
            // Use the symbol PNG's own transparent margin without adding a second inset;
            // preserve its aspect ratio and native alpha within the gear cell.
            var size = new Vector2(_symbol.width * _uv.width, _symbol.height * _uv.height);
            float scale = Mathf.Min(rect.width / size.x, rect.height / size.y);
            var half = size * (scale * .5f);
            var lo = rect.center - half; var hi = rect.center + half;
            _mesh.AddVert(new Vector2(lo.x, lo.y), _ink, new Vector2(_uv.xMin, _uv.yMin));
            _mesh.AddVert(new Vector2(lo.x, hi.y), _ink, new Vector2(_uv.xMin, _uv.yMax));
            _mesh.AddVert(new Vector2(hi.x, hi.y), _ink, new Vector2(_uv.xMax, _uv.yMax));
            _mesh.AddVert(new Vector2(hi.x, lo.y), _ink, new Vector2(_uv.xMax, _uv.yMin));
            _mesh.AddTriangle(0, 1, 2); _mesh.AddTriangle(0, 2, 3);
        }

        Vector2 Point(Vector2 p) => _center + (p - new Vector2(.5f,.5f)) * _unit;
        void Vertex(Vector2 p) => _mesh.AddVert(Point(p), _ink, Vector2.zero);
        static Vector2 Normal(Vector2 d) => new Vector2(-d.y,d.x).normalized;

        void Line(float x, float y, float r, float t, float width)
        {
            var a = new Vector2(x,y); var b = new Vector2(r,t);
            var n = Normal(b-a) * (width*.5f);
            int first = _mesh.currentVertCount;
            Vertex(a-n); Vertex(a+n); Vertex(b+n); Vertex(b-n);
            _mesh.AddTriangle(first,first+1,first+2); _mesh.AddTriangle(first,first+2,first+3);
        }

        // One joined strip avoids darker alpha overlaps at each corner of an empty-slot outline.
        void Stroke(Vector2[] points, float width, bool closed = true)
        {
            int first = _mesh.currentVertCount, count = points.Length;
            for (int i = 0; i < count; i++)
            {
                var prev = Normal(points[i] - points[(i+count-1)%count]);
                var next = Normal(points[(i+1)%count] - points[i]);
                Vector2 offset;
                if (!closed && i == 0) offset = next * (width*.5f);
                else if (!closed && i == count-1) offset = prev * (width*.5f);
                else
                {
                    var join = (prev+next).normalized;
                    offset = join * (width*.5f / Mathf.Max(.4f,Vector2.Dot(join,next)));
                }
                Vertex(points[i]-offset); Vertex(points[i]+offset);
            }
            int segments = closed ? count : count-1;
            for (int i = 0; i < segments; i++)
            {
                int a = first+i*2, b = first+((i+1)%count)*2;
                _mesh.AddTriangle(a,a+1,b+1); _mesh.AddTriangle(a,b+1,b);
            }
        }

        void Ring(float x, float y, float rx, float ry, float width)
        {
            const int count = 32;
            int first = _mesh.currentVertCount;
            for (int i = 0; i < count; i++)
            {
                float a = i * (Mathf.PI*2/count), c = Mathf.Cos(a), s = Mathf.Sin(a);
                Vertex(new Vector2(x+c*(rx+width*.5f), y+s*(ry+width*.5f)));
                Vertex(new Vector2(x+c*(rx-width*.5f), y+s*(ry-width*.5f)));
            }
            for (int i = 0; i < count; i++)
            {
                int a = first+i*2, b = first+((i+1)%count)*2;
                _mesh.AddTriangle(a,b,a+1); _mesh.AddTriangle(a+1,b,b+1);
            }
        }

        void Diamond(float x, float y, float rx, float ry)
        {
            var top = new Vector2(x,y+ry); var bottom = new Vector2(x,y-ry);
            Triangle(top,new Vector2(x+rx,y),bottom);
            Triangle(top,bottom,new Vector2(x-rx,y));
        }
        void Triangle(Vector2 a, Vector2 b, Vector2 c)
        {
            int first = _mesh.currentVertCount;
            Vertex(a); Vertex(b); Vertex(c); _mesh.AddTriangle(first,first+1,first+2);
        }
        void Fill(Vector2[] points)
        {
            int first = _mesh.currentVertCount;
            foreach (var p in points) Vertex(p);
            for (int i = 1; i < points.Length-1; i++) _mesh.AddTriangle(first,first+i,first+i+1);
        }
    }
}
