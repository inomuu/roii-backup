// THE ONLINE ROUND TRIP, proved end to end and without a scene.
//
// This is the milestone, not the screens: if a save produced by the UNITY client replays cleanly
// inside the server, everything server-dependent is a UI job on top. If it does not, we learn the
// reason here rather than after building ten screens against it. It is the same shape of gate the
// P0 work used when it compared state hashes across the two engines.
//
// The sequence is the one the server's own comments require, in order:
//   1. POST /auth/dev   -> token + the SERVER'S seed + the server's sim_version
//   2. compare versions -> a skew means every save would bounce; say so now, not later
//   3. GET  /save       -> the handshake that sets the server's tick anchor
//   4. new session ON THE SERVER'S SEED, ticked forward
//   5. export_log       -> POST /save -> the server REPLAYS it
//   6. commit_anchor    -> only on ok, because that is what re-anchors the local log
//
//   ROI_SERVER / ROI_DEV_KEY override the defaults.
//   echo exec:OnlineProbe.Run > unity/ROI/.roi_cmd
using System;
using System.Collections;
using UnityEditor;
using UnityEngine;

public static class OnlineProbe
{
    const string DefaultUrl = "http://127.0.0.1:8787";
    const string DefaultKey = "roi-local-dev";
    const int Ticks = 300;   // 30 s of sim; enough to have a log worth replaying

    public static void Run()
    {
        var url = Environment.GetEnvironmentVariable("ROI_SERVER") ?? DefaultUrl;
        var key = Environment.GetEnvironmentVariable("ROI_DEV_KEY") ?? DefaultKey;
        EditorCoroutine.Start(Probe(url, key));
    }

    static IEnumerator Probe(string url, string key)
    {
        Debug.Log($"ONLINE probe -> {url}");
        var client = new Roi.Net.ServerClient(url) { DevKey = key };

        bool done = false, ok = false; string msg = "";
        yield return client.LoginDev("unity-probe", (o, m) => { ok = o; msg = m; done = true; });
        while (!done) yield return null;
        if (!ok) { Debug.LogError($"ONLINE login FAILED: {msg}"); yield break; }
        Debug.Log($"ONLINE login ok: account={client.AccountId} seed={client.Seed} serverSim={client.ServerSimVersion}");

        // The version gate BEFORE playing. The server checks two — its sim version and its
        // catalog's — and a mismatch on either rejects every save with a reason that looks like a
        // bug in the save rather than a mismatched build.
        var localSim = Roi.Sim.SimSession.SimVersion();
        if (localSim != client.ServerSimVersion)
        {
            Debug.LogError($"ONLINE VERSION SKEW: local sim {localSim} vs server {client.ServerSimVersion}. "
                         + "Every save would be rejected. Rebuild the plugin or redeploy the server.");
            yield break;
        }
        Debug.Log($"ONLINE version ok: {localSim}");

        done = false; string stored = "";
        yield return client.BeginSession((o, s, m) => { ok = o; stored = s; msg = m; done = true; });
        while (!done) yield return null;
        if (!ok) { Debug.LogError($"ONLINE handshake FAILED: {msg}"); yield break; }
        Debug.Log($"ONLINE handshake ok ({msg}); stored save {stored.Length} bytes");

        var catalog = Roi.Sim.SimSession.LoadCatalog();
        if (string.IsNullOrEmpty(catalog)) { Debug.LogError("ONLINE: no catalog in StreamingAssets"); yield break; }

        // ON THE SERVER'S SEED. This is the whole reason login returns one: the account's genesis
        // hash is anchored from it, and a session on any other seed can never anchor.
        using var sim = Roi.Sim.SimSession.NewAs(client.Seed, catalog, "zone_b1_1",
                                                 "swordsman_t1", "unity-probe");
        if (sim == null || !sim.IsAlive) { Debug.LogError("ONLINE: could not start a session"); yield break; }
        Debug.Log($"ONLINE session start hash={sim.StateHash()}");

        sim.Tick((ulong)Ticks);
        var bundle = sim.ExportLog();
        if (string.IsNullOrEmpty(bundle)) { Debug.LogError("ONLINE: export_log produced nothing"); yield break; }
        Debug.Log($"ONLINE bundle {bundle.Length} bytes at tick {Ticks}, hash={sim.StateHash()}");

        done = false; Roi.Net.ServerClient.SaveResp resp = null;
        yield return client.PutSave(bundle, (o, r, m) => { ok = o; resp = r; msg = m; done = true; });
        while (!done) yield return null;
        if (!ok)
        {
            // The reason is the server's own words. It is the useful half of a rejection.
            Debug.LogError($"ONLINE save REJECTED: {msg}");
            yield break;
        }
        // Only now: the anchor moves, so the next push carries only what happened after this one.
        sim.CommitAnchor();
        Debug.Log($"ONLINE SAVE VERIFIED by replay — final_tick={resp.final_tick}. "
                + "The Unity client's log reproduces inside the server.");
    }

    /// A coroutine runner for the Editor, which has no MonoBehaviour to host one.
    ///
    /// It keeps a STACK, and that is the whole substance of it: `yield return SomeCoroutine()`
    /// hands back an IEnumerator that nothing will advance unless the runner descends into it.
    /// A version that only understood AsyncOperation walked straight past every nested call — the
    /// login fired its request and the outer routine then waited forever on a flag the inner half
    /// never got to set. Unity's own StartCoroutine does this descent; outside play mode there is
    /// no Unity doing it for you.
    sealed class EditorCoroutine
    {
        readonly System.Collections.Generic.Stack<IEnumerator> _stack = new();

        EditorCoroutine(IEnumerator r) { _stack.Push(r); }

        public static void Start(IEnumerator r)
        {
            var c = new EditorCoroutine(r);
            EditorApplication.update += c.Step;
        }

        void Step()
        {
            // FINISHED — including the seven `yield break` exits above, which is why the call
            // lives here and not at each of them. This probe works in EDIT MODE, so leaving play
            // mode cannot signal its end and `ProbeWatch` would wait out the runner's timeout.
            if (_stack.Count == 0)
            {
                EditorApplication.update -= Step;
                RemoteControl.ProbeWatch.Done();
                return;
            }
            var top = _stack.Peek();

            // A yielded UnityWebRequestAsyncOperation has to be POLLED: outside play mode nothing
            // else pumps it, so "not done" means come back next editor frame.
            if (top.Current is AsyncOperation op && !op.isDone) return;

            bool moved;
            try { moved = top.MoveNext(); }
            catch (Exception e)
            {
                Debug.LogError($"ONLINE probe threw: {e}");
                EditorApplication.update -= Step;
                RemoteControl.ProbeWatch.Done();
                return;
            }

            if (!moved) { _stack.Pop(); return; }
            // Descend. `yield return null` is a frame wait and must NOT be treated as a child.
            if (top.Current is IEnumerator child) _stack.Push(child);
        }
    }
}
