using System;
using UnityEngine;

namespace Roi.UI
{
    // Generated pure-white symbols for the ten LIVE equipment slots; no retired mask/cloak.
    internal static class GeneratedGearSymbols
    {
        static readonly string[] Shapes = { "Sword", "Shield", "Helmet", "Gloves", "Armor",
            "Boots", "Earring", "Necklace", "Ring", "Bracer" };
        static readonly Rect[] Bounds = { new(96,414,271,338), new(512,414,217,338), new(888,429,207,318), new(1271,423,180,314), new(1626,415,265,321), new(83,55,254,304), new(522,65,192,290), new(871,49,241,326), new(1273,83,206,260), new(1639,67,254,287) };
        static Texture2D _atlas;
        static bool _loaded;
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.SubsystemRegistration)]
        static void Reset() { _atlas = null; _loaded = false; }
        public static bool TryGet(string shape, out Texture2D texture, out Rect uv)
        {
            texture = null; uv = new Rect(0, 0, 1, 1);
            int index = Array.IndexOf(Shapes, shape);
            if (index < 0) return false;
            if (!_loaded) { _atlas = Resources.Load<Texture2D>("EquipmentArtwork/SlotSymbols"); _loaded = true; }
            if (_atlas == null) return false;
            texture = _atlas;
            var rect = Bounds[index];
            uv = new Rect(rect.x / 1983f, rect.y / 793f, rect.width / 1983f, rect.height / 793f);
            return true;
        }
    }
}
