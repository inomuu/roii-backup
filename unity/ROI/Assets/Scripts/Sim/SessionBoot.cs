// THE BOOT SEQUENCE — the Unity counterpart of `sim_driver_core.gd`'s `_begin_online`, and the one
// place that decides which session the game runs.
//
// It exists because a session cannot be started until the server has been asked two questions:
//
//   THE SEED IS THE SERVER'S. The account's genesis hash is derived from it — the server rebuilds
//   `new_game_as(seed, START_ZONE, class)` for every creatable class and will only trust a save
//   whose start snapshot is one of those. A session on a client-chosen seed is unpushable from its
//   first tick, and the rejection ("start snapshot is not anchored") arrives ten seconds later
//   looking like a bug in the save. That is exactly what happened while the client booted itself
//   on seed 42.
//
//   IS THERE A SAVE. `GET /save` both sets the server's tick anchor and hands back the account's
//   authoritative state. A save means RESUME (load it); none means a fresh account, and the party
//   has to be founded before there is anything to run.
//
// The party NAME is not part of the genesis — the FFI records it as a `SetPartyName` event, so the
// start snapshot stays the name-free state the server can reproduce. That is why naming a party
// here cannot break the anchor.
//
// TWO DOORS, as in the Godot picker: PLAY signs in, and "local session" starts the sim on a client
// seed with the cheats available. The second is a DEV path (memory: no-offline-version) — it can
// never push a save, and it is what the editor probes take through `ROI_BOOT=dev`.
using System;
using System.Collections;
using System.Collections.Generic;
using Roi.Net;
using UnityEngine;

namespace Roi.Sim
{
    public sealed partial class SessionBoot : MonoBehaviour
    {
        /// The zone the server builds its genesis candidates in (`START_ZONE` in verify-server).
        /// A fresh session in any other zone hashes differently and can never anchor.
        public const string StartZone = "zone_b1_1";

        /// The seed a DEV session runs on. Never used online — see the file header.
        public const ulong DevSeed = 42;

        public static SessionBoot Instance { get; private set; }

        /// True while the session is one the server verifies. A dev session is false and stays
        /// false: nothing it produces is pushed.
        public bool Online { get; private set; }

        Roi.Arena.ArenaView _arena;
        SimDriver _driver;
        Roi.UI.TitleScreen _title;
        string _catalog;
        bool _pushing;

