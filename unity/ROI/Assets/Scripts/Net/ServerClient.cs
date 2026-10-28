// THE ONLINE LAYER — the Unity half of client-sim + server-VERIFY.
//
// The model (docs/server-plan-phases.md, GDD §4.4) is NOT server-authoritative: this client runs
// the simulation, and the server REPLAYS what it is sent to check it. So the whole of this file is
// three things — get a token, tell the server a session is starting, and hand it the log.
//
// FOUR THINGS THE SERVER WILL REFUSE, all of them silent-looking from the client if you get them
// wrong, and each one cost the Godot client a debugging session first:
//
//  1. THE SEED IS THE SERVER'S. It anchors the genesis state hash for the account, so a session
//     started on a client-chosen seed can never anchor and every save is rejected. It arrives as a
//     decimal STRING because it exceeds 2^53 and a JSON number would lose precision — parse it as
//     u64, never as a double.
//  2. GET /save FIRST. That handshake sets the server's tick anchor for the session. Skip it and
//     the anti-fast-forward check measures from now, bounding a legitimate session to a flat grace.
//  3. VERSION SKEW. The server reports its `sim_version` at LOGIN so a mismatch is caught before
//     playing rather than when the first save bounces. Both gates matter — the sim's version and
//     the catalog's.
//  4. THE TOKEN IS SINGLE-SESSION. A second login supersedes the first, and the old token starts
//     answering 401.
//
// `/auth/dev` is a DEVELOPMENT door and is gated behind a key the server refuses to run without.
// Real login is `/auth/steam` with an encrypted app ticket; everything downstream — token, seed,
// single-session, taint — is identical, which is the point of the placeholder.
using System;
using System.Collections;
using System.Collections.Generic;
using System.Text;
using UnityEngine;
using UnityEngine.Networking;

namespace Roi.Net
{
    public sealed class ServerClient
    {
        public string BaseUrl { get; }
        /// The dev-door key. Never needed for a Steam login; never shipped in a build.
        public string DevKey { get; set; }

        public string Token { get; private set; }
        public ulong Seed { get; private set; }
        public long AccountId { get; private set; }
        public string ServerSimVersion { get; private set; }
        public VersionInfo ServerVersion { get; private set; }
        [Serializable] public class VersionInfo
        { public string sim_version, game_data_version, sim_build; }
        [Serializable] class CatalogVersion { public string version; }

        public static VersionInfo LocalVersion() => new VersionInfo {
            sim_version = Roi.Sim.SimSession.SimVersion(),
            sim_build = Roi.Sim.SimSession.SimBuild(),
            game_data_version = ParseResponse<CatalogVersion>(Roi.Sim.SimSession.LoadCatalog())?.version
        };

        public static string CompatibilityError(VersionInfo local, VersionInfo server)
        {
            if (local == null || string.IsNullOrEmpty(local.sim_version) || string.IsNullOrEmpty(local.sim_build)
                || string.IsNullOrEmpty(local.game_data_version)) return Roi.UI.Loc.Get("UI_LOGIN_BUILD_INVALID");
            if (server == null || string.IsNullOrEmpty(server.sim_version) || string.IsNullOrEmpty(server.sim_build)
                || string.IsNullOrEmpty(server.game_data_version)) return Roi.UI.Loc.Get("UI_LOGIN_VERSION_UNAVAILABLE");
            return local.sim_version == server.sim_version && local.sim_build == server.sim_build
                && local.game_data_version == server.game_data_version ? "" : Roi.UI.Loc.Get("UI_LOGIN_UPDATE_REQUIRED");
        }

