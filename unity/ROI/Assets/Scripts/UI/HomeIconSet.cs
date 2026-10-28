using UnityEngine;

namespace Roi.UI
{
    // One generated, alpha-preserving 4 x 3 atlas. Each cell is a reusable UI sprite.
    public static class HomeIconSet
    {
        static Material _material;
        public static Material Material
        {
            get
            {
                if(_material==null)
                {
                    var shader=Resources.Load<Shader>("HomeUI/HomeEmblem");
                    if(shader!=null)_material=new Material(shader){name="Home emblems",hideFlags=HideFlags.HideAndDontSave};
                }
                return _material;
            }
        }
        static Sprite _wolf;
        static readonly Sprite[] Sprites = new Sprite[12];
        // Visible alpha bounds measured in each 362 px source cell (top-left coordinates).
        // Trim the unequal transparent gutters so preserveAspect centers the artwork itself.
        static readonly Vector4[] InkBounds =
        {
            new(38,53,332,356), new(64,53,288,336), new(22,117,329,313), new(3,77,328,311),
            new(38,58,359,299), new(62,46,331,325), new(30,43,303,325), new(11,44,324,320),
            new(48,28,327,283), new(42,19,311,287), new(43,73,309,253), new(38,22,307,287)
        };
        public static Sprite Get(string id)
        {
            if(id=="TartarusWolf")
            {
                if(_wolf!=null)return _wolf;
                var source=Resources.Load<Texture2D>("HomeUI/TartarusWolfEmblem");
                if(source==null)return null;
                _wolf=Sprite.Create(source,new Rect(0,0,source.width,source.height),new Vector2(.5f,.5f),100,0,SpriteMeshType.FullRect);
                _wolf.name="HomeEmblem_TartarusWolf";
                return _wolf;
            }
            int index = id switch
            {
                "Story" => 0, "Mission" => 1, "Exemplars" => 2, "Party" => 3,
                "Excavation" => 4, "Secondary0" => 5, "Secondary1" => 6, "Workshop" => 6,
                "Secondary2" => 7, "Store" => 7, "Secondary3" => 8, "Options" => 9,
                "Mail" => 10, "Combat" => 11, _ => -1
            };
            if(index < 0) return null;
            if(Sprites[index] != null) return Sprites[index];
            var texture = Resources.Load<Texture2D>("HomeUI/MenuEmblems-Clean");
            if(texture == null) return null;
            float cellWidth = texture.width / 4f, cellHeight = texture.height / 3f;
            var ink = InkBounds[index];
            const float padding = 4f, sourceCell = 362f;
            float left = Mathf.Max(0, ink.x-padding), top = Mathf.Max(0, ink.y-padding);
            float right = Mathf.Min(sourceCell, ink.z+padding), bottom = Mathf.Min(sourceCell, ink.w+padding);
            var rect = new Rect(index % 4 * cellWidth + left/sourceCell*cellWidth,
                (2-index/4)*cellHeight + (sourceCell-bottom)/sourceCell*cellHeight,
                (right-left)/sourceCell*cellWidth, (bottom-top)/sourceCell*cellHeight);
            var sprite = Sprite.Create(texture, rect, new Vector2(.5f, .5f), 100, 0, SpriteMeshType.FullRect);
            sprite.name = "HomeEmblem_" + id;
            Sprites[index] = sprite;
            return sprite;
        }
    }
}
