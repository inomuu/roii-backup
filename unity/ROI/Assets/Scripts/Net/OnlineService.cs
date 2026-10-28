// THE ONE LOGGED-IN CLIENT, and the coroutine host that runtime UI needs.
//
// `ServerClient` is a plain object: it has no `StartCoroutine` and no idea when it is alive. A
// screen needs both, and every screen must talk to the SAME session — the server allows one active
// token per account, so a second login would silently 401 the first.
//
// The connection is EXPLICIT and offered by the screen that needs it, not made on startup. This
// client is a development one talking to a local server through `/auth/dev`; connecting quietly in
// the background is how a build ends up reaching for a server nobody meant it to reach for.
using System;
using System.Collections;
using UnityEngine;

namespace Roi.Net
{
    public sealed class OnlineService : MonoBehaviour
    {
        /// Normal play connects to Fly. Local test runs opt in with ROI_SERVER.
        public const string DefaultUrl = "https://renrei-idle-server.fly.dev";
        public static string ServerUrl
        {
            get
            {
#if UNITY_EDITOR || DEVELOPMENT_BUILD
                var configured = Environment.GetEnvironmentVariable("ROI_SERVER");
                if (!string.IsNullOrWhiteSpace(configured)) return configured.TrimEnd('/');
#endif
                return DefaultUrl;
            }
        }
        public const string DefaultDevKey = "roi-local-dev";

        static OnlineService _instance;
        public static OnlineService Instance
        {
            get
            {
                if (_instance != null) return _instance;
                var go = new GameObject("~Online");
                DontDestroyOnLoad(go);
                // HIDDEN from the scene, or the Editor reports it as a leak every time play mode
                // ends: a DontDestroyOnLoad object is still "in a scene" as far as the teardown
                // check is concerned, and it logs that at ERROR level — which a probe runner
                // counting errors then reads as a failed probe.
                go.hideFlags = HideFlags.HideAndDontSave;
                _instance = go.AddComponent<OnlineService>();
                return _instance;
            }
        }

        public ServerClient Client { get; private set; }
        public bool LoggedIn => Client != null && Client.LoggedIn;
        public string Status { get; private set; } = "offline";

        /// True once login AND the `/save` handshake have both landed. `LoggedIn` alone is not
        /// enough to start playing: without the handshake the server's anti-fast-forward clock is
        /// unanchored and a legitimate session is bounded to a flat grace.
        public bool Ready { get; private set; }

        /// The account's authoritative save, as the server holds it. Empty = a fresh account with
        /// a party still to found. Only meaningful once `Ready`.
        public string ResumeSaveJson { get; private set; } = "";

        /// Why the last connection attempt ended where it did — shown to the player as-is. The
        /// server's own words where there are any: "could not connect" tells nobody anything.
        public string LastError { get; private set; } = "";

        /// Raised whenever the connection state changes, so a screen can redraw without polling.
        public event Action Changed;

        /// The door a re-connect should use. The nine windows that offer a "connect" button are
        /// RE-connects — the player already chose a door at the title — so they must not pick one
        /// of their own, or pressing connect inside the Mail window would sign a Steam player in as
        /// somebody else. `SessionBoot` sets this from whatever door was actually opened; it starts
        /// at the dev door because that is what a scripted run and a machine with no Steam get, and
        /// because a wrong default here must fail loudly rather than sign someone in.
        public bool PreferredUseSteam { get; set; }
        Coroutine _login;
        bool _connecting;

        public void Connect(string name = "unity-dev")
        {
            if (Ready || _connecting) return;
            ConnectAndWait(name, PreferredUseSteam);
        }

        /// The same sequence, for a caller that must WAIT for it — the login screen, which cannot
        /// start a session until it knows whether there is a save to resume.
        public Coroutine ConnectAndWait(string name, bool useSteam)
        {
            if (_connecting) return _login;
            PreferredUseSteam = useSteam;
            _connecting = true;
            _login = StartCoroutine(LoginOnce(name, useSteam));
            return _login;
        }

        IEnumerator LoginOnce(string name, bool useSteam)
        {
            try { yield return Login(name, useSteam); }
            finally { _connecting = false; _login = null; }
        }

