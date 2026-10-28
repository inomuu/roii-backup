using System;
using System.Collections.Generic;
using UnityEngine;

namespace Roi.UI
{
    // Presentation-only mapping. Equipment Tier comes from catalog identity, never grade/level.
    // Legacy emission groups (mask/garment/pendant) do NOT describe the current gear slots.
    public static class EquipmentArtwork
    {
        static readonly string[] Kinds = { "sword", "greatsword", "dagger", "bow", "rod",
            "mace", "shield", "armor", "helm", "gloves", "shoes", "earring", "ring", "necklace", "bracer" };
        static readonly Dictionary<string, Vector2Int> Items = BuildLookup();
        static readonly Texture2D[] Atlases = new Texture2D[7];
        static readonly bool[] Loaded = new bool[7];
        // Alpha-trimmed pixel rectangles, bottom-left origin on the approved 1619 x 971 sheets.
        // Per-item rectangles intentionally differ from an equal grid; this prevents clipped
        // tips and neighbors bleeding into a cell. Keep the original alpha and aspect ratio.
        static readonly Rect[][] Bounds = {
            // T1
            new Rect[] { new(38,647,235,281), new(367,646,249,293), new(716,672,213,213), new(1037,646,235,290), new(1362,640,236,302), new(42,345,240,273), new(348,347,266,273), new(670,335,287,289), new(1030,355,232,246), new(1323,367,281,221), new(29,31,279,277), new(391,78,210,179), new(716,82,181,183), new(1064,40,190,285), new(1370,63,190,214) },
            // T2
            new Rect[] { new(52,646,237,313), new(360,630,256,335), new(760,648,122,271), new(1084,631,206,330), new(1397,638,167,328), new(59,313,205,315), new(393,317,169,302), new(660,321,301,295), new(1039,331,200,285), new(1312,346,299,241), new(13,10,290,292), new(391,80,186,163), new(709,55,202,209), new(1043,19,207,293), new(1398,27,167,264) },
            // T3
            new Rect[] { new(104,598,131,348), new(408,596,134,369), new(755,608,110,317), new(1096,593,173,360), new(1407,595,128,363), new(107,281,109,307), new(373,280,195,311), new(643,279,328,293), new(1050,281,231,295), new(1345,300,230,258), new(36,22,230,247), new(383,50,175,194), new(715,98,180,100), new(1033,22,221,250), new(1373,12,202,261) },
            // T4
            new Rect[] { new(64,616,218,341), new(384,615,233,348), new(704,631,182,277), new(1043,612,210,352), new(1396,613,169,350), new(61,294,197,311), new(384,281,192,334), new(676,280,269,327), new(1037,306,212,284), new(1328,319,276,253), new(23,12,284,264), new(399,19,183,256), new(722,68,185,149), new(1060,19,159,264), new(1342,37,234,207) },
            // T5
            new Rect[] { new(50,609,241,348), new(333,609,290,357), new(693,636,265,263), new(980,618,235,348), new(1353,604,242,363), new(67,297,165,312), new(364,301,240,295), new(664,292,284,311), new(1017,317,226,252), new(1326,310,266,273), new(27,13,312,262), new(396,44,235,226), new(712,41,209,200), new(1027,23,213,284), new(1352,13,233,265) },
            // T6
            new Rect[] { new(35,634,272,319), new(350,611,274,354), new(717,640,193,269), new(1079,624,204,335), new(1341,626,267,339), new(18,328,275,289), new(347,327,280,282), new(659,312,302,307), new(1042,323,177,299), new(1329,337,270,251), new(19,11,277,300), new(368,44,253,245), new(695,51,237,219), new(1034,13,199,309), new(1349,18,243,280) },
            // T7
            new Rect[] { new(61,647,203,302), new(390,637,199,321), new(712,650,202,283), new(1047,609,210,354), new(1370,632,205,321), new(63,326,216,290), new(367,323,245,295), new(681,333,250,263), new(1025,333,238,276), new(1348,352,242,245), new(44,21,224,277), new(387,45,210,234), new(715,53,193,229), new(1031,20,202,288), new(1361,33,212,260) }
        };
        static Dictionary<string, Vector2Int> BuildLookup()
        {
            var items = new Dictionary<string, Vector2Int>(StringComparer.Ordinal);
            for (int tier = 1; tier <= 7; tier++)
                for (int kind = 0; kind < Kinds.Length; kind++)
                    items.Add(Kinds[kind] + "_s" + tier, new Vector2Int(tier - 1, kind));
            items.Add("starting_sword", new Vector2Int(0, 0));
            items.Add("starting_bow", new Vector2Int(0, 3));
            items.Add("starting_dagger", new Vector2Int(0, 2));
            items.Add("starting_staff", new Vector2Int(0, 4));
            items.Add("starting_mace", new Vector2Int(0, 5));
            return items;
        }
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.SubsystemRegistration)]
        static void Reset()
        {
            Array.Clear(Atlases, 0, Atlases.Length);
            Array.Clear(Loaded, 0, Loaded.Length);
        }
        public static bool TryGet(ItemView item, out Texture2D texture, out Rect uv)
        {
            texture = null; uv = new Rect(0, 0, 1, 1);
            if (item == null || string.IsNullOrEmpty(item.def_id) ||
                !Items.TryGetValue(item.def_id, out var key)) return false;
            if (!Loaded[key.x])
            {
                Atlases[key.x] = Resources.Load<Texture2D>("EquipmentArtwork/T" + (key.x + 1) + "-Atlas");
                Loaded[key.x] = true;
            }
            texture = Atlases[key.x];
            if (texture == null) return false;
            var rect = Bounds[key.x][key.y];
            uv = new Rect(rect.x / 1619f, rect.y / 971f, rect.width / 1619f, rect.height / 971f);
            return true;
        }
    }
}
