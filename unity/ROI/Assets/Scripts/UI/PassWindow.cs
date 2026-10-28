// THE SEASON PASS — the server's track, and one press that claims everything earned.
//
// Ported from `bp_view.gd`, with one structural change and it is deliberate: Godot draws the 50
// slots as a horizontally scrolling grid of columns; this draws them as rows in the vertical scroll
// every other screen uses. The client is mobile-shaped now (user, 2026-09-07) — a sideways scroll
// inside a vertical one is the gesture that fights the phone hardest, and 50 rows is a list a thumb
// already knows how to read.
//
// SERVER TRUTH ONLY. Level, XP, the track and the claim ledger all come from `/bp/state`; nothing
// here derives a level from an XP number, because the day the curve changes the client would be
// paying out a different ladder than the server honours.
//
// The claim is the gacha's path, not the mailbox's: `/bp/claim` mints reward MAILS and answers with
// them, and the client applies each one to the sim as a `ClaimMail` event. The next verified save is
// what settles it. Calling the mailbox's own claim endpoint instead would consume the mail
// out-of-band and credit nothing — see the note on `ServerClient.MailInbox`.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class PassWindow : MonoBehaviour
    {
        const float RowH = 46f, Gap = 4f, BarH = 6f;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _head, _status;
        readonly List<GameObject> _built = new();
        string _json = "";
        /// What the last claim actually collected. The claim refreshes the screen, and the redraw
        /// would otherwise overwrite the one line the player pressed the button to read.
        string _flash;

        public static PassWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<PassWindow>();
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
            _head = UiTheme.Label(body, "Head", "", UiTheme.HeadSize,
                                  UiTheme.Accent, TextAlignmentOptions.Right);
            var hr = _head.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(pad, -40f); hr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize,
                                    UiTheme.TextLow, TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-(pad + 340f), -UiTheme.PadSmall);

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
                _head.text = "";
                _status.text = "";
                Clear();
                var notice=MenuNotice.Create(_content,"Pass",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_PASS"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            net.Run(net.Client.PassState((ok, json) =>
            {
                if (!ok) { _status.text = "state failed: " + json; return; }
                _json = json;
                Rebuild();
            }));
        }

        void Rebuild()
        {
            Clear();
            var st = JsonUtility.FromJson<PassState>(_json) ?? new PassState();
            if (!st.ok)
            {
                // "no active season" is the ordinary answer on a fresh server, not a failure. It is
                // said plainly rather than dressed up as an error.
                _head.text = "";
                _status.text = string.IsNullOrEmpty(st.reason) ? "The pass is not running." : st.reason;
                UiScroll.Fit(_content, UiTheme.Pad);
                return;
            }

            var claimed = new HashSet<int>(st.claimed ?? new int[0]);
            var ready = new List<int>();
            foreach (var r in st.free_track ?? new Reward[0])
                if (r.slot <= st.level && !claimed.Contains(r.slot)) ready.Add(r.slot);

            _head.text = $"Level {st.level}   ·   season ends in {Span(st.ends_at_ms - st.now_ms)}";
            _status.text = _flash ?? (ready.Count > 0
                ? $"{ready.Count} reward{(ready.Count == 1 ? "" : "s")} waiting"
                : "Nothing to claim yet.");
            _flash = null;

            float y = 0f;

            // The XP row: how far into the CURRENT level, from the server's own slot cost. The
            // level itself is never derived here — only the fraction inside it.
            long into = st.slot_cost > 0 ? st.xp % st.slot_cost : 0;
            var track = UiTheme.Flat(_content, "XpTrack", new Color(1f, 1f, 1f, 0.06f));
            Place(track.rectTransform, y, BarH);
            _built.Add(track.gameObject);
            var fill = UiTheme.Flat(track.rectTransform, "Fill", UiTheme.Accent);
            var fr = fill.rectTransform;
            fr.anchorMin = new Vector2(0f, 0f); fr.anchorMax = new Vector2(0f, 1f);
            fr.pivot = new Vector2(0f, 0.5f);
            fr.sizeDelta = new Vector2(0f, 0f);
            fill.raycastTarget = false;
            // Sized from the track's real width, which uGUI only knows after a layout pass.
            Canvas.ForceUpdateCanvases();
            float w = track.rectTransform.rect.width;
            fr.sizeDelta = new Vector2(w * (st.slot_cost > 0 ? Mathf.Clamp01(into / (float)st.slot_cost) : 0f), 0f);
            y += BarH + UiTheme.Gap;

            var xp = UiTheme.Label(_content, "Xp",
                st.slot_cost > 0 ? $"{into:N0} / {st.slot_cost:N0} XP to the next level"
                                 : $"{st.xp:N0} XP",
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(xp.rectTransform, y, 22f);
            _built.Add(xp.gameObject);
            y += 22f + UiTheme.Pad;

            // ONE button for everything earned: the server takes up to 64 slots in a call and pays
            // each exactly once, so a per-slot button would be fifty presses for one transaction.
            var claim = HudBar.MakeButton(_content, Loc.Get("UI_COMMON_CLAIM"), "", 300f, UiLayout.Touch,
                                          () => Claim(ready));
            claim.interactable = ready.Count > 0;
            var ct = claim.GetComponentInChildren<TextMeshProUGUI>();
            ct.text = ready.Count > 0 ? $"Claim {ready.Count}" : "Nothing to claim";
            if (ready.Count == 0) ct.color = UiTheme.TextLow;
            Place(claim.GetComponent<RectTransform>(), y);
            _built.Add(claim.gameObject);
            y += UiLayout.Touch + UiTheme.Pad;

            Head(ref y, "Free track");
            foreach (var r in st.free_track ?? new Reward[0])
            {
                bool got = claimed.Contains(r.slot);
                bool earned = r.slot <= st.level;
                var row = UiTheme.Box(_content, "S" + r.slot,
                                      earned && !got ? UiTheme.SurfaceActive : UiTheme.Surface);
                // Three states, and the middle one is what the screen is for: earned-and-unclaimed.
                UiTheme.Border(row.rectTransform, earned && !got ? UiTheme.Accent : UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var lvl = UiTheme.Label(row.rectTransform, "L", r.slot.ToString(), UiTheme.CaptionSize,
                                        earned ? UiTheme.TextMid : UiTheme.TextLow,
                                        TextAlignmentOptions.Left);
                Inset(lvl, UiTheme.Pad, RowH * 8f);

                var name = UiTheme.Label(row.rectTransform, "N", RewardName(r), UiTheme.BodySize,
                                         got ? UiTheme.TextLow : earned ? UiTheme.TextHigh : UiTheme.TextMid,
                                         TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad + 44f, 160f);

                var state = UiTheme.Label(row.rectTransform, "S",
                                          got ? "claimed" : earned ? "ready" : $"level {r.slot}",
                                          UiTheme.CaptionSize,
                                          got ? UiTheme.TextLow : earned ? UiTheme.Accent : UiTheme.TextLow,
                                          TextAlignmentOptions.Right);
                Inset(state, UiTheme.Pad, UiTheme.Pad);
                y += RowH + Gap;
            }

            var note = UiTheme.Label(_content, "Note",
                Loc.Get("UI_PASS_FREE_TRACK_REWARDS_ARE_BOUND"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            Place(note.rectTransform, y + UiTheme.Gap, 24f);
            _built.Add(note.gameObject);
            UiScroll.Fit(_content, y + 32f + UiTheme.Pad);
        }

        void Claim(List<int> slots)
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn || slots.Count == 0) return;
            _status.text = Loc.Get("UI_PASS_CLAIMING");
            net.Run(net.Client.PassClaim(slots, (ok, body) =>
            {
                if (!ok) { _status.text = "refused: " + body; return; }
                Collect(body);
                Refresh();
            }));
        }

        /// Apply the mails the claim minted straight to the save. The reply already carries the
        /// full rows, so nothing has to be fetched — and a reward that fails here is REPORTED, not
        /// swallowed: it is still in the mailbox, and saying so is what tells the player where.
        void Collect(string body)
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            var rows = MailWindow.RowsUnder(body, "\"mails\":");
            int got = 0, failed = 0;
            foreach (var row in rows)
            {
                var m = System.Text.RegularExpressions.Regex.Match(row, "\"id\":(\\d+)");
                if (sim == null || !m.Success) { failed++; continue; }
                if (MailWindow.ClaimFrom(sim, ulong.Parse(m.Groups[1].Value), row)) got++; else failed++;
            }
            _flash = failed == 0
                ? $"{got} collected — it settles with the server on the next save"
                : $"{got} collected, {failed} left in your mailbox";
        }

        static string RewardName(Reward r)
            => r.qty > 1 ? $"{Display(r)}  ×{r.qty}" : Display(r);

        static string Display(Reward r)
            => !string.IsNullOrEmpty(r.name) ? r.name : r.def_id;

        static string Span(long ms)
        {
            if (ms <= 0) return "now";
            long s = ms / 1000, h = s / 3600, m = (s % 3600) / 60;
            return h >= 24 ? $"{h / 24}d {h % 24}h" : h > 0 ? $"{h}h {m}m" : $"{m}m";
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 26f);
            _built.Add(t.gameObject);
            y += 26f;
        }

        static void Inset(TextMeshProUGUI t, float left, float right)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, 0f); rt.offsetMax = new Vector2(-right, 0f);
        }

        void Place(RectTransform rt, float y, float h = 0f)
        {
            if (h > 0f)
            {
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.sizeDelta = new Vector2(0f, h);
                rt.anchoredPosition = new Vector2(0f, -y);
            }
            else
            {
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.anchoredPosition = new Vector2(0f, -y);
            }
        }

        void Clear()
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }

        [System.Serializable]
        class PassState
        {
            public bool ok;
            public string reason;
            public long season_id, ends_at_ms, now_ms, xp, slot_cost;
            public int level, slots;
            public Reward[] free_track;
            public int[] claimed;
        }
        [System.Serializable]
        class Reward { public int slot; public string def_id, name, rarity, kind; public long qty; }
    }
}
