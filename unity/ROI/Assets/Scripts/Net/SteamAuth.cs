// STEAM, behind one door. Ported from godot/net/steam_auth.gd.
//
// It asks Steam for a WEB-API auth ticket and hands the hex to `/auth/steam`, which validates it
// with the publisher key and derives the steamid itself. The steamid this client knows is never
// sent: it is a public number anyone can read off a profile URL, and trusting it would make the
// login a formality.
//
// COMPILED OUT BY DEFAULT. The SDK binding is a package this project does not carry, so every call
// into it sits behind `ROI_STEAM` and the client builds either way. Without the symbol `Available`
// is false and the game falls back to the development login — the same graceful path the Godot
// version takes when its addon is missing, and what lets the game boot on a machine with no Steam.
//
// TO TURN IT ON
//   1. Add Steamworks.NET as a package. Pick a version built on Steamworks SDK 1.57 or newer:
//      `GetAuthTicketForWebApi` does not exist before that, and the older `GetAuthSessionTicket`
//      is the SDK-to-SDK path, which the Web API rejects.
//   2. Put `steam_appid.txt` containing 4958230 beside the project (for the Editor) and beside the
//      built player.
//   3. Add ROI_STEAM to Scripting Define Symbols.
//   4. Set STEAM_PUBLISHER_KEY on the server, or `/auth/steam` answers 503 and says so.
using System;
using System.Collections;
using UnityEngine;

namespace Roi.Net
{
    public static class SteamAuth
    {
        /// Not a secret — it is in the store URL. Kept beside the ticket code rather than in a
        /// config file: the SERVER pins the same number when it validates, and two values that
        /// must agree are better one line apart than one file apart.
        public const uint AppId = 4958230;

        /// A stuck Steam client must not hang the login forever. The same five seconds the Godot
        /// version races its callback against.
        const float TicketTimeout = 5f;

        /// True only when this build carries the SDK *and* Steam is running and initialised.
        public static bool Available { get; private set; }
        /// The Steam display name, for pre-filling the party-name field. Never sent as identity.
        public static string Persona { get; private set; } = "";
        /// Why it is unavailable, in the server's spirit: a reason beats a bare false.
        public static string Status { get; private set; } = "built without the Steam SDK";

        /// Raised ONCE when Steam goes away under a running game — the player quit the client, or
        /// the IPC pipe broke. Whoever owns the session decides what that means; this only reports
        /// it, because "what happens to an in-progress party" is not a networking question.
        public static event Action Lost;

        /// Every road to "Steam is gone" ends here, so the state change and the announcement cannot
        /// drift apart: the callback below is the SDK telling us, and `Pump`'s catch is us finding
        /// out the hard way. Idempotent, because both can fire for the same loss.
        static void MarkLost(string why)
        {
            if (!Available) return;
            Available = false;
            Status = why;
            UnityEngine.Debug.LogWarning("STEAM: " + why);
            try { Lost?.Invoke(); }
            catch (Exception e) { UnityEngine.Debug.LogWarning("STEAM: a Lost handler threw — " + e.Message); }
        }

        public static bool Init()
        {
#if ROI_STEAM
            try
            {
                if (!Steamworks.SteamAPI.Init())
                {
                    Status = "Steam is not running, or steam_appid.txt is not beside the player";
                    Available = false;
                    return false;
                }
                Available = true;
                Status = "ready";
                Persona = Steamworks.SteamFriends.GetPersonaName();
                // THE SDK'S OWN WARNING SHOT. `IPCFailure_t` is Steam saying the pipe to the client
                // is finished — it fires BEFORE the next write turns into `CClientPipe ... Fatal
                // assert; application exiting`, which is a process death no handler can catch. It
                // is the only chance there is to end the session on our terms instead of Steam's,
                // and holding the callback in a static is what keeps it alive to be delivered.
                _ipcFailure ??= Steamworks.Callback<Steamworks.IPCFailure_t>.Create(
                    _ => MarkLost("the connection to the Steam client was lost"));
                return true;
            }
            catch (DllNotFoundException e)
            {
                // The NATIVE library is missing, which is our packaging — not the player having
                // Steam closed. Worth telling the two apart in the one line anyone will read.
                Status = "the Steam native library is missing from this build: " + e.Message;
                Available = false;
                return false;
            }
#else
            Available = false;
            return false;
#endif
        }

#if ROI_STEAM
        static Steamworks.Callback<Steamworks.IPCFailure_t> _ipcFailure;
        static Steamworks.HAuthTicket _live = Steamworks.HAuthTicket.Invalid;