        void Awake()
        {
            Instance = this;
            // BEFORE ArenaView.Start (every Awake runs before any Start), which is what stops it
            // booting its own seed-42 session out from under the login.
            _arena = FindFirstObjectByType<Roi.Arena.ArenaView>();
            if (_arena != null) _arena.AwaitBoot = true;
            // STEAM, before the title screen draws. It decides which sign-in the screen offers, so
            // asking later would mean drawing the wrong one and swapping it under the player.
            //
            // BUT NOT ON A DOOR THAT WILL NEVER USE IT. `SteamAPI.Init` opens a pipe to the Steam
            // client and the SDK treats losing that pipe as FATAL — `CClientPipe::BWriteAndReadResult
            // ... Fatal assert; application exiting` takes the whole process down with no handler to
            // catch it. That is correct for a shipped game (when Steam goes, the game goes) and
            // ruinous for the Editor, which holds one pipe across hours of play-mode entries: the
            // developer signing in to Steam killed the Editor mid-probe-suite and cost the run.
            //
            // The scripted doors have named their route by now and neither `dev` nor `online` needs
            // Steam, so they do not open the pipe at all. SteamAuthProbe, which does need it, calls
            // Init itself — the door it books is `dev`, and that is the point: asking for Steam is
            // now something a caller does on purpose.
            var bootDoor = BootDoor();
            // WHO OPENS A STEAM PIPE, narrowed to whoever actually needs one.
            //
            // `SteamAPI.Init` opens a pipe to the Steam client, and the SDK treats losing that pipe
            // as FATAL: `pipes.cpp … Fatal assert; application exiting` takes the process down with
            // no handler able to catch it. That is not a rare event and it is not a crash bug —
            // SIGNING IN TO STEAM ON ANOTHER MACHINE kicks this client offline, and every process
            // holding a pipe dies with it. A developer with two computers triggers it by accident;
            // a player does the same thing to their own running game.
            //
            // So a pipe is opened only where the Steam DOOR might actually be used. `dev` and
            // `online` never touch it, and a probe that stands on the title screen for other
            // reasons (LoginProbe types a name and takes the swap past Steam) says so with
            // ROI_STEAM_PIPE=0. A real launch sets none of this and gets Steam, which is the point.
            // Named for the PIPE, not for the SDK: `ROI_STEAM` is already the scripting define that
            // compiles Steamworks in at all, and two unrelated things under one name is how someone
            // later turns off the wrong one.
            var wantSteam = Setting("ROI_STEAM_PIPE", "-roisteampipe") != "0";
            if (wantSteam && bootDoor != "dev" && bootDoor != "online") Roi.Net.SteamAuth.Init();
            // STEAM GOING AWAY ENDS THE SESSION, and it ends it at the title with a sentence rather
            // than as a process that vanished. The save is not lost by this: the session pushes on
            // its own cadence and the server holds the last verified one, so returning to the login
            // costs the unsaved tail and nothing else — which is the same thing a network blip
            // costs. Quitting outright would cost the same and explain nothing.
            Roi.Net.SteamAuth.Lost += OnSteamLost;
        }

        void Start()
        {
            // The saved display settings, before anything draws — a frame cap that only takes
            // effect after someone visits Options is a frame cap nobody set.
            Roi.UI.OptionsWindow.ApplySaved();
            _catalog = SimSession.LoadCatalog();
            if (string.IsNullOrEmpty(_catalog))
            {
                Debug.LogError("BOOT: no game_data.json in StreamingAssets — cannot start anything.");
                return;
            }

            // THE SCRIPTED DOORS, the same shape as the Godot client's (VERIFY_SERVER_URL /
            // ARENA_SHOT_TICKS): a scripted run must not have to press a button.
            //
            // THREE ROUTES, and each one is NAMED rather than inferred:
            //   `dev`    — a local session on a client seed with the cheats on. Nothing it produces
            //              can be pushed. This is the Godot picker's third door, and it is a DEV
            //              path, never a shipping offline mode (CLAUDE.md: there is no offline
            //              version).
            //   `online` — the dev door on the server: signs in as ROI_NAME via /auth/dev.
            //   `steam`  — the Steam door: a real Web-API ticket via /auth/steam.
            //
            // `online` used to mean "sign in, and let OnlineService pick how". That is exactly the
            // ambiguity that made every scripted run depend on whether a Steam client happened to
            // be open on the machine. A scripted run now says which door it wants and gets that
            // door or a failure — never a different one.
            var door = BootDoor();
#if !UNITY_EDITOR && !DEVELOPMENT_BUILD
            // The scripted doors are a DEV affordance and a release build honours none of them: an
            // environment variable must never be able to put a shipped client on the dev door.
            // `steam` is left alone — it opens the same door the title screen does.
            if (door == "dev" || door == "online") door = "steam";
#endif
            Debug.Log($"BOOT: door={(string.IsNullOrEmpty(door) ? "(title screen)" : door)}");
            // ONE FRAME LATER, and the frame matters. Start order between components is undefined,
            // so a session handed over inside this Start can reach an ArenaView whose own Start has
            // not run — its tables are null and the first published view throws in RegionOfZone.
            // The online door never hit it (its handshake is asynchronous, so the arena is long
            // since up); the dev door hit it every time, and the symptom was a scene with no HUD.
            if (door == "dev") { StartCoroutine(DevNextFrame()); return; }

            _title = Roi.UI.TitleScreen.Create(transform, DefaultName(), OnlineService.ServerUrl);
            _title.PlayPressed += (name, useSteam) => StartCoroutine(SignIn(name, useSteam));
            _title.DevPressed += StartDevSession;
            _title.CreatePressed += (unitDef, party) => Found(unitDef, party);

            if (door == "online") StartCoroutine(SignIn(DefaultName(), useSteam: false));
            else if (door == "steam") StartCoroutine(SignIn(DefaultName(), useSteam: true));
        }