        // Check before authenticating: an incompatible client must not supersede a working login.
        public IEnumerator GetVersion(Action<VersionInfo, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/version");
            req.timeout = 30;
            yield return req.SendWebRequest();
            if (req.result != UnityWebRequest.Result.Success) { onDone?.Invoke(null, Describe(req)); yield break; }
            var version = ParseResponse<VersionInfo>(req.downloadHandler.text);
            onDone?.Invoke(version, version == null ? Roi.UI.Loc.Get("UI_LOGIN_VERSION_UNAVAILABLE") : "");
        }
        /// The HTTP status of the last `PutSave`. See the note there.
        public long LastSaveStatus { get; private set; }
        public bool LoggedIn => !string.IsNullOrEmpty(Token);

        public ServerClient(string baseUrl) => BaseUrl = baseUrl.TrimEnd('/');

        /// One batch of counter names. The response says how many the server's allow-list took;
        /// a name it does not know is not dropped silently, it lands on `client.unknown`, which is
        /// how a rename shows up instead of reading zero forever.
        public IEnumerator Telemetry(string[] events, Action<int, int> onDone)
        {
            if (events == null || events.Length == 0) { onDone?.Invoke(0, 0); yield break; }
            var sb = new System.Text.StringBuilder("{\"events\":[");
            for (int i = 0; i < events.Length; i++)
            {
                if (i > 0) sb.Append(',');
                sb.Append('"').Append(Escape(events[i])).Append('"');
            }
            sb.Append("]}");
            using var req = Post("/telemetry", sb.ToString());
            Authorize(req);
            yield return req.SendWebRequest();
            var body = req.downloadHandler != null ? req.downloadHandler.text : "";
            onDone?.Invoke(IntOf(body, "accepted"), IntOf(body, "rejected"));
        }

        static int IntOf(string json, string key)
        {
            int i = json.IndexOf("\"" + key + "\":", StringComparison.Ordinal);
            if (i < 0) return -1;
            i += key.Length + 3;
            int end = i;
            while (end < json.Length && char.IsDigit(json[end])) end++;
            return int.TryParse(json.Substring(i, end - i), out var v) ? v : -1;
        }

        /// A mission's FIRST-CLEAR reward. The client says which mission finished; the server
        /// decides whether that is worth anything, exactly once keyed on (account, mission), and
        /// delivers it as MAIL — the same road the pass takes. Nothing lands in the sim here: the
        /// mailbox screen applies it, so a replay sees a server-authored `ClaimMail` and verifies.
        /// A number written on this side would be a mint.
        ///
        /// Retrying is FREE and is the reason this is a button rather than an automatic call: the
        /// claim row makes a second press either pay the first time or say it already did.
        public IEnumerator StoryClaim(string missionId, Action<bool, long, string> onDone)
        {
            using var req = Post("/story/claim", "{\"mission_id\":\"" + Escape(missionId) + "\"}");
            Authorize(req);
            yield return req.SendWebRequest();
            var body = req.downloadHandler != null ? req.downloadHandler.text : "";
            if (req.result != UnityWebRequest.Result.Success && req.responseCode == 0)
            {
                onDone?.Invoke(false, 0, Describe(req));
                yield break;
            }
            bool ok = body.Contains("\"ok\":true");
            long gil = 0;
            int i = body.IndexOf("\"gil\":", StringComparison.Ordinal);
            if (i >= 0)
            {
                i += 6; int end = i;
                while (end < body.Length && char.IsDigit(body[end])) end++;
                long.TryParse(body.Substring(i, end - i), out gil);
            }
            onDone?.Invoke(ok, gil, ReasonOf(body));
        }

        /// The missions this ACCOUNT has cleared. The unlock chain is server state, not session
        /// state — a client that remembered it in a field would re-lock every mission on restart.
        public IEnumerator StoryProgress(Action<bool, string[]> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/story/progress");
            Authorize(req);
            yield return req.SendWebRequest();
            var body = req.downloadHandler != null ? req.downloadHandler.text : "";
            if (!body.Contains("\"ok\":true")) { onDone?.Invoke(false, new string[0]); yield break; }
            var list = new System.Collections.Generic.List<string>();
            int i = body.IndexOf("\"cleared\":[", StringComparison.Ordinal);
            if (i >= 0)
            {
                int end = body.IndexOf(']', i);
                foreach (var part in body.Substring(i + 11, end - i - 11).Split(','))
                {
                    var t = part.Trim().Trim('"');
                    if (t.Length > 0) list.Add(t);
                }
            }
            onDone?.Invoke(true, list.ToArray());
        }