        IEnumerator Login(string name, bool useSteam)
        {
            Status = "connecting…"; Ready = false; ResumeSaveJson = ""; LastError = "";
            Changed?.Invoke();
            Client = new ServerClient(ServerUrl) { DevKey = DefaultDevKey };
            bool done = false;
            var localVersion = ServerClient.LocalVersion();
            yield return Client.GetVersion((version, error) =>
                LastError = string.IsNullOrEmpty(error) ? ServerClient.CompatibilityError(localVersion, version) : error);
            if (!string.IsNullOrEmpty(LastError))
            {
                Status = "login failed: " + LastError;
                Client = null;
                Changed?.Invoke();
                yield break;
            }

            // THE ROUTE IS THE CALLER'S, not this method's. It used to be decided here — Steam
            // whenever the SDK reported itself available — and that one line quietly took the
            // choice away from everybody: the day the Steam package landed, every screen that
            // needed a login moved to the Steam door at once, and a machine with no publisher key
            // on its server could not sign in at all. Seven probes went red together for a reason
            // that had nothing to do with any of them.
            //
            // So the doors are named by whoever opens them (`SessionBoot`, from the title screen or
            // the scripted `ROI_BOOT`), and this reports what it was asked to do rather than
            // guessing. What does NOT change: a ticket Steam refuses does not fall through to the
            // dev door — signing in as somebody else because Steam had a bad minute is worse than
            // not signing in — and asking for Steam on a machine that has none is an error here,
            // not a silent downgrade.
            if (useSteam)
            {
                if (!SteamAuth.Available)
                {
                    Status = "login failed: " + SteamAuth.Status;
                    LastError = "Steam sign-in was asked for, but Steam is not available: " + SteamAuth.Status;
                    Changed?.Invoke();
                    yield break;
                }
                string ticket = "";
                yield return SteamAuth.GetWebTicket(t => ticket = t);
                if (string.IsNullOrEmpty(ticket))
                {
                    Status = "login failed: " + SteamAuth.Status;
                    LastError = SteamAuth.Status;
                    Changed?.Invoke();
                    yield break;
                }
                yield return Client.LoginSteam(ticket, (ok, msg) =>
                {
                    Status = ok ? $"connected as account {Client.AccountId}" : "login failed: " + msg;
                    if (!ok) LastError = msg;
                    done = true;
                });
            }
            else
            {
                yield return Client.LoginDev(name, (ok, msg) =>
                {
                    Status = ok ? $"connected as account {Client.AccountId}" : "login failed: " + msg;
                    if (!ok) LastError = msg;
                    done = true;
                });
            }
            while (!done) yield return null;

            if (LoggedIn)
            {
                // THE VERSION GATE, at login and not at the first save. A skewed client can never
                // sync — every push would bounce with a reason that reads like a bug in the save
                // rather than a mismatched build — so it is blocked before a session exists to
                // waste. This is `sim_driver_core.gd`'s check, in the same place in the sequence.
                // Recheck the authenticated response too, in case a deploy happened after /version.
                var mismatch = ServerClient.CompatibilityError(localVersion, Client.ServerVersion);
                if (!string.IsNullOrEmpty(mismatch))
                {
                    LastError = mismatch;
                    Status = "login failed: " + LastError;
                    Client = null;
                    Changed?.Invoke();
                    yield break;
                }

                // CHEATS OFF the moment we are online. A direct state edit is absent from the input
                // log, so a save touched by one stops verifying and every push after it is
                // rejected. Refusing up front beats discovering a whole session is unpushable.
                var driver = FindFirstObjectByType<Roi.Sim.SimDriver>();
                driver?.Session?.SetDebugAllowed(false);

                // TELEMETRY follows the session. It buffers happily while offline — that is the
                // normal state before a login — and only sends once there is a token to send with.
                // Steam's callbacks need pumping for the life of the session, not just during the
                // ticket fetch — an unpumped SDK stops answering everything else too.
                // ONE pump, however many times this runs. A second login would otherwise add a
                // second component: both call RunCallbacks every frame, and destroying either one
                // shuts the SDK down under the other.
                if (SteamAuth.Available && GetComponent<SteamPump>() == null)
                    gameObject.AddComponent<SteamPump>();
                Telemetry.Create(transform);
                Telemetry.Attach(Client);
                Telemetry.Note("session.start");

                // The handshake sets the server's tick anchor. Without it the anti-fast-forward
                // check measures from now and bounds a legitimate session to a flat grace.
                bool handshake = false;
                yield return Client.BeginSession((ok, save, msg) =>
                {
                    handshake = ok;
                    ResumeSaveJson = ok ? save : "";
                    if (!ok) LastError = msg;
                    Status += ok ? $" · session ready ({msg})" : $" · handshake failed: {msg}";
                });
                Ready = handshake;
            }
            Changed?.Invoke();
        }

        /// Drop the session. The token stays valid server-side until it is superseded or expires —
        /// this is the CLIENT forgetting it, which is what a return to the login screen is.
        public void Disconnect()
        {
            if (_login != null) StopCoroutine(_login);
            _login = null;
            _connecting = false;
            // Other coroutines are deliberately LEFT running. This object hosts every screen's
            // requests as well as its own, and stopping them all would abort work that has nothing
            // to do with the connection — an in-flight push holds its own client reference and
            // finishes against a token the server has already retired, which is harmless.
            Client = null;
            Ready = false;
            ResumeSaveJson = "";
            Status = "offline";
            Changed?.Invoke();
        }

        /// Run one of the client's coroutines from a screen that has no MonoBehaviour of its own.
        public Coroutine Run(IEnumerator routine) => StartCoroutine(routine);
    }

    /// One component whose only job is to keep Steam's callback queue moving. Its own type so the
    /// pump cannot be forgotten by a refactor of whatever else happened to hold it.
    public sealed class SteamPump : MonoBehaviour
    {
        void Update()
        {
            SteamAuth.Pump();
            // The pump outliving Steam is the state that produced an exception per frame. Pump()
            // switches itself off when the dispatcher goes; this is the component doing the same,
            // so a dead pump does not sit in the scene waiting to be handed a second Steam session
            // it was never re-initialised for.
            if (!SteamAuth.Available) Destroy(this);
        }

        void OnDestroy() => SteamAuth.Shutdown();
    }
}