        IEnumerator DevNextFrame()
        {
            yield return null;
            StartDevSession();
        }

        static string DefaultName() => Setting("ROI_NAME", "-roiname") ?? "unity-dev";

        static string BootDoor() => Setting("ROI_BOOT", "-roiboot");

        /// One switch, from wherever the caller could set it. A BUILD gets it from the environment
        /// or the command line; the EDITOR gets it from `SessionState`, because a managed
        /// `SetEnvironmentVariable` does not survive the domain reload that entering play mode
        /// causes — an editor probe that set one would find it gone by the time this ran.
        static string Setting(string envKey, string argKey)
        {
            var v = Environment.GetEnvironmentVariable(envKey);
            if (!string.IsNullOrEmpty(v)) return v;
            var args = Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < args.Length; i++)
                if (args[i] == argKey) return args[i + 1];
#if UNITY_EDITOR
            v = UnityEditor.SessionState.GetString(envKey, "");
            if (!string.IsNullOrEmpty(v)) return v;
#endif
            return null;
        }

        // --- the online door ------------------------------------------------------------------

        /// How often the local slot is written. Ten seconds: an idle session's whole point is that
        /// it runs unattended, and losing the last ten seconds of one is not worth a write per tick.
        const float DevSaveEvery = 10f;
        float _devSaveAt;

        void Update()
        {
            if (Online && _driver != null && _driver.WaitingForSave)
                Roi.UI.SessionNotice.Warn(Roi.UI.Loc.Get("UI_SYNC_WAITING"));
            // ONLY the local door. An online session's save belongs to the server, and a second
            // copy on disk would be a second answer to "what happened" — the one thing a
            // replay-verified save must never have.
            if (Online || _devSaveAt == 0f || Time.time < _devSaveAt) return;
            _devSaveAt = Time.time + DevSaveEvery;
            WriteDevSave();
        }

        /// Also on the way out, so quitting does not cost the last interval.
        void OnApplicationQuit() => WriteDevSave();

        void WriteDevSave()
        {
            if (Online) return;
            var driver = FindFirstObjectByType<SimDriver>();
            var s = driver != null ? driver.Session : null;
            if (s == null || !s.IsAlive) return;
            DevSaveStore.Write(s.SaveJson());
        }

        void OnDestroy()
        {
            WriteDevSave();
            Roi.Net.SteamAuth.Lost -= OnSteamLost;
        }

        /// Only a session that came THROUGH Steam is ended by Steam leaving. A dev session has no
        /// stake in it, and a name-door session is signed in on a token the Steam client was never
        /// part of — tearing either of those down would be punishing them for somebody else's pipe.
        void OnSteamLost()
        {
            if (!Online || !OnlineService.Instance.PreferredUseSteam) return;
            Debug.LogWarning("BOOT: Steam went away under an online session — returning to the title.");
            ReturnToLogin(Roi.UI.Loc.Get("UI_TITLE_STEAM_CLOSED"));
        }

