// MAIL — where every server reward lands, and the last link between the server and the save.
//
// The gacha says "delivered to your mail" and it means it literally: `/gacha/roll` mints its
// rewards as mail rows inside the same transaction that debits the wallet. So without this screen
// the reward loop simply stops halfway.
//
// CLAIMING DOES NOT CALL A CLAIM ENDPOINT, and getting that backwards would have shipped a client
// that loses rewards. `/mail/claim` exists but is DEV-GATED, and the server says why on the route
// itself: it "consumes REAL mailbox state out-of-band (no GameState credit) → value loss". It takes
// the mail away and credits nothing.
//
// The production path is the one the same note names: **ClaimMail -> verified save ->
// consume_claim**. The client applies `InputEvent::ClaimMail` to the SIM, built from the inbox row
// it already holds, and the next save is what settles the mail with the server. That is also why
// the event is Server-authored: the verifier reconciles it against the real mail record instead of
// trusting the tag, so a forged one is caught at the save rather than at the click.
//
// The item is SPLICED from the row rather than rebuilt: a full instance carries stats and
// stars, and re-emitting it would be a second serializer to keep in step with the sim's.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class MailWindow : MonoBehaviour
    {
        const float RowH = 62f, Gap = UiTheme.Gap;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _status;
        readonly List<GameObject> _built = new();
        /// The raw JSON of each inbox row, because the claim is built FROM the row and a DTO would
        /// have to re-emit the item instance it carries.
        readonly Dictionary<ulong, string> _rows = new();

        public static MailWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<MailWindow>();
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
            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize,
                                    UiTheme.TextLow, TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.Normal;
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
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn)
            {
                _status.text = "";
                Clear();
                var notice=MenuNotice.Create(_content,"Mail",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_MAIL"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            net.Run(net.Client.MailInbox((ok, json) =>
            {
                if (!ok) { _status.text = "inbox failed: " + json; return; }
                Rebuild(json);
            }));
        }

        void Rebuild(string arrayJson)
        {
            Clear();
            _rows.Clear();
            foreach (var raw in SplitRows(arrayJson))
            {
                var m = System.Text.RegularExpressions.Regex.Match(raw, "\"id\":(\\d+)");
                if (m.Success) _rows[ulong.Parse(m.Groups[1].Value)] = raw;
            }
            var list = JsonUtility.FromJson<Inbox>("{\"items\":" + arrayJson + "}");
            var mail = list != null && list.items != null ? list.items : new Mail[0];
            _status.text = mail.Length == 0
                ? "The mailbox is empty."
                : $"{mail.Length} unclaimed. Claiming applies the reward to your save, not just to the mailbox.";

            float y = 0f;
            foreach (var m in mail)
            {
                ulong id = m.id;
                var row = UiTheme.Box(_content, "M" + m.id, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var name = UiTheme.Label(row.rectTransform, "N", Describe(m), UiTheme.BodySize,
                                         UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad, 200f, 22f, -12f);
                // The REASON is the mint provenance the server records — "gacha char_seraphine".
                // It is the only thing on the row that says where a reward came from.
                var why = UiTheme.Label(row.rectTransform, "W", m.reason, UiTheme.CaptionSize,
                                        UiTheme.TextLow, TextAlignmentOptions.Left);
                Inset(why, UiTheme.Pad, 200f, 10f, -36f);

                // ALREADY APPLIED THIS SESSION? The row STAYS in the inbox until a verified save
                // consumes it — that is the production path working, not a stale list — so the
                // screen has to remember what it has already applied. Without that, a second press
                // puts a second `ClaimMail` for the same id in the input log, the sim pays twice,
                // and the server rejects the whole save as a FORGED market claim: the player loses
                // everything since their last good save for double-tapping a button.
                bool done = _applied.Contains(id);
                var btn = HudBar.MakeButton(row.rectTransform, done ? "Claimed" : "Claim", "",
                                            132f, UiLayout.Touch, () => Claim(id));
                btn.interactable = !done;
                if (done)
                {
                    var label = btn.GetComponentInChildren<TextMeshProUGUI>();
                    if (label != null) label.color = UiTheme.TextLow;
                }
                var br = btn.GetComponent<RectTransform>();
                br.anchorMin = br.anchorMax = new Vector2(1f, 0.5f);
                br.pivot = new Vector2(1f, 0.5f);
                br.anchoredPosition = new Vector2(-UiTheme.PadSmall, 0f);
                y += RowH + Gap;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        /// Apply `ClaimMail` to the SIM from the row we already have. No endpoint is called: the
        /// next save is what settles it, and that is the whole production path.
        void Claim(ulong id)
        {
            if (_applied.Contains(id)) return;      // belt as well as braces; the button is off too
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null || !_rows.TryGetValue(id, out var raw)) return;
            bool ok = ClaimFrom(sim, id, raw);
            if (ok) _applied.Add(id);
            _status.text = ok
                ? "applied to your save — it settles with the server on the next save"
                : "the sim REFUSED the event; the mail is untouched";
            Refresh();
        }

        /// The mail ids this session has already applied to the sim. Session-scoped on purpose: a
        /// verified save consumes the mail server-side, so after a restart the inbox itself is the
        /// record and this set has nothing left to remember.
        readonly System.Collections.Generic.HashSet<ulong> _applied = new();

        /// Builds the event from an inbox ROW. Explicit fields rather than a wholesale splice: a
        /// row carries `id`, `account_id`, `kind` and `reason` that the event has no place for, and
        /// relying on a deserializer to ignore them is relying on a setting nobody chose on purpose.
        public static bool ClaimFrom(Roi.Sim.SimSession sim, ulong id, string rowJson)
        {
            var sb = new System.Text.StringBuilder("{\"ClaimMail\":{\"mail_id\":").Append(id);
            long gil = LongOf(rowJson, "gil"), qty = LongOf(rowJson, "qty");
            var def = StringOf(rowJson, "def_id");
            if (gil > 0) sb.Append(",\"gil\":").Append(gil);
            if (!string.IsNullOrEmpty(def)) sb.Append(",\"def_id\":\"").Append(def).Append('"');
            if (qty > 0) sb.Append(",\"qty\":").Append(qty);
            var item = Roi.Net.ServerClient.InnerObject(rowJson, "\"item\":");
            if (!string.IsNullOrEmpty(item)) sb.Append(",\"item\":{").Append(item).Append('}');
            sb.Append("}}");
            return sim.ApplyInput(sb.ToString());
        }

        static long LongOf(string json, string key)
        {
            var m = System.Text.RegularExpressions.Regex.Match(json, "\"" + key + "\":(\\d+)");
            return m.Success ? long.Parse(m.Groups[1].Value) : 0;
        }

        static string StringOf(string json, string key)
        {
            var m = System.Text.RegularExpressions.Regex.Match(json, "\"" + key + "\":\"([^\"]*)\"");
            return m.Success ? m.Groups[1].Value : "";
        }

        /// The element objects of the ARRAY that follows `key` — the `mails` a claim reply carries,
        /// wherever it came from. Every server call that pays a reward answers in this shape
        /// (gacha, the pass, the ladders), and every one of them then feeds the rows to
        /// `ClaimFrom`, so the walk lives here once rather than in each screen.
        public static List<string> RowsUnder(string json, string key)
        {
            int at = json.IndexOf(key, System.StringComparison.Ordinal);
            if (at < 0) return new List<string>();
            int open = json.IndexOf('[', at + key.Length);
            int close = open >= 0 ? MatchingBracket(json, open) : -1;
            return close > open ? SplitRows(json.Substring(open + 1, close - open - 1))
                                : new List<string>();
        }

        /// The `]` that closes the `[` at `open`, skipping strings and nested arrays.
        static int MatchingBracket(string json, int open)
        {
            int depth = 0;
            bool inStr = false, esc = false;
            for (int i = open; i < json.Length; i++)
            {
                char c = json[i];
                if (esc) { esc = false; continue; }
                if (c == '\\') { esc = true; continue; }
                if (c == '"') { inStr = !inStr; continue; }
                if (inStr) continue;
                if (c == '[') depth++;
                else if (c == ']' && --depth == 0) return i;
            }
            return -1;
        }

        /// The element objects of a top-level JSON ARRAY, as text. Same walk as `RowsUnder`, for a
        /// payload that IS the array (`/item-market`, the inbox) rather than one nesting it.
        public static List<string> RowsOfArray(string arrayJson)
            => string.IsNullOrEmpty(arrayJson) ? new List<string>() : SplitRows(arrayJson);

        /// Split a top-level JSON array into its element objects, by brace matching. Needed
        /// because each row must be kept as TEXT for the claim to splice its item out of.
        static List<string> SplitRows(string arr)
        {
            var outp = new List<string>();
            int depth = 0, start = -1;
            bool inStr = false, esc = false;
            for (int i = 0; i < arr.Length; i++)
            {
                char c = arr[i];
                if (esc) { esc = false; continue; }
                if (c == '\\') { esc = true; continue; }
                if (c == '"') { inStr = !inStr; continue; }
                if (inStr) continue;
                if (c == '{') { if (depth++ == 0) start = i; }
                else if (c == '}' && --depth == 0 && start >= 0)
                    outp.Add(arr.Substring(start, i - start + 1));
            }
            return outp;
        }

        static string Describe(Mail m)
        {
            if (m.item != null && !string.IsNullOrEmpty(m.item.def_id))
                return GameText.ItemName(m.item.def_id, m.item.name);
            if (!string.IsNullOrEmpty(m.def_id))
            {
                var n = GameText.ItemName(m.def_id, "");
                return m.qty > 1 ? $"{n}  ×{m.qty}" : n;
            }
            if (m.gil > 0) return $"{m.gil:N0} Gil";
            return "(empty)";
        }

        static void Inset(TextMeshProUGUI t, float left, float right, float bottom, float top)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, bottom); rt.offsetMax = new Vector2(-right, top);
        }

        void Place(RectTransform rt, float y, float h = 0f)
        {
            if (h > 0f)
            {
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.sizeDelta = new Vector2(0f, h);
            }
            else
            {
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
            }
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Clear()
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }

        [System.Serializable] class Inbox { public Mail[] items; }
        [System.Serializable]
        class Mail
        {
            public ulong id;
            public string kind, def_id, reason;
            public long gil, qty;
            public MailItemView item;
        }
        [System.Serializable] class MailItemView { public string def_id, name, rarity; }
    }
}
