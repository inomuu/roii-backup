// THE WORLD MAP — the twenty regions as a clickable grid. Ported from
// godot/views/world_map_view.gd, whose tile positions and biome tints are hand-authored tables and
// are copied here value for value; they are the placeholder "art" and not something to re-derive.
//
// A region is FIVE bands. `select_band` takes the first band of the region, which is why the tile
// sends `(region - 1) * 5 + 1` and shows `Lv (region-1)*5+1 .. region*5` — the same arithmetic the
// original does, and the same constant the sim owns (`state::ZONES_PER_REGION`). The Godot file
// carries a comment saying that mirroring has broken once already; it is written out here rather
// than hidden in a helper so the next person sees the number they have to keep in step.
//
// Region NAMES come from the localisation table's `ZONE_NAME_b<n>` rows, transcribed. There is no
// loc layer on this side yet, and "Region 7" would have been a worse placeholder than the name the
// game already ships.
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class WorldMapWindow : MonoBehaviour
    {
        /// Mirrors `state::ZONES_PER_REGION` — twelve STAGES to a region (0.0.398).
        const int ZonesPerRegion = 12;
        const int MaxRegion = 20, Cols = 7, Rows = 4;
        const float TileW = 118f, TileH = 118f, TileGap = 8f;

        // BAND_POS — the authored grid position of each region.
        static readonly Vector2Int[] Pos =
        {
            new(0,2), new(1,2), new(1,1), new(2,1), new(2,2), new(3,2), new(2,3),
            new(3,3), new(4,3), new(5,3), new(4,2), new(4,1), new(3,0), new(4,0),
            new(5,0), new(5,1), new(6,1), new(5,2), new(6,2), new(6,0),
        };

        // BAND_TINTS — the biome colour. Gothic arc is regions 5-10.
        static readonly Color[] Tint =
        {
            new(0.28f,0.44f,0.22f), new(0.30f,0.48f,0.26f), new(0.25f,0.40f,0.20f),
            new(0.48f,0.36f,0.16f), new(0.30f,0.24f,0.38f), new(0.28f,0.22f,0.36f),
            new(0.26f,0.20f,0.34f), new(0.22f,0.20f,0.32f), new(0.20f,0.18f,0.28f),
            new(0.18f,0.14f,0.26f), new(0.55f,0.42f,0.20f), new(0.24f,0.36f,0.48f),
            new(0.28f,0.42f,0.55f), new(0.30f,0.34f,0.46f), new(0.36f,0.40f,0.55f),
            new(0.48f,0.26f,0.14f), new(0.55f,0.22f,0.10f), new(0.20f,0.14f,0.24f),
            new(0.14f,0.10f,0.20f), new(0.45f,0.36f,0.12f),
        };

        static readonly string[] Names =
        {
            "Willowmere Meadows", "Cloverdell", "Chomwood Thicket", "Amberfall Hollow",
            "Duskmourn Outskirts", "Duskmourn Gardens", "Wolfshollow Trail", "Witchmere Marsh",
            "Mourngrave Cemetery", "Duskmourn Manor", "Sunscorch Dunes", "Frostfen Pass",
            "Glacierheart Tundra", "Stormpeak Heights", "Skyshard Summit", "Emberwaste",
            "Magmara Caldera", "Shadowfen Depths", "Voidreach Chasm", "The Vacant Throne",
        };

        Roi.Sim.SimDriver _driver;
        RectTransform _body, _grid;
        readonly System.Collections.Generic.List<GameObject> _tiles = new();
        int _here = -1;

        /// The region a BAND belongs to, named. Exposed so the area selector reads the same table
        /// this window draws — two transcriptions of twenty hand-authored names is two things to
        /// keep in step, and the Godot original already records that the mirroring broke once.
        public static string RegionNameOfBand(int band)
        {
            int region = Mathf.Clamp((band - 1) / ZonesPerRegion + 1, 1, MaxRegion);
            return region - 1 < Names.Length ? Names[region - 1] : "Region " + region;
        }

        public static WorldMapWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<WorldMapWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            _body = body;
            var go = new GameObject("Grid", typeof(RectTransform));
            go.transform.SetParent(body, false);
            _grid = (RectTransform)go.transform;
            _grid.anchorMin = Vector2.zero; _grid.anchorMax = Vector2.one;
            _grid.offsetMin = new Vector2(UiTheme.PadLarge, UiTheme.PadLarge);
            _grid.offsetMax = new Vector2(-UiTheme.PadLarge, -UiTheme.PadLarge);
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;

            int band = BandOf(session);
            int region = band > 0 ? (band - 1) / ZonesPerRegion + 1 : 0;
            if (region == _here && _tiles.Count > 0) return;
            _here = region;
            Rebuild();
        }

        /// "zone_b7_2" -> 7, and 0 for anything that carries no band — a dungeon or an instance.
        /// The Godot side notes this parse had grown four copies before it was given one home; it
        /// gets exactly one here too.
        static int BandOf(Roi.Sim.SimSession session)
        {
            var m = System.Text.RegularExpressions.Regex.Match(
                session.QueryJson("purse"), "\"zone_id\":\"zone_b(\\d+)_");
            return m.Success && int.TryParse(m.Groups[1].Value, out var n) ? n : 0;
        }

        void Rebuild()
        {
            foreach (var t in _tiles) Destroy(t);
            _tiles.Clear();

            // The cluster CENTRES itself in whatever room the window has, rather than sitting in a
            // corner: the authored grid is 7x4 and a window is neither.
            float w = Cols * TileW + (Cols - 1) * TileGap;
            float h = Rows * TileH + (Rows - 1) * TileGap;
            float fit=Mathf.Min(1f,_grid.rect.width/w,_grid.rect.height/h);
            w*=fit;h*=fit;
            float ox = Mathf.Max(0f, (_grid.rect.width - w) * 0.5f);
            float oy = Mathf.Max(0f, (_grid.rect.height - h) * 0.5f);

            for (int i = 0; i < MaxRegion; i++)
            {
                int region = i + 1;
                bool here = region == _here;
                var tint = Tint[i];
                var tile = UiTheme.Box(_grid, "R" + region, here ? UiTheme.SurfaceActive : UiTheme.Surface);
                UiTheme.Border(tile.rectTransform, here ? UiTheme.Accent : UiTheme.Line,
                               8, here ? 2f : 1f);
                var rt = tile.rectTransform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.sizeDelta = new Vector2(TileW*fit, TileH*fit);
                rt.anchoredPosition = new Vector2(ox + Pos[i].x * (TileW + TileGap)*fit,
                                                  -(oy + Pos[i].y * (TileH + TileGap)*fit));

                var biome=UiTheme.Flat(rt,"Biome",tint*1.5f);biome.raycastTarget=false;
                biome.rectTransform.anchorMin=new Vector2(0,1);biome.rectTransform.anchorMax=new Vector2(1,1);biome.rectTransform.pivot=new Vector2(.5f,1);biome.rectTransform.sizeDelta=new Vector2(-16,3);biome.rectTransform.anchoredPosition=new Vector2(0,-5);
                if (!here)
                {
                    var btn = tile.gameObject.AddComponent<UnityEngine.UI.Button>();
                    btn.targetGraphic = tile;
                    btn.navigation = new UnityEngine.UI.Navigation
                    { mode = UnityEngine.UI.Navigation.Mode.None };
                    var colors = btn.colors;
                    colors.highlightedColor = new Color(1.3f, 1.3f, 1.3f, 1f);
                    colors.fadeDuration = 0.08f;
                    btn.colors = colors;
                    int first = (region - 1) * ZonesPerRegion + 1;
                    btn.onClick.AddListener(() => Send(first));
                }

                var name = UiTheme.Label(rt, "Name", Names[i], UiTheme.BodySize*fit,
                                         UiTheme.TextHigh, TextAlignmentOptions.Top);
                name.textWrappingMode = TextWrappingModes.Normal;
                var nr = name.rectTransform;
                nr.anchorMin = Vector2.zero; nr.anchorMax = Vector2.one;
                nr.offsetMin = new Vector2(8f, 34f); nr.offsetMax = new Vector2(-8f, -12f);

                var lv = UiTheme.Label(rt, "Stages",
                                       Loc.Fmt("UI_STAGE_RANGE", (region - 1) * ZonesPerRegion + 1, region * ZonesPerRegion),
                                       UiTheme.CaptionSize,
                                       here ? UiTheme.Accent : UiTheme.TextMid,
                                       TextAlignmentOptions.Bottom);
                var lr = lv.rectTransform;
                lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
                lr.offsetMin = new Vector2(6f, here ? 26f : 10f); lr.offsetMax = new Vector2(-6f, -TileH*fit + 46f);

                if (here)
                {
                    var mark = UiTheme.Caps(rt, "Here", Loc.Get("UI_WORLD_MAP_HERE"), 9f, UiTheme.Accent,
                                            TextAlignmentOptions.Bottom);
                    var mr = mark.rectTransform;
                    mr.anchorMin = Vector2.zero; mr.anchorMax = Vector2.one;
                    mr.offsetMin = new Vector2(6f, 8f); mr.offsetMax = new Vector2(-6f, -TileH*fit + 26f);
                }
                _tiles.Add(tile.gameObject);
            }
        }

        void Send(int band)
        {
            _driver?.Session?.ApplyInput($"{{\"SelectBand\":{{\"band\":{band}}}}}");
            _here = -1;
            Refresh();
        }
    }
}