        IEnumerator SignIn(string accountName, bool useSteam)
        {
            _title?.SetBusy(true, useSteam ? "Signing in through Steam…" : "Signing in…");
            var online = OnlineService.Instance;
            yield return online.ConnectAndWait(accountName, useSteam);

            if (!online.Ready)
            {
                var why = string.IsNullOrEmpty(online.LastError)
                    ? "Could not reach the server." : online.LastError;
                // LOUD. A sign-in that fails silently is the whole failure mode this boot exists to
                // stop being invisible.
                Debug.LogError("BOOT: sign-in failed — " + why);
                _title?.SetBusy(false, why);
                yield break;
            }
            Debug.Log($"BOOT: signed in as account {online.Client.AccountId}, "
                    + $"stored save {(string.IsNullOrEmpty(online.ResumeSaveJson) ? "none" : online.ResumeSaveJson.Length + " bytes")}");

            _title?.SetStatus($"Signed in as account {online.Client.AccountId}.");

            if (!string.IsNullOrEmpty(online.ResumeSaveJson))
            {
                // RESUME. The loaded snapshot becomes this session's start checkpoint, and it is a
                // state the server already trusts — so the first push anchors on it.
                var session = SimSession.Load(online.ResumeSaveJson, _catalog);
                if (session == null || !session.IsAlive)
                {
                    _title?.SetBusy(false, "The stored save could not be loaded by this build.");
                    yield break;
                }
                Begin(session, online: true);
                yield break;
            }

            // A fresh account names the fixed starter party. The sim and verifier
            // construct the same two characters from the server seed.
            if (BootDoor() == "online")
            {
                // A SCRIPTED run has nobody to press Begin. The door means "boot into a playable
                // online session", so a first sign-in founds the default party rather than parking
                // on a form no test can fill in.
                Debug.Log("BOOT: scripted online door — founding starter party");
                Found("seraphine", _arena != null ? _arena.PartyName : "Idlers");
                yield break;
            }
            _title?.SetBusy(false, "No character on this account yet.");
            _title?.ShowCreation(new List<Roi.UI.TitleScreen.ClassInfo>());
        }

        void Found(string unitDef, string partyName)
        {
            var online = OnlineService.Instance;
            if (online.Client == null || !online.Ready) { _title?.SetStatus("Not signed in."); return; }
            _title?.SetBusy(true, "Founding your party…");
            // The fixed pair is built on the server seed in the server start zone.
            var session = SimSession.NewStarter(online.Client.Seed, _catalog, StartZone, partyName);
            if (session == null || !session.IsAlive)
            {
                _title?.SetBusy(false, "Could not start the party.");
                return;
            }
            // WHERE THE STORY WILL GO. A first-time player is meant to start in the PROLOGUE, not
            // in the farming field (user, 2026-09-07 — kept as-is for now). It belongs HERE, on the
            // branch that runs exactly once per account, and it will need the Godot architecture
            // rather than a flag: `story_driver.gd` is a SECOND, isolated sim with no save path and
            // no server client, so a story fight structurally cannot touch the account.
            Begin(session, online: true);
        }

        // --- the dev door ---------------------------------------------------------------------

        void StartDevSession()
        {
            // The ARENA'S OWN fields, not fresh constants: the cross-engine dump and the shot
            // passes set seed/zone/class on the ArenaView and expect the session to match them.
            ulong seed = _arena != null ? _arena.Seed : DevSeed;
            string zone = _arena != null ? _arena.ZoneId : StartZone;
            string party = _arena != null ? _arena.PartyName : "Idlers";
            // RESUME THE LOCAL SLOT FIRST. The dev door is the only one with cheats, so it is the
            // only one long-form testing can use — and it forgot everything on quit, which made it
            // useless for anything that takes more than one sitting.
            //
            // A save that will not load is PRESERVED rather than replaced (DevSaveStore.Preserve),
            // because the moment a schema change breaks it is the moment it is worth keeping.
            SimSession session = null;
            string stored;
            try { var slots = DevSaveStore.Slots; stored = slots.Read(slots.Active); }
            catch (Exception e)
            {
                _title?.SetBusy(false, e.Message);
                return;
            }
            if (!string.IsNullOrEmpty(stored))
            {
                session = SimSession.Load(stored, _catalog);
                if (session == null || !session.IsAlive)
                {
                    DevSaveStore.Preserve();
                    _title?.SetBusy(false, Roi.UI.Loc.Get("UI_LOCAL_SAVE_UNREADABLE"));
                    return;
                }
                else Debug.Log($"BOOT: resumed the local dev save ({stored.Length} bytes) from {DevSaveStore.Where}");
            }
            if (session == null) session = SimSession.NewStarter(seed, _catalog, zone, party);
            if (session == null || !session.IsAlive)
            {
                _title?.SetBusy(false, "Could not start a local session.");
                return;
            }
            Debug.Log("BOOT: LOCAL DEV SESSION — cheats on, nothing will be pushed.");
            Begin(session, online: false);
            _devSaveAt = Time.time + DevSaveEvery;
        }