        /// Hand back the one outstanding ticket handle, if there is one.
        static void Release()
        {
            if (_live == Steamworks.HAuthTicket.Invalid) return;
            try { Steamworks.SteamUser.CancelAuthTicket(_live); } catch { /* the pipe may be gone */ }
            _live = Steamworks.HAuthTicket.Invalid;
        }
#endif

        public static void Shutdown()
        {
#if ROI_STEAM
            if (Available) { Release(); Steamworks.SteamAPI.Shutdown(); }
#endif
            Available = false;
        }

        /// Callbacks have to be pumped or the ticket response never arrives. A no-op without Steam.
        ///
        /// AND IT HAS TO SURVIVE STEAM GOING AWAY. `Available` is a static, and with the Editor's
        /// domain reload disabled a static outlives the play-mode session that set it — so a run
        /// that ended without a clean `Shutdown` (a probe killed on timeout, a crash) leaves this
        /// saying "yes" over a dispatcher that has already been torn down. `RunCallbacks` then
        /// throws EVERY FRAME: one abandoned probe poisoned the twenty after it with 5,520 copies
        /// of the same exception, and none of those probes had anything to do with Steam.
        ///
        /// A shipped build reaches the same state by the other road — the player quits Steam. So
        /// the failure is caught, said once, and the pump switched off, rather than left to shout.
        /// This is the soft half of the same problem the SDK handles by killing the process
        /// (`CClientPipe ... Fatal assert`); catching it here cannot stop that, but it does stop
        /// the client from turning a finished Steam session into an endless error.
        public static void Pump()
        {
#if ROI_STEAM
            if (!Available) return;
            try { Steamworks.SteamAPI.RunCallbacks(); }
            catch (System.Exception e)
            {
                MarkLost("callbacks stopped, the pump is off — " + e.Message);
            }
#endif
        }

        /// A Web-API auth ticket as HEX, or "" on failure, timeout, or no Steam.
        ///
        /// The identity string is EMPTY and has to stay that way: the server calls
        /// `AuthenticateUserTicket` with no `identity` parameter at all (an empty one makes Steam
        /// answer error 104), and a ticket minted for a different identity is refused with nothing
        /// on either side saying why.
        public static IEnumerator GetWebTicket(Action<string> onDone)
        {
            if (!Available) { onDone?.Invoke(""); yield break; }
#if ROI_STEAM
            byte[] ticket = null;
            uint size = 0;
            var result = Steamworks.EResult.k_EResultPending;
            var handle = Steamworks.HAuthTicket.Invalid;

            var cb = Steamworks.Callback<Steamworks.GetTicketForWebApiResponse_t>.Create(r =>
            {
                result = r.m_eResult;
                size = (uint)r.m_cubTicket;
                ticket = r.m_rgubTicket;
                handle = r.m_hAuthTicket;
            });
            Steamworks.SteamUser.GetAuthTicketForWebApi(null);

            float t0 = Time.realtimeSinceStartup;
            while (ticket == null && Time.realtimeSinceStartup - t0 < TicketTimeout)
            {
                Pump();
                yield return null;
            }
            cb.Dispose();

            if (ticket == null || result != Steamworks.EResult.k_EResultOK)
            {
                Status = $"Steam refused the ticket ({result})";
                onDone?.Invoke("");
                yield break;
            }
            // Only `size` bytes ARE the ticket — the buffer is fixed-length and its tail is noise.
            var hex = new System.Text.StringBuilder((int)size * 2);
            for (int i = 0; i < size; i++) hex.Append(ticket[i].ToString("x2"));
            // NOT CANCELLED HERE, which is the whole point of this comment. Valve does ask for the
            // handle back, and it used to be released on this line — one statement before the hex
            // was handed to a caller that had not yet sent it anywhere. Cancelling a ticket
            // invalidates it for `AuthenticateUserTicket`, so every login was a RACE between our
            // own cancel propagating and the server getting its answer from Steam. It won for
            // weeks and then lost, and the symptom is maximally misleading: a ticket that was
            // minted perfectly (234 bytes, EResultOK) and comes back "steam ticket rejected", which
            // reads as a bad key, a wrong appid, or a server bug — anything but the client
            // throwing the ticket away on the way out.
            //
            // So the handle is kept and released when the NEXT one is minted, or at Shutdown. That
            // is still bounded — one live handle, not one per login — and it cannot be released
            // before the ticket it belongs to has been used, because minting the next one is the
            // proof the last one is finished with.
            Release();
            _live = handle;
            onDone?.Invoke(hex.ToString());
#else
            onDone?.Invoke("");
            yield break;
#endif
        }
    }
}
