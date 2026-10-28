// RECORD — everything the party has done since the save began, ported from `stats_view.gd`.
//
// Distinct from two neighbours that sound like it: the DPS meter is a resettable window measuring
// how hard you hit right now, and the Bestiary tallies kills per family. This answers "what have I
// done in total", and nothing else does.
//
// ONE QUERY ON OPEN, not a per-tick read: none of these numbers move at a rate a player can watch,
// so paying for them every frame buys nothing.
//
// THE SCOPE CAVEAT IS PART OF THE SCREEN, not a footnote. These are the SIM's counters; gacha,
// market and store activity happen server-side and are genuinely absent — and a player who counts
// their pulls here and finds nothing would reasonably call it a bug.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class RecordWindow : MonoBehaviour
    {
        const float RowH = 30f, HeadH = 30f;

        /// Section, then rows as (label, json key, format). A table rather than code, so adding a
        /// counter stays a one-line change — the same shape `stats_view.gd` keeps it in.
        static readonly (string Section, (string Label, string Key, string Fmt)[] Rows)[] Sections =
        {
            ("Overall", new[] { ("Time played", "total_ticks", "duration") }),
            ("Combat", new[]
            {
                ("Monsters defeated", "kills", "int"),
                ("Bosses defeated", "boss_kills", "int"),
                ("Rooms cleared", "rooms_cleared", "int"),
                ("Bands cleared", "bands_cleared", "int"),
                ("Damage dealt", "damage_dealt", "int"),
                ("Damage taken", "damage_taken", "int"),
                ("Biggest hit", "biggest_hit", "int"),
                ("Healing done", "healing_done", "int"),
                ("Skills cast", "skills_cast", "int"),
                ("Deaths", "deaths", "int"),
                ("Wipes", "wipes", "int"),
            }),
            ("Building", new[]
            {
                ("Levels gained", "levels_gained", "int"),
                ("Items crafted", "items_crafted", "int"),
                ("Star Stones used", "orbs_used", "int"),
                ("Cards fed", "cards_fed", "int"),
                ("Exemplars fed", "exemplars_fed", "int"),
            }),
            ("Economy", new[]
            {
                ("Gil earned", "gil_earned", "int"),
                ("Gil spent", "gil_spent", "int"),
                ("Gil received", "gil_received", "int"),
                ("Gil sent", "gil_sent", "int"),
                ("Items listed", "items_listed", "int"),
            }),
        };

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _status;
        readonly List<GameObject> _built = new();

        public static RecordWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<RecordWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            _status = UiTheme.Label(body, "Status",
                Loc.Get("UI_RECORD_THE_SIM_S_OWN_COUNTERS"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            Clear();
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            var json = sim.QueryJson("record");

            Canvas.ForceUpdateCanvases();
            int columns=_content.rect.width>=720?2:1;
            float width=(_content.rect.width-(columns-1)*18)/columns;
            var heights=new float[columns];
            foreach(var (section,rows) in Sections) {
                int col=columns==2&&heights[1]<heights[0]?1:0;
                float h=56+rows.Length*36;
                var card=UiTheme.Flat(_content,"Section_"+section,Color.clear);card.raycastTarget=false;
                var cr=card.rectTransform;cr.anchorMin=cr.anchorMax=new Vector2(0,1);cr.pivot=new Vector2(0,1);cr.sizeDelta=new Vector2(width,h);cr.anchoredPosition=new Vector2(col*(width+18),-heights[col]);
                GameMenuSurface.Add(cr);_built.Add(card.gameObject);
                var title=UiTheme.Label(cr,"Heading",section,18,UiTheme.Accent,TextAlignmentOptions.Left);
                Place(title.rectTransform,10,32);title.rectTransform.sizeDelta=new Vector2(-32,32);
                for(int i=0;i<rows.Length;i++) {
                    var (label,key,fmt)=rows[i];float y=48+i*36;
                    var name=UiTheme.Label(cr,"L_"+key,label,15,UiTheme.TextMid,TextAlignmentOptions.Left);
                    Place(name.rectTransform,y,32);name.rectTransform.sizeDelta=new Vector2(-32,32);
                    name.rectTransform.anchorMax=new Vector2(.62f,1);name.rectTransform.offsetMin=new Vector2(16,-y-32);name.rectTransform.offsetMax=new Vector2(0,-y);
                    var val=UiTheme.Label(cr,"V_"+key,Format(json,key,fmt),17,UiTheme.TextHigh,TextAlignmentOptions.Right);
                    Place(val.rectTransform,y,32);val.rectTransform.sizeDelta=new Vector2(-32,32);
                    val.rectTransform.anchorMin=new Vector2(.64f,1);val.rectTransform.offsetMin=new Vector2(0,-y-32);val.rectTransform.offsetMax=new Vector2(-16,-y);
                    val.enableAutoSizing=true;val.fontSizeMin=13;val.fontSizeMax=17;
                    var line=UiTheme.Flat(cr,"Rule",UiTheme.Line);line.raycastTarget=false;Place(line.rectTransform,y+33,1);line.rectTransform.sizeDelta=new Vector2(-32,1);
                }
                heights[col]+=h+18;
            }
            UiScroll.Fit(_content,Mathf.Max(heights));
        }

        static string Format(string json, string key, string fmt)
        {
            long v = LongOf(json, key);
            switch (fmt)
            {
                case "duration":
                    // Ticks are the sim's clock: 10 to the second, everywhere.
                    long s = v / 10, h = s / 3600, m = (s % 3600) / 60;
                    return h > 0 ? $"{h}h {m}m" : $"{m}m {s % 60}s";
                default:
                    return v.ToString("N0");
            }
        }

        static long LongOf(string json, string key)
        {
            var m = System.Text.RegularExpressions.Regex.Match(json, "\"" + key + "\":(-?\\d+)");
            return m.Success ? long.Parse(m.Groups[1].Value) : 0;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Clear()
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }
    }
}