        // --- handing over -----------------------------------------------------------------------

        void Begin(SimSession session, bool online)
        {
            Online = online;
            // A dev session keeps the cheats; an online one must not have them for a single tick.
            session.SetDebugAllowed(!online);
            _driver = _arena != null ? _arena.BeginSession(session) : null;
            if (_driver == null)
            {
                Debug.LogError("BOOT: no ArenaView to hand the session to.");
                session.Dispose();
                return;
            }
            _driver.RequireVerifiedSaves = online;
            _driver.SaveRequested -= OnSaveRequested;
            _driver.SaveRequested += OnSaveRequested;
            _title?.Dismiss();
            _title = null;
        }

        /// PUSH NOW, and wait for it. A market action is settled by the server on a VERIFIED save,
        /// so the screen has to push before it refetches — otherwise it refetches a book that does
        /// not yet contain the order it just placed. Same reason `sim_driver_core.gd` force-pushes
        /// after a list or a buy, and it goes through the same `_pushing` guard: two overlapping
        /// pushes race on one pending anchor.
        ///
        /// `done(ok, why)`. A dev session answers false immediately — there is nothing to push to.
        public Coroutine PushNow(Action<bool, string> done)
        {
            if (!Online || _driver == null || _pushing || !OnlineService.Instance.Ready)
            {
                done?.Invoke(false, _pushing ? "a save is already in flight" : "not online");
                return null;
            }
            var bundle = _driver.Session != null ? _driver.Session.ExportLog() : null;
            if (string.IsNullOrEmpty(bundle)) { done?.Invoke(false, "nothing to push"); return null; }
            _pushing = true;
            return StartCoroutine(PushAndReport(bundle, done));
        }

        IEnumerator PushAndReport(string bundle, Action<bool, string> done)
        {
            var client = OnlineService.Instance.Client;
            var session = _driver.Session;
            if (client == null) { _pushing = false; done?.Invoke(false, "not online"); yield break; }
            bool ok = false; string why = "";
            yield return client.PutSave(bundle, (o, resp, msg) => { ok = o; why = msg; });
            _pushing = false;
            if (_driver == null || !ReferenceEquals(_driver.Session, session)
                || !ReferenceEquals(OnlineService.Instance.Client, client))
            { done?.Invoke(false, "session changed"); yield break; }
            if (ok) { session.CommitAnchor(); _failedPushes = 0; Roi.UI.SessionNotice.Clear(); }
            else if (client.LastSaveStatus == 401) Superseded();
            done?.Invoke(ok, why);
        }

        void OnSaveRequested(string bundle)
        {
            if (!Online)
            {
                // No server to verify it, so the anchor moves anyway — otherwise a dev session's
                // log grows without bound. This is the one place a dev session differs, and it is
                // also why nothing it produces would be accepted.
                _driver.CommitAnchor();
                return;
            }
            // One push at a time. Two overlapping pushes race on the same pending anchor —
            // `_pushing` is the same guard `sim_driver_core.gd` keeps for the same reason.
            if (_pushing || !OnlineService.Instance.Ready) return;
            _pushing = true;
            StartCoroutine(Push(bundle));
        }

