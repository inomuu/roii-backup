using System;
using System.Reflection;
using Roi.Net;
using Roi.Sim;
using UnityEditor;
using UnityEngine;
using UnityEngine.Networking;

// Isolated sessions only: never reads or writes the player's save or contacts a server.
public static class SaveReliabilityProbe
{
    static int _checks;
    static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception(message);
        _checks++;
    }

    public static void Run()
    {
        GameObject go = null, owner = null;
        try
        {
            _checks = 0;
            var localVersion = ServerClient.LocalVersion();
            Check(!string.IsNullOrEmpty(localVersion.sim_build), "Loaded native plugin exposes its actual build identity");
            var remoteVersion = JsonUtility.FromJson<ServerClient.VersionInfo>(JsonUtility.ToJson(localVersion));
            Check(ServerClient.CompatibilityError(localVersion, remoteVersion) == "", "Matching replay versions can connect");
            remoteVersion.game_data_version = "different-catalog";
            Check(ServerClient.CompatibilityError(localVersion, remoteVersion) != "", "Catalog mismatch must block before playing");
            remoteVersion.game_data_version = localVersion.game_data_version;
            remoteVersion.sim_build = "different-build";
            Check(ServerClient.CompatibilityError(localVersion, remoteVersion) != "", "Same version with different simulation source must block");
            remoteVersion.sim_build = localVersion.sim_build;
            remoteVersion.sim_version = "different-version";
            Check(ServerClient.CompatibilityError(localVersion, remoteVersion) != "", "Simulation version mismatch must block");
            Check(ServerClient.CompatibilityError(localVersion, new ServerClient.VersionInfo()) != "",
                "Missing server identity must not silently bypass compatibility");
            var parse = typeof(ServerClient).GetMethod("ParseResponse", BindingFlags.NonPublic | BindingFlags.Static)
                .MakeGenericMethod(typeof(ServerClient.SaveResp));
            foreach (string body in new[] { "", "<html>Bad gateway</html>", "{broken" })
                Check(parse.Invoke(null, new object[] { body }) == null, "Malformed response must be retryable");
            var response = (ServerClient.SaveResp)parse.Invoke(null, new object[] { "{\"ok\":true,\"final_tick\":123}" });
            Check(response.ok && response.final_tick == 123, "Valid save response must survive parsing");
            response = (ServerClient.SaveResp)parse.Invoke(null, new object[] { "{ \"ok\" : true }" });
            Check(response.ok, "Whitespace in valid JSON must not turn success into failure");
            var client = new ServerClient("http://127.0.0.1:1");
            var post = typeof(ServerClient).GetMethod("Post", BindingFlags.NonPublic | BindingFlags.Instance);
            using (var req = (UnityWebRequest)post.Invoke(client, new object[] { "/save", "{}" }))
                Check(req.timeout > 0, "Save request must time out");
            using (var req = UnityWebRequest.Get("http://127.0.0.1:1/save"))
            {
                typeof(ServerClient).GetMethod("Authorize", BindingFlags.NonPublic | BindingFlags.Instance)
                    .Invoke(client, new object[] { req });
                Check(req.timeout > 0, "Handshake must time out");
            }
            go = new GameObject("SaveReliabilityProbe");
            var online = go.AddComponent<OnlineService>();
            var connecting = typeof(OnlineService).GetField("_connecting", BindingFlags.NonPublic | BindingFlags.Instance);
            connecting.SetValue(online, true);
            online.Connect("must-not-start");
            online.ConnectAndWait("must-not-replace", true);
            Check(online.Client == null && !online.PreferredUseSteam,
                "A concurrent login must not start a second request or replace the selected account route");
            online.Disconnect();
            Check(!(bool)connecting.GetValue(online) && !online.Ready && online.Client == null,
                "Disconnect must cancel login ownership and readiness");
            var driver = go.AddComponent<SimDriver>();
            var session = SimSession.NewStarter(42, SimSession.LoadCatalog(), "zone_b1_1", "Probe");
            Check(session != null, "Native session must start");
            driver.Attach(session);
            driver.RequireVerifiedSaves = true;
            int attempts = 0;
            driver.SaveRequested += _ => attempts++;
            var advance = typeof(SimDriver).GetMethod("Advance", BindingFlags.NonPublic | BindingFlags.Instance);
            session.Tick((ulong)SimDriver.MaxUnsavedTicks - 2);
            advance.Invoke(driver, new object[] { 1f });
            Check(session.ReplaySpan() == SimDriver.MaxUnsavedTicks, "Tick batch must stop exactly at the unsaved limit");
            Check(driver.WaitingForSave, "An unverified session must wait at the limit");
            var hash = session.StateHash();
            Check(!driver.ApplyInput("{\"SetPartyName\":{\"name\":\"Changed\"}}") && session.StateHash() == hash,
                "Waiting session must refuse new inputs");
            Check(driver.ApplyVerifiedInputs(session, new[] { "{\"ResetDps\":null}" }, _ => true)
                == VerifiedInputResult.Unavailable, "Verified input path must respect the wait");
            advance.Invoke(driver, new object[] { 30f });
            advance.Invoke(driver, new object[] { 30f });
            Check(attempts == 2 && session.StateHash() == hash, "Save retries must continue without advancing the session");
            driver.CommitAnchor();
            advance.Invoke(driver, new object[] { 0.1f });
            Check(!driver.WaitingForSave && session.ReplaySpan() == 1, "Successful save must resume without accumulated time");
            owner = new GameObject("Pause owner");
            driver.Paused = true;
            driver.HoldPresentation(owner);
            var next = SimSession.NewStarter(43, SimSession.LoadCatalog(), "zone_b1_1", "Next");
            driver.Attach(next);
            Check(!driver.Paused && !session.IsAlive, "Replacing session must release obsolete pauses and native handle");
            driver.Attach(next);
            Check(next.IsAlive, "Reattaching the same session must not dispose it");
            driver.Paused = true;
            attempts = 0;
            advance.Invoke(driver, new object[] { 11f });
            Check(attempts == 1 && next.ReplaySpan() == 0, "Presentation pause must not block saving");
            driver.StateUpdated += _ => { };
            driver.Attach(null);
            Check(driver.Session == null && !next.IsAlive, "Detaching must publish safely");
            Debug.Log($"SAVE RELIABILITY PASS: {_checks} checks");
        }
        catch (Exception ex)
        {
            Debug.LogError("SAVE RELIABILITY FAIL: " + ex);
            if (Application.isBatchMode) EditorApplication.Exit(1);
        }
        finally
        {
            if (go != null) UnityEngine.Object.DestroyImmediate(go);
            if (owner != null) UnityEngine.Object.DestroyImmediate(owner);
        }
    }
}