        /// DEV ONLY — mail this account some Gil.
        ///
        /// The way a probe funds an ONLINE account. It cannot cheat: the sim refuses every debug
        /// edit the moment the client logs in, because a cheated save stops verifying — which is
        /// the rule working, not a limitation to route around. Mail is the real grant path (a
        /// server-authored payload the client claims and the next save settles), so a probe using
        /// it is exercising production wiring rather than sidestepping it.
        public IEnumerator DevMailGil(long gil, Action<bool> onDone)
        {
            var body = "{\"to_account\":" + AccountId + ",\"kind\":\"Reward\",\"gil\":" + gil
                     + ",\"reason\":\"probe\"}";
            using var req = Post("/dev/mail-send", body);
            if (!string.IsNullOrEmpty(DevKey)) req.SetRequestHeader("X-Dev-Key", DevKey);
            yield return req.SendWebRequest();
            onDone?.Invoke(req.result == UnityWebRequest.Result.Success);
        }

        /// DEV ONLY — put Crystals in this account's server-side wallet.
        ///
        /// The wallet is the server's, so a probe cannot seed it the way it seeds a bag. The route
        /// is dev-gated and refuses on production, which is the point: this exists so the summon
        /// probe can fund its own pull instead of assuming an earlier run left a balance behind.
        public IEnumerator DevGrantCrystals(long amount, Action<bool> onDone)
        {
            var body = "{\"account_id\":" + AccountId + ",\"tradeable\":0,\"bound\":" + amount + "}";
            using var req = Post("/dev/gacha-crystal", body);
            if (!string.IsNullOrEmpty(DevKey)) req.SetRequestHeader("X-Dev-Key", DevKey);
            yield return req.SendWebRequest();
            onDone?.Invoke(req.result == UnityWebRequest.Result.Success);
        }

        /// The development login. `onDone(ok, message)` — a failure message is the server's own
        /// where there is one, because "could not log in" tells nobody anything.
        /// THE REAL LOGIN. A Steam Web-API auth ticket, hex, validated by the SERVER against
        /// `ISteamUserAuth/AuthenticateUserTicket` with the publisher key — which is why the
        /// steamid this client knows is never sent: the server derives it from the ticket, and a
        /// client-supplied one would be a number anyone can read off a profile URL.
        ///
        /// Everything past the answer is identical to the dev door — same token, same seed, same
        /// single-session pinning — because the account is the same shape whichever door opened it.
        public IEnumerator LoginSteam(string ticketHex, Action<bool, string> onDone)
        {
            if (string.IsNullOrEmpty(ticketHex)) { onDone?.Invoke(false, "no Steam ticket"); yield break; }
            using var req = Post("/auth/steam", "{\"ticket\":\"" + Escape(ticketHex) + "\"}");
            yield return req.SendWebRequest();
            var body = req.downloadHandler != null ? req.downloadHandler.text : "";
            if (req.result != UnityWebRequest.Result.Success)
            {
                // The server's own words, because they are all worth reading: 503 means
                // STEAM_PUBLISHER_KEY is unset on that deploy, 403 means the ticket was rejected
                // or the account does not own the game. "Login failed" would hide which.
                var why = ErrorOf(body);
                onDone?.Invoke(false, string.IsNullOrEmpty(why) ? Describe(req) : why);
                yield break;
            }
            var resp = ParseResponse<AuthResp>(body);
            if (resp == null || string.IsNullOrEmpty(resp.token))
            {
                onDone?.Invoke(false, "steam login answered no token: " + body);
                yield break;
            }
            if (!ulong.TryParse(resp.seed, out var parsedSeed) || parsedSeed == 0UL)
            {
                onDone?.Invoke(false, "seed did not parse: " + resp.seed);
                yield break;
            }
            Token = resp.token;
            AccountId = resp.account_id;
            ServerSimVersion = resp.sim_version;
            ServerVersion = new VersionInfo { sim_version=resp.sim_version, game_data_version=resp.game_data_version, sim_build=resp.sim_build };
            Seed = ulong.TryParse(resp.seed, out var s) ? s : 0UL;
            onDone?.Invoke(Seed != 0UL, Seed != 0UL ? "ok" : "seed did not parse: " + resp.seed);
        }