        IEnumerator Push(string bundle)
        {
            var client = OnlineService.Instance.Client;
            var session = _driver.Session;
            if (client == null) { _pushing = false; yield break; }
            bool ok = false; string why = ""; long code = 0;
            yield return client.PutSave(bundle, (o, resp, msg) =>
            {
                ok = o; why = msg; code = client.LastSaveStatus;
            });
            _pushing = false;

            if (_driver == null || !ReferenceEquals(_driver.Session, session)
                || !ReferenceEquals(OnlineService.Instance.Client, client)) yield break;

            if (ok)
            {
                // ONLY NOW. The anchor moves when the server has verified the segment, so the next
                // push carries only what happened after it.
                _driver.CommitAnchor();
                _failedPushes = 0;
                Roi.UI.SessionNotice.Clear();
                yield break;
            }
            if (code == 401)
            {
                // SUPERSEDED: this account signed in somewhere else and the server kicked this
                // token. Keeping the sim running would pile up a log nothing will ever accept.
                Superseded();
                yield break;
            }
            // A rejection leaves the anchor where it is, so the same segment is re-sent next time
            // rather than being silently dropped. The reason is the server's own words.
            Debug.LogError($"SAVE REJECTED: {why}");
            // AND THE PLAYER IS TOLD. Silence is the worst outcome an idle game has: hours
            // accumulate against a save nothing will accept, and the loss is discovered on the
            // next resume. One failure can be a blip; a run of them is a session going nowhere.
            _failedPushes++;
            Roi.UI.SessionNotice.Warn(_failedPushes == 1
                ? "Not saved — retrying"
                : $"Not saved for {_failedPushes} attempts. Your progress is not being kept.");
        }

        int _failedPushes;

        /// The account signed in somewhere else and the server retired this token. The session
        /// STOPS rather than piling up a log nothing will ever accept — that is the whole reason
        /// this is a blocking panel and not a banner.
        void Superseded()
        {
            if (_driver != null) _driver.HoldPresentation(this);
            Debug.LogWarning("SESSION SUPERSEDED — this account is active on another device.");
            OnlineService.Instance.Disconnect();
            Roi.UI.SessionNotice.Warn("This session is no longer active.");
            Roi.UI.SessionNotice.Ended(
                "Session ended",
                "This account is now active on another device. Nothing more will be saved from "
                + "here; sign in again to continue on this one.",
                () => ReturnToLogin("This account was active on another device."));
        }

        /// Back to the title. A full scene reload rather than an unwind: the arena holds actors,
        /// the bar holds windows and the driver holds a native handle, and a reload retires all
        /// three at once. The UI's generated textures survive it — that is what their
        /// `HideAndDontSave` is for.
        public void ReturnToLogin(string message = null)
        {
            if (!string.IsNullOrEmpty(message)) PendingMessage = message;
            // The player is back at the door, so the client forgets the token it had. The next
            // sign-in supersedes it server-side anyway; dropping it here is what makes the title
            // screen's state honest.
            OnlineService.Instance.Disconnect();
            var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            UnityEngine.SceneManagement.SceneManager.LoadScene(scene.buildIndex);
        }

        /// Survives the reload (a static does) so the title screen can say WHY it is showing again.
        public static string PendingMessage;

        void OnEnable()
        {
            if (string.IsNullOrEmpty(PendingMessage)) return;
            StartCoroutine(ShowPending());
        }

        IEnumerator ShowPending()
        {
            yield return null;   // the title screen is built in Start
            _title?.SetStatus(PendingMessage);
            PendingMessage = null;
        }

        // ONLY `id`. The pivot's `classes` query answers nothing else — the six RO attributes it
        // used to carry are deleted from the sim, and a field left declared here would be filled
        // with 0 by JsonUtility rather than reported as missing, which is how six zero bars nearly
        // shipped on the class-creation screen.
        [Serializable] class ClassRow { public string id; }
        [Serializable] class ClassList { public ClassRow[] rows; }
    }
}
