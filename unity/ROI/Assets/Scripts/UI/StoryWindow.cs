// THE STORY TAB: a `.dialogue` file plays, and the cues in it drive everything else. Ported from
// godot/views/story_view.gd.
//
// The script is the SPINE — read, fight, read — so this file does not sequence a mission. It plays
// the file and answers what the file asks for: field a cast, start a stage, change a backdrop,
// take someone off the field. That is why the prologue can grow from two scenes to twenty without
// this file changing.
//
// It owns its own DialogueBox and its own StoryDriver, so nothing here can reach the live game.
// The arena it renders is the SAME arena the real game uses, LENT to the sandbox driver for the
// length of a stage: the live sim keeps ticking, it just stops being what the screen looks at.
//
// The mission list walks the `requires` CHAIN rather than sorting filenames. Sorting looked fine
// with three files and breaks at ten — "1-10" sorts between "1-1" and "1-2", and "prologue" sorts
// after both because digits precede letters.
//
// WHAT IS NOT HERE: the reward. A mission's payout is a SERVER call keyed once per
// (account, mission_id), and `story_reward_view.gd`'s summary screen is a screen over that call.
// Until the endpoint is wired this window says a mission was completed and grants nothing, which
// is the honest half — the half that pays is the half that must not be guessed at.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class StoryWindow : MonoBehaviour
    {
        /// Named casts a mission can field with `@ roster preset <id>`. Data, not code, the day a
        /// second one is needed — for now the Prologue's three are the only cast that exists.
        static readonly Dictionary<string, Roi.Story.StoryDriver.Cast[]> Presets = new()
        {
            ["trial_three"] = new[]
            {
                Roi.Story.StoryDriver.Cast.Of("seraphine", 50),
                Roi.Story.StoryDriver.Cast.Of("lucielle", 50),
                Roi.Story.StoryDriver.Cast.Of("rapis", 50),
            },
        };

        RectTransform _body, _list;
        TextMeshProUGUI _note, _status;
        readonly List<GameObject> _built = new();

        Roi.Story.StoryDriver _driver;
        Roi.Story.DialogueBox _box;
        Roi.Arena.ArenaView _arena;

        Roi.Story.StoryReward _reward;
        System.Diagnostics.Stopwatch _clock;
        string _missionId = "";
        List<Roi.Story.StoryDriver.Cast> _pendingRoster = new();
        /// True while an AUTHORED stage is running — see the battle cue.
        bool _stageAuthored;
        int _kills;
        readonly HashSet<string> _cleared = new();
        string _missionTitle = "", _missionReward = "";
        bool _askedProgress;

        public static StoryWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<StoryWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            _body = body;
            float pad = UiTheme.PadLarge;
            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -34f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _note = UiTheme.Label(body, "Note",
                Loc.Get("UI_STORY_NOTE"), UiTheme.CaptionSize, UiTheme.TextLow,
                TextAlignmentOptions.BottomLeft);
            _note.textWrappingMode = TextWrappingModes.Normal;
            var nr = _note.rectTransform;
            nr.anchorMin = Vector2.zero; nr.anchorMax = new Vector2(1f, 0f);
            nr.pivot = new Vector2(0.5f, 0f);
            nr.offsetMin = new Vector2(pad, UiTheme.PadSmall);
            nr.offsetMax = new Vector2(-pad, UiTheme.PadSmall + 34f);
            var host=new GameObject("ChapterList",typeof(RectTransform));host.transform.SetParent(body,false);
            var hr=(RectTransform)host.transform;hr.anchorMin=Vector2.zero;hr.anchorMax=Vector2.one;
            hr.offsetMin=new Vector2(pad,58);hr.offsetMax=new Vector2(-pad,-44);
            _list=UiScroll.Create(hr,out _);

        }

        // --- the list ---------------------------------------------------------------------

        public void Refresh()
        {
            // WHAT THIS ACCOUNT HAS CLEARED is the server's record, not this session's. A set kept
            // in a field re-locked every mission on restart, which reads as progress lost.
            var net = Roi.Net.OnlineService.Instance;
            if (net != null && net.LoggedIn && !_askedProgress)
            {
                _askedProgress = true;
                StartCoroutine(net.Client.StoryProgress((ok, cleared) =>
                {
                    if (!ok) return;
                    foreach (var id in cleared) _cleared.Add(id);
                    if (isActiveAndEnabled) Refresh();
                }));
            }

            foreach (var go in _built) Destroy(go);
            _built.Clear();

            var missions = LoadMissions(out var errors);
            _status.text = missions.Count == 0
                ? Loc.Get("UI_STORY_EMPTY")
                : Loc.Get("UI_STORY_TITLE");
            foreach (var e in errors) Debug.LogWarning("STORY: " + e);

            float y = 0f;
            foreach (var m in missions)
            {
                bool open = string.IsNullOrEmpty(m.Requires) || _cleared.Contains(m.Requires);
                var title = Loc.GetOr(m.Title, m.Id);
                var brief = Loc.GetOr(m.Brief, "");
                var mission = m;
                var button=HudBar.MakeButton(_list,title,"",0,112,()=>Begin(mission));
                button.interactable=open;
                var rt=(RectTransform)button.transform;rt.anchorMin=new Vector2(0,1);rt.anchorMax=new Vector2(1,1);
                rt.pivot=new Vector2(.5f,1);rt.sizeDelta=new Vector2(-8,112);rt.anchoredPosition=new Vector2(0,-y);
                var surface=button.GetComponent<CodexButtonImage>();
                surface.color=open?Color.white:new Color(.66f,.66f,.66f,1);

                var label=button.GetComponentInChildren<TextMeshProUGUI>();UiTheme.Display(label);
                label.fontSize=26;label.alignment=TextAlignmentOptions.Left;label.color=open?UiTheme.TextHigh:UiTheme.TextMid;
                var lr=label.rectTransform;lr.anchorMin=new Vector2(0,1);lr.anchorMax=new Vector2(1,1);
                lr.pivot=new Vector2(.5f,1);lr.offsetMin=new Vector2(106,-57);lr.offsetMax=new Vector2(-160,-18);
                var number=UiTheme.Label(rt,"Chapter",(missions.IndexOf(m)+1).ToString("00"),36,open?UiTheme.Accent:UiTheme.TextLow,TextAlignmentOptions.Center);
                UiTheme.Display(number);var nr=number.rectTransform;nr.anchorMin=nr.anchorMax=new Vector2(0,.5f);nr.sizeDelta=new Vector2(80,60);nr.anchoredPosition=new Vector2(50,0);
                var sub=UiTheme.Label(rt,"Brief",open?(brief==""?(m.Recommend>0?Loc.Get("UI_STORY_RECOMMEND")+" "+m.Recommend:""):brief):Loc.Fmt("UI_STORY_LOCKED_BY",m.Requires),15,UiTheme.TextMid,TextAlignmentOptions.Left);
                sub.textWrappingMode=TextWrappingModes.Normal;
                var sr=sub.rectTransform;sr.anchorMin=Vector2.zero;sr.anchorMax=new Vector2(1,0);sr.pivot=new Vector2(.5f,0);
                sr.offsetMin=new Vector2(106,17);sr.offsetMax=new Vector2(-160,51);
                var state=UiTheme.Label(rt,"State",open?Loc.Get("UI_STORY_START")+"  ›":"—",17,open?UiTheme.Accent:UiTheme.TextLow,TextAlignmentOptions.Right);
                var ar=state.rectTransform;ar.anchorMin=new Vector2(1,0);ar.anchorMax=Vector2.one;ar.offsetMin=new Vector2(-145,16);ar.offsetMax=new Vector2(-26,-16);
                _built.Add(button.gameObject);
                y+=126f;
            }
            UiScroll.Fit(_list,y);
        }

        sealed class Mission
        {
            public string Id, Requires, Title, Brief, Path, RewardText;
            public int Recommend;
            public Roi.Story.DialogueScript Script;
        }

        /// Every mission file, in CHAIN order: the roots first, then whatever each one unlocks.
        /// One `requires` edge per mission makes the missions a chain, which is what "clear N to
        /// see N+1" means, and it makes the display order fall out of the data.
        List<Mission> LoadMissions(out List<string> errors)
        {
            errors = new List<string>();
            var dir = System.IO.Path.Combine(Application.streamingAssetsPath, "story");
            var found = new List<Mission>();
            if (!System.IO.Directory.Exists(dir)) return found;
            foreach (var path in System.IO.Directory.GetFiles(dir, "*.dialogue"))
            {
                var script = Roi.Story.DialogueScript.LoadFile(path);
                foreach (var e in script.Errors) errors.Add(System.IO.Path.GetFileName(path) + ": " + e);
                if (!script.Meta.TryGetValue("id", out var id) || id == "") continue;
                script.Meta.TryGetValue("recommend", out var rec);
                found.Add(new Mission
                {
                    Id = id,
                    Requires = script.Meta.TryGetValue("requires", out var r) ? r : "",
                    Title = script.Meta.TryGetValue("title", out var t) ? t : id,
                    Brief = script.Meta.TryGetValue("brief", out var b) ? b : "",
                    Recommend = int.TryParse(rec, out var n) ? n : 0,
                    RewardText = script.Meta.TryGetValue("reward_text", out var rw) ? rw : "",
                    Path = path,
                    Script = script,
                });
            }
            var order = new List<Mission>();
            var placed = new HashSet<string>();
            foreach (var m in found) if (m.Requires == "") { order.Add(m); placed.Add(m.Id); }
            // Walk the chain until nothing more can be placed. A mission whose `requires` names
            // nothing real, or a cycle, simply never appears — the sim-core guard is what reports
            // it, and reporting it twice in two clients is two places to keep in step.
            bool moved = true;
            while (moved)
            {
                moved = false;
                foreach (var m in found)
                {
                    if (placed.Contains(m.Id) || !placed.Contains(m.Requires)) continue;
                    order.Add(m); placed.Add(m.Id); moved = true;
                }
            }
            return order;
        }

        // --- playing --------------------------------------------------------------------

        void Begin(Mission m)
        {
            EnsureParts();
            _missionId = m.Id;
            _missionTitle = m.Title;
            _missionReward = m.RewardText;
            _kills = 0;
            _clock = System.Diagnostics.Stopwatch.StartNew();
            _pendingRoster = new List<Roi.Story.StoryDriver.Cast>();
            // OUT OF THE WAY. The box plays over the arena and a story fight has to be visible;
            // this window is the menu that got you here, not part of the scene.
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar != null) bar.CloseAll();
            Object.FindFirstObjectByType<ModeTabs>()?.Select(true);
            _box.Play(m.Id, m.Script);
        }

        /// The box and the sandbox are parented to the ARENA, not to this window — the same reason
        /// the companion widget is. Pressing a mission CLOSES this window so the scene can be seen,
        /// and a child of a closed window is a deactivated child: the box played its whole script
        /// to an object nobody could see, and the mission ended with nothing on screen.
        void EnsureParts()
        {
            if (_arena == null) _arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            var host = _arena != null ? _arena.transform : null;
            if (_driver == null)
            {
                _driver = Roi.Story.StoryDriver.Create(host);
                _driver.StageEnded += OnStageEnded;
                _driver.StateUpdated += OnStoryView;
            }
            if (_box == null)
            {
                _box = Roi.Story.DialogueBox.Create(host);
                _box.CueReached += OnCue;
                _box.ScriptFinished += OnScriptFinished;
            }
            if (_reward == null)
            {
                _reward = Roi.Story.StoryReward.Create(host);
                // The list comes back only when the player dismisses the summary — the one place
                // this flow waits on a click rather than on the script.
                _reward.Closed += () => { var b = Object.FindFirstObjectByType<HudBar>(); if (b != null) b.Toggle("Story"); };
                // RUN ON THE PANEL, not on this window. Pressing a mission CLOSES this window,
                // so by the time the summary appears this component's own GameObject is inactive
                // and `StartCoroutine` on it throws — the claim would never leave the client.
                _reward.ClaimRequested += () => _reward.StartCoroutine(ClaimReward(_missionId));
            }
        }

        void OnStoryView(string json)
        {
            if (_arena != null) _arena.FeedView(json);
        }

        void OnCue(string scriptId, string cue, string[] args)
        {
            switch (cue)
            {
                case "roster": CueRoster(args); break;
                case "battle": CueBattle(args); break;
                // Backdrops are not built. Named here so a script can already carry them and the
                // log shows what a finished mission will ask for.
                case "scene": Debug.Log($"STORY: scene -> {string.Join(" ", args)}"); break;
                case "cutscene": break;
                case "remove_member": CueRemoveMember(args); break;
            }
        }

        /// `@ roster preset <id>` / `@ roster party`. Recorded, not applied: the cast is only
        /// needed when a stage actually starts, and a mission may declare it long before the first
        /// fight.
        void CueRoster(string[] args)
        {
            _pendingRoster = new List<Roi.Story.StoryDriver.Cast>();
            if (args.Length == 0) return;
            if (args[0] == "preset" && args.Length > 1)
            {
                if (Presets.TryGetValue(args[1], out var cast)) _pendingRoster.AddRange(cast);
                else Debug.LogWarning($"STORY: unknown roster preset '{args[1]}'");
            }
            // `party` — the player's own, once snapshotting exists. An empty roster leaves whatever
            // the zone's own start produces.
        }

        /// `@ battle <zone_id> [ticks]`. HOLDS the script: the closing lines of a scene must not
        /// play over the fight they are supposed to follow.
        void CueBattle(string[] args)
        {
            if (args.Length == 0) { Debug.LogWarning("STORY: `@ battle` needs a zone id"); return; }
            var zone = args[0];
            int ticks = args.Length > 1 && int.TryParse(args[1], out var t) ? t : 0;
            // Giving a tick count is how a script says "this beat is authored — I do not care how
            // it goes". The scene after it is already written, so winning a fight the story calls a
            // defeat still leads to the written defeat, and that is correct: the player beat the
            // monsters, not the situation. WITHOUT a tick count the stage is a real fight and
            // losing it fails the mission.
            _stageAuthored = ticks > 0;
            if (!_driver.StartStage(_missionId, zone, _pendingRoster, ticks))
            {
                // ABANDON, not skip. Skipping used to feel like the forgiving choice and it paid
                // for a fight that never happened: the script ran to `=> END`, the finish fired,
                // and the reward screen claimed its Gil. A one-word typo in a `.dialogue` was a
                // free mission. A mission whose fight cannot start has not been completed.
                Debug.LogError($"STORY: stage '{zone}' does not exist — mission '{_missionId}' "
                             + "failed (no reward)");
                Fail();
                return;
            }
            if (_arena != null) { _arena.SuspendLiveView(true); _arena.ResetActors(); }
            _box.Hold();
        }

        /// `@ remove_member <unit_def>` — take someone off the field for good, which is how a
        /// sacrifice lands mid-fight. Matched on class id because the sandbox cast has no names.
        void CueRemoveMember(string[] args)
        {
            if (args.Length == 0 || _driver == null || !_driver.Running) return;
            foreach (var id in _driver.PartyIds())
                if (_driver.ClassOf(id).StartsWith(args[0], System.StringComparison.Ordinal))
                {
                    _driver.Kill(id);
                    return;
                }
            Debug.LogWarning($"STORY: no member matching '{args[0]}' to remove");
        }

        /// An AUTHORED stage continues whatever happened — cleared early, ran out its clock, or
        /// wiped. The script already knows what comes next and the player does not get to argue
        /// with the plot by winning. An UNauthored stage is a real fight, so a wipe ends the
        /// mission: there is nothing sensible for the next scene to say.
        void OnStageEnded(string stageId, string outcome, int kills)
        {
            _kills += kills;
            GiveTheStageBack();
            if (outcome == "wiped" && !_stageAuthored) { Fail(); return; }
            _box.Release();
        }

        void GiveTheStageBack()
        {
            if (_arena == null) return;
            _arena.ResetActors();
            _arena.SuspendLiveView(false);
        }

        void OnScriptFinished(string id)
        {
            if (string.IsNullOrEmpty(id)) return;
            _cleared.Add(id);
            GiveTheStageBack();
            var stats = new List<KeyValuePair<string, string>>
            {
                new(Loc.Get("UI_STORY_STAT_KILLS"), _kills.ToString()),
                new(Loc.Get("UI_STORY_STAT_TIME"), Elapsed()),
            };
            // The panel ASKS for the payout; it never pays. The claim waits for the player to
            // press, because an automatic one that failed had no second chance.
            _reward.ShowResult(_missionTitle, _missionReward, stats);
        }

        string Elapsed()
        {
            var t = _clock != null ? _clock.Elapsed : System.TimeSpan.Zero;
            return $"{(int)t.TotalMinutes}:{t.Seconds:00}";
        }

        /// The payout. Routed through the server for the reason nothing here can be: the server
        /// decides what a mission is worth, pays it once keyed on (account, mission), and delivers
        /// it as MAIL — so a replay of this save sees a server-authored claim and verifies. A
        /// number written on this side would be a mint.
        System.Collections.IEnumerator ClaimReward(string missionId)
        {
            var net = Roi.Net.OnlineService.Instance;
            if (net == null || !net.LoggedIn)
            {
                // Not settled: signing back in and pressing again is a real path to the reward, so
                // the button has to survive this.
                _reward.SetRewardLine(Loc.Get("UI_STORY_REWARD_OFFLINE"));
                _reward.Settled(false);
                yield break;
            }
            _reward.SetRewardLine(Loc.Get("UI_STORY_REWARD_PENDING"));
            bool ok = false; long gil = 0; string why = "";
            yield return net.Client.StoryClaim(missionId, (o, g, r) => { ok = o; gil = g; why = r; });
            if (!_reward.IsOpen) yield break;      // dismissed while the call was in flight
            if (!ok)
            {
                _reward.SetRewardLine(Loc.Get("UI_STORY_REWARD_FAILED"));
                _reward.Settled(false);
                Debug.LogWarning($"STORY: reward claim refused: {why}");
                yield break;
            }
            // 0 means the mission was already paid on an earlier run — the honest answer, not a
            // second payout.
            _reward.SetRewardLine(gil > 0 ? Loc.Fmt("UI_STORY_REWARD_GRANTED", gil)
                                          : Loc.Get("UI_STORY_REWARD_ALREADY"));
            _reward.Settled(true);
        }

        /// Drop the mission where it stands — on a wipe, or when the player leaves. Nothing is
        /// recorded and nothing is granted, which is the whole reason a failed story run is cheap.
        public void Abandon()
        {
            if (_driver != null) _driver.Stop();
            if (_box != null) _box.Clear();
            GiveTheStageBack();
        }

        void Fail()
        {
            var id = _missionId;
            Abandon();
            Debug.LogWarning($"STORY: mission '{id}' failed — nothing recorded");
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar != null) bar.Toggle("Story");
        }

        void OnDestroy()
        {
            Abandon();
            // They live outside this window's subtree, so closing it does not take them with it.
            if (_driver != null) Destroy(_driver.gameObject);
            if (_box != null) Destroy(_box.gameObject);
            if (_reward != null) Destroy(_reward.gameObject);
        }
    }
}
