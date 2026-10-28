// Managed handle over one sim session (crates/sim-ffi). Owns the native pointer; Dispose frees it.
// Nothing here computes gameplay — it only forwards calls and copies JSON out (CLAUDE.md rule 2).
using System;

namespace Roi.Sim
{
    public sealed class SimSession : IDisposable
    {
        IntPtr _h;

        SimSession(IntPtr h) { _h = h; }

        public bool IsAlive => _h != IntPtr.Zero;

        public static SimSession New(ulong seed, string gameDataJson, string zoneId)
        {
            var h = SimFfi.roi_new(seed, gameDataJson, zoneId);
            return h == IntPtr.Zero ? null : new SimSession(h);
        }

        public static SimSession NewStarter(ulong seed, string gameDataJson, string zoneId, string partyName)
        {
            var h = SimFfi.roi_new_starter(seed, gameDataJson, zoneId, partyName);
            return h == IntPtr.Zero ? null : new SimSession(h);
        }

        public static SimSession NewAs(ulong seed, string gameDataJson, string zoneId, string unitDef, string partyName)
        {
            var h = SimFfi.roi_new_as(seed, gameDataJson, zoneId, unitDef, partyName);
            return h == IntPtr.Zero ? null : new SimSession(h);
        }

        public static SimSession Load(string saveJson, string gameDataJson)
        {
            var h = SimFfi.roi_load_json(saveJson, gameDataJson);
            return h == IntPtr.Zero ? null : new SimSession(h);
        }

        public void Tick(ulong n) { if (IsAlive) SimFfi.roi_tick(_h, n); }

        /// Exact disposable preview, without save migrations or catalog parsing.
        public SimSession Clone()
        {
            if (!IsAlive) return null;
            var h = SimFfi.roi_clone(_h);
            return h == IntPtr.Zero ? null : new SimSession(h);
        }

        /// serde-shaped InputEvent JSON, e.g. {"SetPartyName":{"name":"x"}}. False = did not parse.
        public bool ApplyInput(string eventJson) => IsAlive && SimFfi.roi_apply_input(_h, eventJson) == 1;

        public string ViewJson() => IsAlive ? SimFfi.TakeString(SimFfi.roi_view_json(_h)) : "{}";
        public string SaveJson() => IsAlive ? SimFfi.TakeString(SimFfi.roi_save_json(_h)) : null;

        /// One item collection as a JSON array, fetched when a screen opens rather than ridden
        /// along on every tick. An unknown key answers `[]`, so a screen for a list this build
        /// does not carry opens empty instead of failing.
        public string QueryJson(string key)
            => IsAlive ? SimFfi.TakeString(SimFfi.roi_query_json(_h, key)) : "[]";
        /// Apply a DEV CHEAT. Direct state edit, never an input — a cheated save stops verifying,
        /// which is the intended consequence. Returns false if refused (online) or unknown.
        public bool Debug(string cmd) => IsAlive && SimFfi.roi_debug_json(_h, cmd) == 1;

        /// Refuse the cheats. Call with false the moment the session goes online.
        public void SetDebugAllowed(bool allowed)
        {
            if (IsAlive) SimFfi.roi_set_debug_allowed(_h, allowed ? 1 : 0);
        }

        public string StateHash() => IsAlive ? SimFfi.TakeString(SimFfi.roi_state_hash(_h)) : "no_state";
        public string ExportLog() => IsAlive ? SimFfi.TakeString(SimFfi.roi_export_log(_h)) : null;
        public void CommitAnchor() { if (IsAlive) SimFfi.roi_commit_anchor(_h); }
        public long ReplaySpan() => IsAlive ? SimFfi.roi_replay_span(_h) : -1;

        /// Reads the catalog JSON shipped in StreamingAssets. Callers pass the result to New/NewAs/Load.
        public static string LoadCatalog()
        {
            var path = System.IO.Path.Combine(UnityEngine.Application.streamingAssetsPath, "game_data.json");
            return System.IO.File.Exists(path) ? System.IO.File.ReadAllText(path) : null;
        }

        public static string SimVersion() => SimFfi.TakeString(SimFfi.roi_sim_version());
        public static string SimBuild()
        {
            try { return SimFfi.TakeString(SimFfi.roi_sim_build()); }
            catch (EntryPointNotFoundException) { return ""; }
        }
        public static string SimGitStamp() => SimFfi.TakeString(SimFfi.roi_sim_git_stamp());

        public void Dispose()
        {
            if (_h != IntPtr.Zero) { SimFfi.roi_free(_h); _h = IntPtr.Zero; }
        }
    }
}
