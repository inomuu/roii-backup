// Terminal → Editor bridge for a headless dev loop while the Editor is open (batchmode cannot
// attach to a project the Editor holds). Poll `<project>/.roi_cmd`; one command per file:
//   play | stop | refresh | quit | exec:<Class.StaticMethod> | probe:<Name>
// Results go to the Editor log (~/Library/Logs/Unity/Editor.log) as `REMOTE ...` lines.
//   echo play > unity/ROI/.roi_cmd
using System;
using System.IO;
using System.Reflection;
using UnityEditor;
using UnityEngine;

[InitializeOnLoad]
public static class RemoteControl
{
    const string CmdPath = ".roi_cmd";
    static double _next;

    static RemoteControl() { EditorApplication.update += Poll; }

    static void Poll()
    {
        if (EditorApplication.timeSinceStartup < _next) return;
        _next = EditorApplication.timeSinceStartup + 0.5;
        if (!File.Exists(CmdPath)) return;
        string cmd;
        try { cmd = File.ReadAllText(CmdPath).Trim(); File.Delete(CmdPath); }
        catch (IOException) { return; }
        if (cmd.Length == 0) return;
        Debug.Log($"REMOTE cmd: {cmd}");
        if (cmd == "play") EditorApplication.isPlaying = true;
        else if (cmd == "stop") EditorApplication.isPlaying = false;
        else if (cmd == "refresh") AssetDatabase.Refresh();   // the Editor only auto-refreshes on focus
        // QUIT, because a new native plugin needs one. The Editor loads `libsim_ffi.dylib` once
        // and holds it for the life of the process, so a rebuilt sim is invisible until it
        // restarts — and `tools/unity_dylib.sh` says so every time it stages one.
        else if (cmd == "quit") EditorApplication.Exit(0);
        else if (cmd.StartsWith("exec:")) Exec(cmd.Substring(5));
        else if (cmd.StartsWith("probe:")) ProbeWatch.Begin(cmd.Substring(6));
        else Debug.LogWarning($"REMOTE unknown: {cmd}");
    }

    /// Returns whether the target was FOUND. The caller matters: a probe run that cannot find its
    /// method has to fail now, not sit there until a runner's timeout, because "not found" and
    /// "still running" look identical from a log tail.
    static bool Exec(string target)
    {
        var dot = target.LastIndexOf('.');
        if (dot <= 0) { Debug.LogWarning($"REMOTE bad target: {target}"); return false; }
        var typeName = target.Substring(0, dot);
        var method = target.Substring(dot + 1);
        foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
        {
            var t = asm.GetType(typeName);
            var m = t?.GetMethod(method, BindingFlags.Public | BindingFlags.Static);
            if (m == null) continue;
            try { m.Invoke(null, null); Debug.Log($"REMOTE exec ok: {target}"); }
            catch (Exception e) { Debug.LogError($"REMOTE exec failed: {target}: {e}"); }
            return true;
        }
        Debug.LogWarning($"REMOTE not found: {target}");
        return false;
    }

    /// Run ONE probe and say, in one line, whether it passed.
    ///
    /// The probes each report in their own vocabulary — "BAG done", "PRESS DONE", "LOGIN SAVE
    /// VERIFIED" — and four of them announce nothing at the end at all. A shell runner that had
    /// to know each of those would be a second list to keep in step with the probes, and the
    /// first probe added without a marker would silently never be waited for.
    ///
    /// So the verdict is computed HERE, where the error stream actually is: count every error
    /// logged from the moment the probe starts until it leaves play mode (which every probe does
    /// through its own `ToEnd` wrapper), then emit `PROBE END <name> ok|fail`. A probe reports a
    /// failure by `Debug.LogError` — that is already the convention in all seventeen — so the
    /// count IS the verdict, and a Unity error the probe did not expect fails it too, which is
    /// the right answer for a smoke run.
    public static class ProbeWatch
    {
        static string _name;
        static int _errors;
        static readonly System.Text.StringBuilder _first = new System.Text.StringBuilder();

        /// For a probe that runs in EDIT MODE and therefore never leaves play mode — `OnlineProbe`
        /// is the one today. Everything else is watched automatically; a probe only needs this if
        /// it does its work without entering play mode at all.
        public static void Done()
        {
            if (_name != null) Finish();
        }

        public static void Begin(string name)
        {
            if (_name != null) { Debug.LogWarning($"PROBE BUSY {_name} — {name} not started"); return; }
            _name = name;
            _errors = 0;
            _first.Clear();
            Application.logMessageReceived += OnLog;
            EditorApplication.playModeStateChanged += OnPlayMode;
            Debug.Log($"PROBE BEGIN {name}");
            if (!Exec(name + ".Run"))
            {
                _errors++;
                _first.Append($"no such probe: {name}.Run");
                Finish();
                return;
            }
            _deadline = EditorApplication.timeSinceStartup + StartWindow;
            EditorApplication.update += WatchStart;
        }

        /// A probe that never REACHES play mode has already failed — it threw in its own setup, or
        /// the scene would not open. Without this the runner waits out its whole timeout for a
        /// verdict that was decided in the first second.
        const double StartWindow = 45.0;
        static double _deadline;

        static void WatchStart()
        {
            if (_name == null) { EditorApplication.update -= WatchStart; return; }
            if (EditorApplication.isPlaying) { EditorApplication.update -= WatchStart; return; }
            if (EditorApplication.timeSinceStartup < _deadline) return;
            // Not a failure by itself: an EDIT-MODE probe never enters play mode and reports
            // through `Done()` instead. Stop watching for a start that is not coming and let the
            // runner's own timeout be the backstop if that call never arrives.
            EditorApplication.update -= WatchStart;
        }

        static void OnLog(string message, string stack, LogType type)
        {
            if (type != LogType.Error && type != LogType.Exception && type != LogType.Assert) return;
            _errors++;
            if (_errors <= 3) _first.Append(_errors == 1 ? "" : " | ").Append(message.Replace('\n', ' '));
        }

        static void OnPlayMode(PlayModeStateChange change)
        {
            // ENTERED edit mode, not EXITING play mode: a probe's last assertions run on the way
            // out, and reading the count before they have been written reports a pass for a probe
            // that was still failing.
            if (change != PlayModeStateChange.EnteredEditMode) return;
            Finish();
        }

        static void Finish()
        {
            EditorApplication.update -= WatchStart;
            Application.logMessageReceived -= OnLog;
            EditorApplication.playModeStateChanged -= OnPlayMode;
            var verdict = _errors == 0 ? "ok" : "fail";
            var detail = _errors == 0 ? "" : $" — {_errors} error(s): {_first}";
            Debug.Log($"PROBE END {_name} {verdict}{detail}");
            _name = null;
        }
    }
}