        /// The `_error` the AUTH routes answer with. Separate from `reason`, which the economy
        /// routes use — two shapes, and reading the wrong one prints an empty string.
        static string ErrorOf(string body)
        {
            int i = body.IndexOf("\"_error\":\"", StringComparison.Ordinal);
            if (i < 0) return "";
            i += 10;
            int end = body.IndexOf('"', i);
            return end > i ? body.Substring(i, end - i) : "";
        }

        public IEnumerator LoginDev(string name, Action<bool, string> onDone)
        {
            var body = "{\"name\":\"" + Escape(name) + "\"}";
            using var req = Post("/auth/dev", body);
            if (!string.IsNullOrEmpty(DevKey)) req.SetRequestHeader("X-Dev-Key", DevKey);
            yield return req.SendWebRequest();
            if (req.result != UnityWebRequest.Result.Success)
            {
                onDone?.Invoke(false, Describe(req));
                yield break;
            }
            var resp = ParseResponse<AuthResp>(req.downloadHandler.text);
            if (resp == null || string.IsNullOrEmpty(resp.token))
            {
                onDone?.Invoke(false, "login answered no token: " + req.downloadHandler.text);
                yield break;
            }
            if (!ulong.TryParse(resp.seed, out var parsedSeed) || parsedSeed == 0UL)
            {
                onDone?.Invoke(false, "seed did not parse: " + resp.seed);
                yield break;
            }
            Token = resp.token;
            AccountId = resp.account_id;
            ServerSimVersion = resp.sim_version;
            ServerVersion = new VersionInfo { sim_version=resp.sim_version, game_data_version=resp.game_data_version, sim_build=resp.sim_build };
            // Decimal string -> u64. See note 1: a double loses the low bits and the genesis then
            // never matches, which looks like "the server hates my saves" rather than a parse bug.
            Seed = ulong.TryParse(resp.seed, out var s) ? s : 0UL;
            onDone?.Invoke(Seed != 0UL, Seed != 0UL ? "ok" : "seed did not parse: " + resp.seed);
        }

        /// The session handshake AND the resume: it sets the server's tick anchor and hands back
        /// this account's authoritative save, or nothing if the account has never stored one.
        /// `onDone(ok, stateJson, message)` — `stateJson` empty means a fresh account, which is not
        /// a failure and is the signal to found a party.
        ///
        /// What comes back is the state the SERVER holds, migrated to the current catalog on its
        /// side. That migration is the point of resuming from here rather than from a local file:
        /// the server re-trusts the migrated hash, so a resume still anchors across a balance patch.
        public IEnumerator BeginSession(Action<bool, string, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/save");
            Authorize(req);
            yield return req.SendWebRequest();
            if (req.responseCode == 404)
            {
                onDone?.Invoke(true, "", "no stored save (new account)");
                yield break;
            }
            if (req.result != UnityWebRequest.Result.Success)
            {
                onDone?.Invoke(false, "", Describe(req));
                yield break;
            }
            // `state_json` is a STRING field holding escaped JSON, so JsonUtility is what unescapes
            // it. Handing the caller the envelope instead would make every caller do that by hand.
            var stored = ParseResponse<StoredSave>(req.downloadHandler.text);
            if (stored == null || string.IsNullOrEmpty(stored.state_json))
            {
                onDone?.Invoke(false, "", "invalid stored save response");
                yield break;
            }
            onDone?.Invoke(true, stored.state_json, $"resume at tick {stored.final_tick}");
        }

