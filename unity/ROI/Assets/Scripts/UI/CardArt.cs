// THE CARD FACE — an element-coloured frame with the monster it depicts inside it. Ported from
// views/card_render.gd, which the Cards panel and the Album both paint through so a card looks
// identical in either.
//
// Monster clips use each creature's authored geometry in arena_tables.json, including bespoke
// sheets. Applying the slime's frame pitch to every sheet clips larger monsters into fragments.
//
// STATIC and cache-backed: a screen full of cards would otherwise decode the same atlas per cell.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.UI
{
    public static class CardArt
    {
        const string FramesFile = "card_frames.png";
        /// 96 px per cell — the 880 px atlas is 9x96 plus 16 px of tail padding. Dividing 880 by 9
        /// sampled a creeping ~1.8 px per frame, and the late-element frames rendered visibly
        /// offset. The number is the cell pitch, not the atlas width over the count.
        const float FrameW = 96f;

        /// Element -> frame index, the nine frame colours.
        static readonly Dictionary<string, int> FrameByElement = new()
        {
            ["Earth"] = 0, ["Fire"] = 1, ["Poison"] = 2, ["Water"] = 3, ["Dark"] = 4, ["Shadow"] = 4,
            ["Thunder"] = 5, ["Wind"] = 6, ["Light"] = 7, ["Holy"] = 7, ["Neutral"] = 8, ["Physical"] = 8,
            // Spirit reuses the neutral frame until there is bespoke art.
            ["Spirit"] = 8,
        };

        /// Card def -> the monster render key it depicts; geometry and clips come from the arena.
        static readonly Dictionary<string, string> CardSprite = new()
        {
            ["card_pinkling"] = "slime_pink", ["card_greenling"] = "slime_green",
            ["card_yellowling"] = "slime_gold", ["card_chomb"] = "chomb",
            ["card_toxic_chom"] = "toxic_chom", ["card_burst_chom"] = "burst_chom",
            ["card_rechum"] = "rechum", ["card_zombie"] = "zombie",
            ["card_twinling"] = "mini_ghost", ["card_gloomtooth"] = "ghost_rabbit",
            ["card_banshee"] = "banshee", ["card_dustelle"] = "maid",
            ["card_ashelle"] = "maid_horn", ["card_marionette"] = "marionette",
            ["card_scarlette"] = "red_hood", ["card_crimsonne"] = "red_hood_crimson",
            ["card_brutus"] = "werewolf", ["card_gnarloak"] = "demon_tree",
            ["card_broomhilda"] = "witch",
        };

        static Texture2D _frames;
        static readonly Dictionary<string, Sprite> _frameSprites = new();
        static readonly Dictionary<string, Texture2D> _sheets = new();
        static readonly Dictionary<string, Sprite[]> _idle = new();
        static Roi.Arena.ArenaView.Tables _tab;

        /// The frame for an element, or null when the atlas is not in StreamingAssets — in which
        /// case the caller keeps its plain themed cell rather than drawing nothing.
        public static Sprite Frame(string element)
        {
            var key = element ?? "Neutral";
            if (_frameSprites.TryGetValue(key, out var cached)) return cached;
            if (_frames == null)
            {
                _frames = Load(FramesFile);
                if (_frames == null) { _frameSprites[key] = null; return null; }
            }
            int fi = FrameByElement.TryGetValue(key, out var v) ? v : 8;
            // The 80x128 window inside the 96-wide cell, inset 16 — the frame art has padding the
            // card does not want to draw.
            var rect = new Rect(fi * FrameW + 16f, 16f, 80f, 128f);
            var spr = Sprite.Create(_frames, rect, new Vector2(0.5f, 0.5f), 100f);
            spr.name = "frame_" + key;
            _frameSprites[key] = spr;
            return spr;
        }

        /// The monster's IDLE frames for a card, or null when this client has no sheet for it.
        public static Sprite[] Idle(ItemView card)
        {
            var defId = card != null ? card.def_id : null;
            if (string.IsNullOrEmpty(defId)) return null;
            if (!CardSprite.TryGetValue(defId, out var sprite)) return null;
            if (_idle.TryGetValue(sprite, out var cached)) return cached;

            _tab ??= Roi.Arena.ArenaView.Tables.Load();
            var sheetFile = _tab.SheetFor(sprite, null);   // a card has no element context; the sprite key or pink
            if (string.IsNullOrEmpty(sheetFile)) { _idle[sprite] = null; return null; }
            if (!_sheets.TryGetValue(sheetFile, out var tex))
            {
                tex = Load(sheetFile);
                _sheets[sheetFile] = tex;
            }
            if (tex == null) { _idle[sprite] = null; return null; }

            // The SAME cut the arena makes: a square frame of `fw`, laid out in `cols` columns,
            // and the idle clip is a list of frame INDEXES into that grid. Reading the numbers
            // rather than assuming a row is what keeps the two in step.
            var m = _tab.GeomFor(sprite);
            var frames = m.idle_frames != null && m.idle_frames.Length > 0 ? m.idle_frames : new[] { 0 };
            var outp = new Sprite[frames.Length];
            for (int i = 0; i < frames.Length; i++)
            {
                int f = frames[i];
                int cx = f % Mathf.Max(1, m.cols), cy = f / Mathf.Max(1, m.cols);
                // Unity's texture origin is BOTTOM-left and the sheet is laid out from the top.
                var r = new Rect(cx * m.fw, tex.height - (cy + 1) * m.fw, m.fw, m.fw);
                outp[i] = Sprite.Create(tex, r, new Vector2(0.5f, 0.5f), 100f);
                outp[i].name = $"{sprite}_idle{i}";
            }
            _idle[sprite] = outp;
            return outp;
        }

        /// Frames per second for the idle clip, so a caller ticks it at the sim's own cadence.
        public static float IdleFps
        {
            get { _tab ??= Roi.Arena.ArenaView.Tables.Load(); return Mathf.Max(1f, _tab.monster.idle_fps); }
        }

        public static float IdleFpsFor(ItemView card)
        {
            _tab ??= Roi.Arena.ArenaView.Tables.Load();
            return card != null && CardSprite.TryGetValue(card.def_id, out var sprite)
                ? Mathf.Clamp(_tab.GeomFor(sprite).idle_fps, 1, 12) : IdleFps;
        }

        static Texture2D Load(string file)
        {
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "art", file);
            if (!System.IO.File.Exists(path)) return null;
            var tex = new Texture2D(2, 2, TextureFormat.RGBA32, false)
            {
                filterMode = FilterMode.Point,      // pixel art: never smoothed
                wrapMode = TextureWrapMode.Clamp,
            };
            return tex.LoadImage(System.IO.File.ReadAllBytes(path)) ? tex : null;
        }
    }
}
