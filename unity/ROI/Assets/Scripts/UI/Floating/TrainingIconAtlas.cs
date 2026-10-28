using System;
using System.Collections.Generic;
using UnityEngine;

namespace Roi.UI
{
    /// White, minimal glyphs for the Training ladder. The source image is a 5x5 atlas;
    /// keeping the slicing here means the UI can use one small texture without importing
    /// 23 separate files or putting stat meaning into the presentation layer.
    public static class TrainingIconAtlas
    {
        const int Columns = 5;
        const int Rows = 5;
        static readonly Dictionary<string, int> Indices = new(StringComparer.Ordinal)
        {
            ["might"] = 0,
            ["vitality"] = 1,
            ["fortitude"] = 2,
            ["precision"] = 3,
            ["lethality"] = 4,
            ["giant_slayer"] = 5,
            ["sprint"] = 6,
            ["acceleration"] = 7,
            ["regeneration"] = 8,
            ["shatter"] = 9,
            ["satchel"] = 10,
            ["attune_neutral"] = 11,
            ["attune_fire"] = 12,
            ["attune_water"] = 13,
            ["attune_thunder"] = 14,
            ["attune_holy"] = 15,
            ["attune_shadow"] = 16,
            ["ward_neutral"] = 17,
            ["ward_fire"] = 18,
            ["ward_water"] = 19,
            ["ward_thunder"] = 20,
            ["ward_holy"] = 21,
            ["ward_shadow"] = 22,
        };

        static readonly Dictionary<string, Sprite> Cache = new(StringComparer.Ordinal);
        static Texture2D _atlas;

        public static Sprite Get(string id)
        {
            if (string.IsNullOrEmpty(id) || !Indices.TryGetValue(id, out var index)) return null;
            if (Cache.TryGetValue(id, out var cached)) return cached;

            _atlas ??= Resources.Load<Texture2D>("TrainingIcons/TrainingIconAtlas");
            if (_atlas == null || _atlas.width < Columns || _atlas.height < Rows) return null;

            int column = index % Columns;
            int row = index / Columns;
            int x0 = Mathf.RoundToInt(_atlas.width * column / (float)Columns);
            int x1 = Mathf.RoundToInt(_atlas.width * (column + 1) / (float)Columns);
            int y0 = Mathf.RoundToInt(_atlas.height * (Rows - row - 1) / (float)Rows);
            int y1 = Mathf.RoundToInt(_atlas.height * (Rows - row) / (float)Rows);
            var rect = new Rect(x0, y0, Mathf.Max(1, x1 - x0), Mathf.Max(1, y1 - y0));
            var sprite = Sprite.Create(_atlas, rect, new Vector2(.5f, .5f), 100f, 0, SpriteMeshType.FullRect);
            sprite.name = "Training_" + id;
            Cache[id] = sprite;
            return sprite;
        }
    }
}