        [Serializable]
        public class StoredSave
        {
            public string state_json;
            public long final_tick;
            public string final_hash;
        }

        /// Push the session's log. The body is `SimSession.ExportLog()` verbatim — a `SaveBundle`
        /// the server replays. On `ok` the caller MUST `CommitAnchor()`: that is what re-anchors
        /// the local session so the next push carries only what happened since.
        public IEnumerator PutSave(string bundleJson, Action<bool, SaveResp, string> onDone)
        {
            using var req = Post("/save", bundleJson);
            Authorize(req);
            yield return req.SendWebRequest();
            // The CODE, kept beside the answer. A 401 here is not a failed save — it means this
            // token was superseded by a login elsewhere, and the caller must stop the session
            // rather than retry it. That distinction is invisible in the body, which is empty.
            LastSaveStatus = req.responseCode;
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            var resp = ParseResponse<SaveResp>(text);
            bool ok = req.result == UnityWebRequest.Result.Success && resp != null && resp.ok;
            // A REJECTION is a normal answer, not a transport error: the server states its reason
            // and the client is expected to show it. Only a transport failure is described.
            var why = resp != null && !string.IsNullOrEmpty(resp.reason) ? resp.reason : Describe(req);
            onDone?.Invoke(ok, resp, ok ? "ok" : why);
        }

        static T ParseResponse<T>(string text) where T : class
        {
            if (string.IsNullOrWhiteSpace(text) || !text.TrimStart().StartsWith("{")) return null;
            try { return JsonUtility.FromJson<T>(text); }
            catch (ArgumentException) { return null; }
        }

