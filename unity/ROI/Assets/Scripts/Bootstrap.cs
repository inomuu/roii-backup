// P0 proof: load the catalog, start a session through sim-ffi, tick at 10 Hz, show tick / hash /
// replay span on screen and log a save bundle every 10 s. No UI framework yet — OnGUI only.
using System.IO;
using Roi.Sim;
using UnityEngine;

public sealed class Bootstrap : MonoBehaviour
{
    public ulong Seed = 42;
    public string ZoneId = "zone_b1_1";
    public string ClassId = "seraphine";
    public string PartyName = "Idlers";

    SimDriver _driver;
    string _lastView = "{}";
    string _hash = "";
    string _status = "starting";
    int _bundles;

    void Start()
    {
        // Idle game: the sim must keep ticking while the window is unfocused (Player setting
        // "Run In Background" is also on; this guards a build made from other settings).
        Application.runInBackground = true;
        var path = Path.Combine(Application.streamingAssetsPath, "game_data.json");
        if (!File.Exists(path)) { _status = "missing " + path; return; }
        var gd = File.ReadAllText(path);
        var simVer = SimSession.SimVersion();
        var catalogVer = ExtractVersion(gd);
        // sim-core VERSION and the catalog's data version are different series (0.0.313 vs
        // 0.0.74); the Godot client gates sim_version against the SERVER (P3), not the catalog.
        _status = $"sim {simVer} / data {catalogVer} / {SimSession.SimGitStamp()}";

        var session = SimSession.NewAs(Seed, gd, ZoneId, ClassId, PartyName);
        if (session == null) { _status += " | roi_new_as FAILED"; return; }
        _driver = gameObject.AddComponent<SimDriver>();
        _driver.StateUpdated += v => { _lastView = v; };
        _driver.SaveRequested += b =>
        {
            _bundles++;
            Debug.Log($"bundle #{_bundles}: {b.Length} bytes, span {_driver.Session.ReplaySpan()}");
            _driver.CommitAnchor();   // P0 has no server: pretend it verified (P3 gates this on /save)
        };
        _driver.Attach(session);
    }

    void Update()
    {
        if (_driver != null && Time.frameCount % 30 == 0) _hash = _driver.Session.StateHash();
    }

    void OnGUI()
    {
        GUI.skin.label.fontSize = 14;
        var y = 10;
        GUI.Label(new Rect(10, y, 900, 24), _status); y += 24;
        if (_driver == null) return;
        GUI.Label(new Rect(10, y, 900, 24), $"tick {_driver.Tick}   hash {_hash}   span {_driver.Session.ReplaySpan()}   bundles {_bundles}"); y += 24;
        GUI.Label(new Rect(10, y, 900, 24), $"view {_lastView.Length} bytes"); y += 24;
        // a few keys straight from the view, to prove we read what Godot reads
        foreach (var key in new[] { "\"tick\"", "\"gil\"", "\"current_zone_id\"" })
        {
            var i = _lastView.IndexOf(key);
            if (i >= 0) { GUI.Label(new Rect(10, y, 900, 24), _lastView.Substring(i, Mathf.Min(60, _lastView.Length - i))); y += 22; }
        }
    }

    static string ExtractVersion(string json)
    {
        var i = json.IndexOf("\"version\"");
        if (i < 0) return "?";
        var q1 = json.IndexOf('"', json.IndexOf(':', i) + 1);
        var q2 = json.IndexOf('"', q1 + 1);
        return (q1 >= 0 && q2 > q1) ? json.Substring(q1 + 1, q2 - q1 - 1) : "?";
    }
}
