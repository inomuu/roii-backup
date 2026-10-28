// COMMANDER — the account's own level, its reward track, and the Outpost that feeds it.
//
// TWO SCREENS, one file, because they are two views of ONE server answer. `/commander/state` carries
// the level, the claim high-water AND the Outpost's accrued preview; splitting them into two files
// would mean two fetches of the same payload and two places to keep its field names right. The user
// split the SCREENS (2026-08-30) — Outpost is the idle faucet, Commander is the ladder — so this
// serves both from one `Mode`.
//
// WHERE THE TRACK COMES FROM. The server sends only this account's POSITION; the reward table is
// static config the client holds as a byte-identical copy of `commander_rewards.json`. That is
// deliberate on the server's side ("the client renders each level's Crystal/skill from its local
// file"), and it means a stale copy shows the wrong numbers rather than failing — so the file is
// shipped in StreamingAssets beside the catalog and belongs in the same sync discipline.
//
// THE SKILL GRANT IS AN ECHO, not a reward the client hands itself. A claim answers with
// `{skill_id, nonce}` and the client applies `GrantCommanderSkill` to the sim so it rides the next
// save, where the server re-verifies its own nonce. Grants are RE-OFFERED on every state and claim
// (that is the recovery path for a lost echo), so anything already owned must be skipped — applying
// blindly would append a redundant event to the log on every refresh.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class CommanderWindow : MonoBehaviour
    {
        public enum Mode { Ladder, Outpost }

        const float RowH = 46f, Gap = 4f, BarH = 6f;

        Mode _mode;
        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _head, _status;
        readonly List<GameObject> _built = new();
        string _json = "";
        bool _busy;
        /// What the last action actually did. A claim sets the status and then immediately calls
        /// Refresh, which redraws the screen — and the redraw would overwrite the one line the
        /// player pressed the button to read. It survives exactly one rebuild.
        string _flash;

        public static CommanderWindow Attach(RectTransform body, Mode mode)
        {
            var w = body.gameObject.AddComponent<CommanderWindow>();
            w._mode = mode;
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
                var notice=MenuNotice.Create(_content,"Commander",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_COMMANDER"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            net.Run(net.Client.CommanderState((ok, json) =>
            {
                if (!ok) { _status.text = "state failed: " + json; return; }
                _json = json;
                ApplyGrants(json, "\"skill_grants\":");
                Rebuild();
            }));
        }

        void Rebuild()
        {
            Clear();
            var st = JsonUtility.FromJson<CommanderState>(_json) ?? new CommanderState();
            if (!st.ok)
            {
                _head.text = "";
                _status.text = string.IsNullOrEmpty(st.reason) ? "Not available." : st.reason;
                UiScroll.Fit(_content, UiTheme.Pad);
                return;
            }
            if (_mode == Mode.Outpost) { BuildOutpost(st); return; }
            BuildLadder(st);
        }

        // --- the idle faucet ------------------------------------------------------------------

        void BuildOutpost(CommanderState st)
        {
            _head.text = $"Commander level {st.level}";
            _status.text = Take() ?? "The Outpost banks Commander XP while you are away.";

            float y = 0f;
            var banked = UiTheme.Label(_content, "Banked",
                $"{st.outpost_preview:N0} / {st.outpost_cap:N0} XP waiting",
                24f, st.outpost_preview > 0 ? UiTheme.TextHigh : UiTheme.TextMid,
                TextAlignmentOptions.Left);
            banked.overflowMode = TextOverflowModes.Overflow;
            Place(banked.rectTransform, y, 34f);
            _built.Add(banked.gameObject);
            y += 34f + UiTheme.Gap;

            var track = UiTheme.Flat(_content, "Track", new Color(1f, 1f, 1f, 0.06f));
            Place(track.rectTransform, y, BarH);
            _built.Add(track.gameObject);
            Fill(track, st.outpost_cap > 0 ? st.outpost_preview / (float)st.outpost_cap : 0f);
            y += BarH + UiTheme.Pad;

            // PRESSABLE AT ZERO, and that is not sloppiness — it is the only way the faucet ever
            // starts. The server accrues from a stored timestamp that is 0 until the first claim
            // stamps it ("first contact stamps `now` and pays 0"), so a client that greys the
            // button out while nothing has accrued leaves the Outpost switched off forever. A
            // press with nothing owed is safe: the claim carries the sub-CXP remainder, so it
            // cannot reset the clock either.
            var collect = HudBar.MakeButton(_content, Loc.Get("UI_COMMANDER_COLLECT"), "", 300f, UiLayout.Touch, Collect);
            collect.interactable = !_busy;
            var ctext = collect.GetComponentInChildren<TextMeshProUGUI>();
            ctext.text = st.outpost_preview > 0 ? $"Collect {st.outpost_preview:N0} XP" : "Collect";
            Place(collect.GetComponent<RectTransform>(), y);
            _built.Add(collect.gameObject);
            y += UiLayout.Touch + UiTheme.Gap;

            // The cap is the honest half of an idle faucet: it says when checking back stops paying.
            var note = UiTheme.Label(_content, "Note",
                st.outpost_preview > 0
                    ? $"Accrual stops at {st.outpost_cap:N0}. The server counts the time, not this client."
                    : $"Collecting starts the clock. Accrual stops at {st.outpost_cap:N0}.",
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
            _status.text = Loc.Get("UI_COMMANDER_COLLECTING");
            net.Run(net.Client.OutpostClaim((ok, body) =>
            {
                _busy = false;
                var got = LongOf(body, "accrued");
                _flash = ok
                    ? (got > 0 ? $"banked {got:N0} XP into Commander level" : "the Outpost clock is running")
                    : "refused: " + body;
                Refresh();
            }));
        }

        // --- the ladder -----------------------------------------------------------------------

        void BuildLadder(CommanderState st)
        {
            var rewards = Track();
            var pending = new HashSet<int>(st.pending_reward_levels ?? new int[0]);

            _head.text = st.to_next > 0
                ? $"Level {st.level}   ·   {st.into_level:N0} / {st.to_next:N0} to the next"
                : $"Level {st.level}   ·   at the cap";
            _status.text = Take() ?? (pending.Count > 0
                ? $"{pending.Count} level reward{(pending.Count == 1 ? "" : "s")} to accept"
                : "Nothing to accept.");

            float y = 0f;
            var track = UiTheme.Flat(_content, "Track", new Color(1f, 1f, 1f, 0.06f));
            Place(track.rectTransform, y, BarH);
            _built.Add(track.gameObject);
            Fill(track, st.to_next > 0 ? st.into_level / (float)st.to_next : 1f);
            y += BarH + UiTheme.Pad;

            var claim = HudBar.MakeButton(_content, Loc.Get("UI_COMMANDER_CLAIMALL"), "", 300f, UiLayout.Touch,
                                          () => Claim(0));
            claim.interactable = !_busy && pending.Count > 0;
            var ct = claim.GetComponentInChildren<TextMeshProUGUI>();
            ct.text = pending.Count > 0 ? $"Accept {pending.Count}" : "Nothing to accept";
            if (pending.Count == 0) ct.color = UiTheme.TextLow;
            Place(claim.GetComponent<RectTransform>(), y);
            _built.Add(claim.gameObject);
            y += UiLayout.Touch + UiTheme.Pad;

            Head(ref y, "Reward track");
            foreach (var rw in rewards)
            {
                bool reached = st.level >= rw.level;
                bool got = st.reward_claimed_through >= rw.level;
                var row = UiTheme.Box(_content, "L" + rw.level,
                                      reached && !got ? UiTheme.SurfaceActive : UiTheme.Surface);
                UiTheme.Border(row.rectTransform, reached && !got ? UiTheme.Accent : UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var lvl = UiTheme.Label(row.rectTransform, "L", rw.level.ToString(), UiTheme.CaptionSize,
                                        reached ? UiTheme.TextMid : UiTheme.TextLow,
                                        TextAlignmentOptions.Left);
                Inset(lvl, UiTheme.Pad, RowH * 8f);

                var what = string.IsNullOrEmpty(rw.skill)
                    ? $"{rw.crystal:N0} {GameText.Crystal}"
                    : $"{rw.crystal:N0} {GameText.Crystal}  ·  {GameText.SkillName(rw.skill)}";
                var name = UiTheme.Label(row.rectTransform, "N", what, UiTheme.BodySize,
                                         got ? UiTheme.TextLow : reached ? UiTheme.TextHigh : UiTheme.TextMid,
                                         TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad + 44f, 160f);

                var state = UiTheme.Label(row.rectTransform, "S",
                                          got ? "accepted" : reached ? "ready" : "locked",
                                          UiTheme.CaptionSize,
                                          got ? UiTheme.TextLow : reached ? UiTheme.Accent : UiTheme.TextLow,
                                          TextAlignmentOptions.Right);
                Inset(state, UiTheme.Pad, UiTheme.Pad);
                y += RowH + Gap;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        void Claim(int upTo)
        {
            var net = Roi.Net.OnlineService.Instance;
            if (_busy || !net.LoggedIn) return;
            _busy = true;
            _status.text = Loc.Get("UI_COMMANDER_ACCEPTING");
            net.Run(net.Client.CommanderClaim(upTo, (ok, body) =>
            {
                _busy = false;
                if (!ok) { _status.text = "refused: " + body; return; }
                ApplyGrants(body, "\"grants\":");
                var got = LongOf(body, "crystal");
                _flash = got > 0 ? $"accepted — {got:N0} {GameText.Crystal}" : "accepted";
                Refresh();
            }));
        }

        // --- the echo -------------------------------------------------------------------------

        /// Apply each grant the server offers that the sim does not already own. The OWNED set is
        /// read raw from the view (`commander_skills_owned`), not from a catalog-filtered bar list:
        /// a skill this build has no catalog row for still reads as owned, and would otherwise be
        /// re-appended to the save log on every single refresh.
        void ApplyGrants(string json, string key)
        {
            var rows = MailWindow.RowsUnder(json, key);
            if (rows.Count == 0) return;
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            var owned = OwnedSkills(sim.ViewJson());
            foreach (var row in rows)
            {
                var skill = StringOf(row, "skill_id");
                if (string.IsNullOrEmpty(skill) || owned.Contains(skill)) continue;
                // The nonce is a decimal STRING: it is a u64 and would lose its low bits as a
                // JSON number, which would make the server refuse its own grant.
                var nonce = StringOf(row, "nonce");
                sim.ApplyInput("{\"GrantCommanderSkill\":{\"skill_id\":\"" + skill
                             + "\",\"nonce\":" + (string.IsNullOrEmpty(nonce) ? "0" : nonce) + "}}");
            }
        }

        /// The pending message, consumed. Null when there is none.
        string Take() { var f = _flash; _flash = null; return f; }

        static HashSet<string> OwnedSkills(string viewJson)
        {
            var owned = new HashSet<string>();
            int at = viewJson.IndexOf("\"commander_skills_owned\"", System.StringComparison.Ordinal);
            if (at < 0) return owned;
            int open = viewJson.IndexOf('[', at);
            int close = open >= 0 ? viewJson.IndexOf(']', open) : -1;
            if (close < 0) return owned;
            foreach (System.Text.RegularExpressions.Match m in
                     System.Text.RegularExpressions.Regex.Matches(
                         viewJson.Substring(open, close - open), "\"([^\"]+)\""))
                owned.Add(m.Groups[1].Value);
            return owned;
        }

        // --- the local reward table -------------------------------------------------------------

        static Reward[] _track;

        /// The static config, loaded once. A MISSING file is reported rather than guessed around:
        /// the numbers here are what the player is promised, and inventing them would show a
        /// different ladder than the one the server pays.
        static Reward[] Track()
        {
            if (_track != null) return _track;
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "commander_rewards.json");
            if (!System.IO.File.Exists(path))
            {
                Debug.LogError("COMMANDER: commander_rewards.json is not in StreamingAssets — "
                             + "the reward track cannot be drawn.");
                return _track = new Reward[0];
            }
            var cfg = JsonUtility.FromJson<RewardFile>(System.IO.File.ReadAllText(path));
            return _track = cfg?.rewards ?? new Reward[0];
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

        void Fill(Image track, float frac)
        {
            var fill = UiTheme.Flat(track.rectTransform, "Fill", UiTheme.Accent);
            fill.raycastTarget = false;
            var fr = fill.rectTransform;
            fr.anchorMin = new Vector2(0f, 0f); fr.anchorMax = new Vector2(0f, 1f);
            fr.pivot = new Vector2(0f, 0.5f);
            Canvas.ForceUpdateCanvases();
            fr.sizeDelta = new Vector2(track.rectTransform.rect.width * Mathf.Clamp01(frac), 0f);
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
        class CommanderState
        {
            public bool ok;
            public string reason;
            public int level, max_level, reward_claimed_through;
            public long cxp, into_level, to_next, outpost_preview, outpost_cap;
            public int[] pending_reward_levels;
        }
        [System.Serializable] class Reward { public int level; public long crystal; public string skill; }
        [System.Serializable] class RewardFile { public int max_level; public Reward[] rewards; }
    }
}