        /// `GET /gacha/state` — the wallet, the pity ladders and the banners, as raw JSON. Handed
        /// back unparsed because the payload is deep and the SCREEN owns which parts it reads;
        /// a DTO here would be a second contract to keep in step with the server's.
        public IEnumerator GachaState(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/gacha/state");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /gacha/roll`.
        ///
        /// THE IDEMPOTENCY KEY IS THE POINT. A pull takes real money, so the server stores the
        /// result against the key: re-sending the SAME key returns the stored pulls and charges
        /// nothing. That is what makes a dropped reply safe to retry — and it is also how the
        /// two-step confirm works, because the quote and the confirmed roll are the same call with
        /// the same key.
        ///
        /// A 402 is NOT an error. It is a QUOTE: the account is short of coupons, nothing has been
        /// charged, and the client is expected to show the price and re-send with `buyShortfall`.
        /// `quotedCrystal` binds the server to the figure it quoted, so a price cannot move between
        /// the asking and the agreeing.
        public IEnumerator GachaRoll(string bannerId, int count, string idemKey,
                                     bool buyShortfall, ulong quotedCrystal,
                                     Action<RollOutcome> onDone)
        {
            var body = "{\"banner_id\":\"" + Escape(bannerId) + "\",\"count\":" + count
                     + ",\"idem_key\":\"" + Escape(idemKey) + "\""
                     + (buyShortfall ? ",\"buy_shortfall\":true" : "")
                     + (quotedCrystal > 0 ? ",\"quoted_crystal\":" + quotedCrystal : "")
                     + "}";
            using var req = Post("/gacha/roll", body);
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            var outcome = new RollOutcome { Code = req.responseCode, Json = text };
            if (req.responseCode == 402)
            {
                var q = ParseResponse<Shortfall>(text);
                outcome.NeedsConfirm = q != null;
                outcome.BuyCoupons = q != null ? q.buy : 0;
                outcome.Crystal = q != null ? q.crystal : 0;
            }
            else outcome.Ok = req.result == UnityWebRequest.Result.Success && ParseResponse<SaveResp>(text)?.ok == true;
            if (!outcome.Ok && !outcome.NeedsConfirm) outcome.Error = Describe(req);
            onDone?.Invoke(outcome);
        }

        /// `POST /fuse/roll` — ask the server to COMMIT to a fusion roll before the forge presses
        /// (design §7.8). `key` and `attempt` come from the sim's own `fuse_preview:` answer, never
        /// built here. The nonce comes back as a decimal STRING (a JSON number past 2^53 loses
        /// precision) and goes into `FuseGear.server_roll` verbatim. A refusal (tainted session,
        /// offline) means DO NOT press — never fall back to a client roll above the line, the
        /// verifier rejects that save.
        public IEnumerator FuseRoll(string key, ulong attempt, Action<bool, string, string> onDone)
        {
            var body = "{\"key\":\"" + Escape(key) + "\",\"attempt\":" + attempt + "}";
            using var req = Post("/fuse/roll", body);
            req.timeout = 20; // A pending forge must return control after a network outage.
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            var r = req.result == UnityWebRequest.Result.Success ? ParseResponse<FuseRollResp>(text) : null;
            if (r != null && r.issued && !string.IsNullOrEmpty(r.nonce) && r.nonce != "0")
                onDone?.Invoke(true, r.nonce, r.reason);
            else
                onDone?.Invoke(false, "0", r != null && !string.IsNullOrEmpty(r.reason) ? r.reason : Describe(req));
        }

        [Serializable] class FuseRollResp { public bool issued; public string nonce, reason; }

        public sealed class RollOutcome
        {
            public bool Ok, NeedsConfirm;
            public long Code;
            public long BuyCoupons;
            public ulong Crystal;
            public string Json, Error;
        }

        [Serializable] class Shortfall { public long have, need, buy; public ulong crystal; }

        /// `GET /daily/state` or `/weekly/state` — the same payload shape either way, which is why
        /// the screen that draws them is one screen. Handed back unparsed for the reason
        /// `GachaState` is: the SCREEN owns which parts it reads.
        public IEnumerator PeriodState(string period, Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get($"{BaseUrl}/{period}/state");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /daily/claim` or `/weekly/claim` — hands over every reached-but-unpaid rung in one
        /// press.
        ///
        /// The period id goes WITH the claim and the server refuses it unless it matches its own.
        /// That is the stale-screen guard: a client whose clock stopped overnight, or a window left
        /// open across the reset, would otherwise be paid yesterday's ladder from a screen showing
        /// yesterday's numbers. Sending the id the screen was DRAWN from is the whole point — do
        /// not re-read the clock here.
        public IEnumerator PeriodClaim(string period, string idField, long id,
                                       Action<bool, string> onDone)
        {
            var body = "{\"" + idField + "\":" + id + "}";
            using var req = Post($"/{period}/claim", body);
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /bp/state` — the season pass: level, XP, the 50-slot free track, and which slots
        /// this account has already claimed. Raw, for the reason `GachaState` is.
        public IEnumerator PassState(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/bp/state");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /bp/claim` — hand over every earned, unclaimed slot in one call (the server takes
        /// up to 64 and is idempotent per slot, so a retry pays nothing twice).
        ///
        /// The reply carries the reward MAILS it minted, and those are what the client applies to
        /// the sim — same production path as the gacha's: `ClaimMail` -> verified save. The mailbox
        /// is the safe transport, not a second inbox to visit.
        public IEnumerator PassClaim(IEnumerable<int> slots, Action<bool, string> onDone)
        {
            var sb = new StringBuilder("{\"slots\":[");
            bool first = true;
            foreach (var s in slots) { if (!first) sb.Append(','); sb.Append(s); first = false; }
            sb.Append("]}");
            using var req = Post("/bp/claim", sb.ToString());
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /commodity?def_id=` — the open sell orders for one stackable, cheapest first, as a
        /// raw JSON ARRAY. Wrapped by the caller: JsonUtility cannot parse a top-level array.
        public IEnumerator CommodityBook(string defId, Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/commodity?def_id="
                                              + UnityWebRequest.EscapeURL(defId));
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `GET /commodity/mine` — every open sell order this account owns, across commodities. A
        /// fully-sold-down item still appears, which is the point: it is the only place to cancel.
        public IEnumerator MyListings(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/commodity/mine");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /commodity/cancel` — pull YOUR order. A DIRECT server op, not a save push: the
        /// server verifies ownership and mails the unsold units back, so the units re-enter the
        /// save through the mailbox like any other server-authored grant.
        public IEnumerator CommodityCancel(long orderId, Action<bool, string> onDone)
        {
            using var req = Post("/commodity/cancel", "{\"order_id\":" + orderId + "}");
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /claim/state` — every login/event ladder that is OPEN for this account, each with
        /// its cells, its position and its own deadline. A LIST, because several tracks run at
        /// once by design.
        public IEnumerator ClaimState(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/claim/state");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /claim/collect` — hand over every reached-but-unpaid cell, across every open
        /// track. One press pays everything owed: a player who misses a day loses nothing, and
        /// asking for N presses when several events run at once is a worse screen, not a safer
        /// one. Exactly-once is the server's (`FOR UPDATE` + a claimed mask), so a second press
        /// pays nothing and that is not an error.
        ///
        /// Crystal lands in the account's BOUND bucket; items and Gil arrive as MAIL, which is why
        /// there is nothing to apply to the sim here — the mailbox screen finishes that.
        public IEnumerator ClaimCollect(Action<bool, string> onDone)
        {
            using var req = Post("/claim/collect", "{}");
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /item-market` — every open EQUIPMENT listing. Unlike the commodity book, each row
        /// is a unique instance at a seller-set price, so there is no matching: you buy a listing.
        public IEnumerator ItemMarket(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/item-market");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `GET /item-market/mine` — this account's own open item listings.
        public IEnumerator ItemMarketMine(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/item-market/mine");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /item-market/cancel` — pull your own listing; the escrowed item comes back as
        /// Return mail. Direct, like the commodity cancel and for the same reason.
        public IEnumerator ItemMarketCancel(long listingId, Action<bool, string> onDone)
        {
            using var req = Post("/item-market/cancel", "{\"order_id\":" + listingId + "}");
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /store` — the Crystal SKUs, every currency the server prices them in, and whether
        /// it is pointing at Steam's sandbox. No auth: it is a price list, not an account.
        public IEnumerator StoreCatalog(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/store");
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `GET /commander/state` — this account's POSITION only: level, CXP, what is claimable, the
        /// Outpost's accrued preview, and the skill grants to echo. The reward TRACK is not in it —
        /// the client renders that from its own byte-identical copy of `commander_rewards.json`.
        public IEnumerator CommanderState(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/commander/state");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// `POST /commander/claim` — accept the level rewards. `upTo` 0 claims every reached level;
        /// a level claims the track up to and including it.
        ///
        /// The Crystal is credited to the SERVER's wallet. What comes back for the client to apply
        /// is `grants` — skill unlocks, echoed into the sim as `GrantCommanderSkill` so they ride
        /// the next save, where the server re-verifies its own nonce. Same echo shape as a drop
        /// grant, and for the same reason: the client may not author a skill unlock by itself.
        public IEnumerator CommanderClaim(int upTo, Action<bool, string> onDone)
        {
            using var req = Post("/commander/claim", "{\"up_to\":" + upTo + "}");
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `POST /commander/outpost/claim` — bank the idle accrual into Commander XP. The server
        /// computes it from a stored timestamp, so the client never counts the time itself.
        public IEnumerator OutpostClaim(Action<bool, string> onDone)
        {
            using var req = Post("/commander/outpost/claim", "{}");
            Authorize(req);
            yield return req.SendWebRequest();
            var text = req.downloadHandler != null ? req.downloadHandler.text : "";
            bool ok = req.result == UnityWebRequest.Result.Success && text.Contains("\"ok\":true");
            onDone?.Invoke(ok, ok ? text : (string.IsNullOrEmpty(text) ? Describe(req) : text));
        }

        /// `GET /mail` — the inbox, as a raw JSON ARRAY. Wrapped by the caller, because
        /// JsonUtility cannot parse a top-level array.
        ///
        /// THERE IS NO CLAIM CALL HERE, and that is deliberate. The server's own note on
        /// `/mail/claim` says why: it "consumes REAL mailbox state out-of-band (no GameState
        /// credit) → value loss", so it is DEV-GATED and exists for tests. The production path is
        /// `ClaimMail -> verified save -> consume_claim`: the client applies the event to the sim
        /// from the inbox row it already has, and the next save is what settles it with the server.
        /// Calling the endpoint would take the mail away and credit nothing.
        public IEnumerator MailInbox(Action<bool, string> onDone)
        {
            using var req = UnityWebRequest.Get(BaseUrl + "/mail");
            Authorize(req);
            yield return req.SendWebRequest();
            bool ok = req.result == UnityWebRequest.Result.Success;
            onDone?.Invoke(ok, ok ? req.downloadHandler.text : Describe(req));
        }

        /// The contents of the object that follows `key`, with its outer braces removed, found by
        /// BRACE MATCHING rather than by a regex — the payload nests a full item and a regex would
        /// stop at the first inner brace.
        public static string InnerObject(string json, string key)
        {
            int at = json.IndexOf(key, StringComparison.Ordinal);
            if (at < 0) return "";
            int open = json.IndexOf('{', at + key.Length);
            if (open < 0) return "";
            int depth = 0;
            bool inStr = false, esc = false;
            for (int i = open; i < json.Length; i++)
            {
                char c = json[i];
                if (esc) { esc = false; continue; }
                if (c == '\\') { esc = true; continue; }
                if (c == '"') { inStr = !inStr; continue; }
                if (inStr) continue;
                if (c == '{') depth++;
                else if (c == '}' && --depth == 0)
                    return json.Substring(open + 1, i - open - 1);
            }
            return "";
        }

        UnityWebRequest Post(string path, string json)
        {
            var req = new UnityWebRequest(BaseUrl + path, "POST")
            {
                uploadHandler = new UploadHandlerRaw(Encoding.UTF8.GetBytes(json)),
                downloadHandler = new DownloadHandlerBuffer(),
                timeout = 30,
            };
            req.SetRequestHeader("Content-Type", "application/json");
            return req;
        }

        void Authorize(UnityWebRequest req)
        {
            if (req.timeout == 0) req.timeout = 30;
            if (!string.IsNullOrEmpty(Token)) req.SetRequestHeader("Authorization", "Bearer " + Token);
        }

        static string Describe(UnityWebRequest req)
        {
            var body = req.downloadHandler != null ? req.downloadHandler.text : "";
            if (body.Length > 300) body = body.Substring(0, 300);
            return $"HTTP {req.responseCode} {req.error} {body}".Trim();
        }

        /// Just enough for a name field. Not a JSON writer: everything else this class sends is
        /// already JSON the sim produced.
        /// The server's own words for why. A rejection is a normal answer here — "already claimed"
        /// is the honest result of a retry — so the reason is what the screen shows, not a generic
        /// failure line.
        static string ReasonOf(string body)
        {
            int i = body.IndexOf("\"reason\":\"", StringComparison.Ordinal);
            if (i < 0) return "";
            i += 10;
            int end = body.IndexOf('"', i);
            return end > i ? body.Substring(i, end - i) : "";
        }

        static string Escape(string s) => s.Replace("\\", "\\\\").Replace("\"", "\\\"");

        [Serializable]
        class AuthResp
        {
            public string token; public long account_id; public string name;
            public string seed; public string sim_version, game_data_version, sim_build;
        }

        [Serializable]
        public class SaveResp
        {
            public bool ok;
            public string reason;
            public long final_tick;
            public int daily_claimable_crystal;
        }
    }
}
