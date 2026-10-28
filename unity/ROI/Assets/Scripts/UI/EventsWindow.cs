// THE EVENT BOARD — every login ladder the server is running, and one button that takes what they
// owe. Ported from `event_board.gd`.
//
// A LIST, not one ladder. Several tracks run at once by design (a permanent newbie ladder plus
// whatever event is live), so a board that could show only one would be wrong on the day a second
// opens. Nearest deadline first, because that is the only ordering a player can act on; a track
// with no deadline sorts last.
//
// EVERYTHING IS SERVER TRUTH, including the countdown: it runs off the server's `closes_at_ms`
// against its own `now_ms`, so a client whose clock drifts cannot disagree with the server about
// when an event ends.
//
// Collecting writes no sim state and no InputEvent: Crystal goes to the bound bucket and items and
// Gil arrive as MAIL, which the mailbox screen then applies to the save. That is the one road by
// which server-authored value enters a replayed inventory.
//
// WHERE IT LIVES. Godot puts this in the HOME mode, which this client has no equivalent of, so it
// is a Menu entry with no hotkey — the bar's thirty letters are full (`hud_bar.gd` says so).
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class EventsWindow : MonoBehaviour
    {
        const float RowH = 40f, CellH = 86f, Gap = 12f;

        RectTransform _content, _footer;
        TextMeshProUGUI _status;
        readonly List<GameObject> _built = new();
        string _json = "";
        string _flash;
        bool _busy;

        public static EventsWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<EventsWindow>();
            w.Build(body);
            return w;
        }

        void OnEnable() => Roi.Net.OnlineService.Instance.Changed += OnNet;
        void OnDisable()
        {
            if (Roi.Net.OnlineService.Instance != null)
                Roi.Net.OnlineService.Instance.Changed -= OnNet;
        }
        void OnNet() { if (isActiveAndEnabled) Refresh(); }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            PeriodWindow.ActivityRail(body,"Events");
            _footer=ExemplarUi.Rect(body,"EventActions",.115f,0,.97f,0);_footer.sizeDelta=new Vector2(0,84);_footer.pivot=new Vector2(.5f,0);_footer.anchoredPosition=new Vector2(0,12);
            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.anchorMin=new Vector2(.115f,1);sr.offsetMin = new Vector2(0, -40f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.anchorMin=new Vector2(.115f,0);hrt.offsetMin = new Vector2(0,112);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn)
            {
                _status.text = "";
                Clear();
                var notice=MenuNotice.Create(_content,"Events",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_EVENTS"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            net.Run(net.Client.ClaimState((ok, json) =>
            {
                if (!ok) { _status.text = "board failed: " + json; return; }
                _json = json;
                Rebuild();
            }));
        }

        void Rebuild()
        {
            Clear();
            var st = JsonUtility.FromJson<Board>(_json) ?? new Board();
            var tracks = new List<Track>(st.tracks ?? new Track[0]);
            // Nearest deadline first; a track that never closes sorts last.
            tracks.Sort((a, b) =>
            {
                long ax = a.closes_at_ms > 0 ? a.closes_at_ms : long.MaxValue;
                long bx = b.closes_at_ms > 0 ? b.closes_at_ms : long.MaxValue;
                return ax.CompareTo(bx);
            });

            int owed = 0;
            foreach (var t in tracks) owed += t.claimable;
            _status.text = Take() ?? (tracks.Count == 0
                ? "No events are running."
                : owed > 0 ? $"{owed} reward{(owed == 1 ? "" : "s")} waiting"
                           : "Nothing to collect yet.");

            float y = 0f;
            var collect = HudBar.MakeButton(_footer, Loc.Get("UI_EVENTS_COLLECT"), "", 300f, UiLayout.Touch, Collect);
            collect.interactable = !_busy && owed > 0;
            var ct = collect.GetComponentInChildren<TextMeshProUGUI>();
            ct.text = owed > 0 ? $"Collect {owed}" : "Nothing to collect";
            if (owed == 0) ct.color = UiTheme.TextLow;
            var action=(RectTransform)collect.transform;action.anchorMin=new Vector2(.75f,.15f);action.anchorMax=new Vector2(1,.85f);action.offsetMin=action.offsetMax=Vector2.zero;
            _built.Add(collect.gameObject);


            foreach (var t in tracks)
            {
                var head = UiTheme.Label(_content, "T_" + t.track,
                    t.closes_at_ms > 0
                        ? $"{Pretty(t.track)}   ·   day {t.position}   ·   closes in {Span(t.closes_at_ms - st.now_ms)}"
                        : $"{Pretty(t.track)}   ·   day {t.position}",
                    UiTheme.HeadSize, UiTheme.TextHigh, TextAlignmentOptions.Left);
                Place(head.rectTransform, y, RowH);
                _built.Add(head.gameObject);
                y += RowH;

                foreach (var c in t.cells ?? new Cell[0])
                {
                    bool ready = c.state == "claimable";
                    bool got = c.state == "claimed";
                    var row=UiTheme.Box(_content,$"C_{t.track}_{c.id}",new Color(.43f,.52f,.55f,.85f));
                    UiTheme.Border(row.rectTransform,ready?UiTheme.Accent:UiTheme.LineStrong);
                    Place(row.rectTransform,y,CellH);_built.Add(row.gameObject);
                    ExemplarUi.Icon(row.transform,"Events",.025f,.20f,.09f,.80f,ready||got?UiTheme.Accent:UiTheme.TextHigh);
                    var what=ExemplarUi.Text(row.transform,"Rewards",Rewards(c.reward),.12f,.40f,.83f,.90f,UiTheme.BodySize,UiTheme.TextHigh);
                    what.enableAutoSizing=true;what.fontSizeMin=12;what.fontSizeMax=UiTheme.BodySize;
                    ExemplarUi.Text(row.transform,"Day",$"Day {c.at}",.12f,.08f,.83f,.38f,UiTheme.CaptionSize,UiTheme.TextMid);
                    ActivityStatus.Add(row.transform,.87f,.15f,.97f,.85f,got,ready);
                    y += CellH + Gap;
                }
                y += UiTheme.Pad;
            }

            var note = UiTheme.Label(_content, "Note",
                $"{GameText.Crystal} is credited to your account; items and Gil arrive in your mailbox.",
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            Place(note.rectTransform, y, 24f);
            _built.Add(note.gameObject);
            UiScroll.Fit(_content, y + 24f + UiTheme.Pad);
        }

        void Collect()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (_busy || !net.LoggedIn) return;
            _busy = true;
            _status.text = Loc.Get("UI_EVENTS_COLLECTING");
            net.Run(net.Client.ClaimCollect((ok, body) =>
            {
                _busy = false;
                if (!ok) { _flash = "refused: " + body; Refresh(); return; }
                var crystal = LongOf(body, "crystal");
                _flash = crystal > 0
                    ? $"collected — {crystal:N0} {GameText.Crystal}, and the rest is in your mailbox"
                    : "collected — the rewards are in your mailbox";
                Refresh();
            }));
        }

        /// One line for a cell's rewards. The server already enriched each with a name, so this
        /// only formats — an item's name comes from the catalog it was checked against.
        static string Rewards(Grant[] gs)
        {
            if (gs == null || gs.Length == 0) return "—";
            var parts = new List<string>();
            foreach (var g in gs)
                parts.Add(g.kind switch
                {
                    "crystal" => $"{g.qty:N0} {GameText.Crystal}",
                    "gil" => $"{g.qty:N0} Gil",
                    "bound" => $"{g.qty:N0} {GameText.BoundKind(g.bound_kind)}",
                    _ => g.qty > 1 ? $"{Name(g)} ×{g.qty}" : Name(g),
                });
            return string.Join("  ·  ", parts);
        }

        static string Name(Grant g) => string.IsNullOrEmpty(g.name) ? g.def_id : g.name;

        /// A track id as a heading. Until there is a loc layer this is the id made readable.
        static string Pretty(string id)
            => string.IsNullOrEmpty(id) ? "Event"
             : char.ToUpperInvariant(id[0]) + id.Substring(1).Replace('_', ' ');

        static string Span(long ms)
        {
            if (ms <= 0) return "now";
            long s = ms / 1000, h = s / 3600, m = (s % 3600) / 60;
            return h >= 24 ? $"{h / 24}d {h % 24}h" : h > 0 ? $"{h}h {m}m" : $"{m}m";
        }

        static long LongOf(string json, string key)
        {
            var m = System.Text.RegularExpressions.Regex.Match(json, "\"" + key + "\":(\\d+)");
            return m.Success ? long.Parse(m.Groups[1].Value) : 0;
        }

        string Take() { var f = _flash; _flash = null; return f; }

        static void Inset(TextMeshProUGUI t, float left, float right)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, 0f); rt.offsetMax = new Vector2(-right, 0f);
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

        [System.Serializable] class Grant { public string kind, def_id, name, rarity, bound_kind; public long qty; }
        [System.Serializable] class Cell { public int id, at; public string state; public Grant[] reward; }
        [System.Serializable]
        class Track { public string track; public int position, claimable; public long closes_at_ms; public Cell[] cells; }
        [System.Serializable] class Board { public bool ok; public long now_ms, day; public Track[] tracks; }
    }
}
