using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text;
using UnityEditor;
using UnityEngine;
using Object = UnityEngine.Object;

// Live visual review, through the same event presentation as real combat.
[InitializeOnLoad]
public static class BreakReview
{
    static bool _running;
    static readonly FieldInfo Banners = typeof(Roi.Arena.ArenaHud).GetField("_banners", BindingFlags.Instance | BindingFlags.NonPublic);
    static readonly FieldInfo FocusAge = typeof(Roi.Arena.ArenaView).GetField("_focusAge", BindingFlags.Instance | BindingFlags.NonPublic);
    sealed class Shot { public string Path; public Texture2D Texture; }

    static BreakReview()
    {
        EditorApplication.playModeStateChanged += state =>
        {
            if (state == PlayModeStateChange.EnteredEditMode) _running = false;
        };
    }
    public static void Run() => Begin(true);
    public static void RunNormal() => Begin(false);
    // A single late frame avoids earlier GPU readbacks skipping the title exit.
    public static void RunStill(float age = .55f) => Begin(true, Mathf.Clamp(age, .15f, 1.5f));
    static void Begin(bool boss, float stillAge = -1f)
    {
        var view = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (!EditorApplication.isPlaying || view == null) { Debug.LogError("BREAK REVIEW: enter Play Mode first"); return; }
        if (_running) { Debug.LogWarning("BREAK REVIEW: a capture is already running"); return; }
        _running = true;
        view.StartCoroutine(Capture(view, boss, stillAge));
    }
    static IEnumerator Capture(Roi.Arena.ArenaView view, bool boss, float stillAge)
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var hud = view.GetComponentInChildren<Roi.Arena.ArenaHud>();
        bool paused = driver != null && driver.Paused;
        var lights = view.GetComponentsInChildren<Light>();
        float[] energy = new float[lights.Length];
        for (int i = 0; i < lights.Length; i++) energy[i] = lights[i].intensity;
        string prefix = stillAge >= 0f ? "/tmp/break-still-" : boss ? "/tmp/break-review-" : "/tmp/break-normal-";
        var shots = new List<Shot>();
        var csv = new StringBuilder("image,requested_seconds,unscaled_elapsed,scaled_elapsed,banner_age,focus_age,frame,delta_time,unscaled_delta_time\n");
        var endOfFrame = new WaitForEndOfFrame();
        try
        {
            if (driver != null) driver.Paused = true;
            yield return endOfFrame;
            TakeShot(shots, csv, prefix + "before.png", 0f, Time.unscaledTime, Time.time, view, hud);
            // Account for the baseline GPU readback before the effect's clocks start.
            yield return null;
            yield return endOfFrame;
            if (!view.DebugBreak(boss, true)) { Debug.LogError("BREAK REVIEW: no living target"); yield break; }
            float start = Time.unscaledTime, scaledStart = Time.time;
            float[] marks = stillAge >= 0f ? new[] { stillAge } : new[] { .15f, .55f, 1.1f, 1.45f, 1.9f };
            int next = 0;
            while (next < marks.Length)
            {
                // The title, focus and shards share unscaled time. Capture the rendered frame,
                // rather than queuing a filename whose image could be read on a later frame.
                yield return endOfFrame;
                if (Time.unscaledTime - start < marks[next]) continue;
                TakeShot(shots, csv, prefix + next + ".png", marks[next], start, scaledStart, view, hud);
                next++;
            }
            while (Time.unscaledTime - start < 1.9f) yield return null;
            // Focus restores in LateUpdate; a still capture may resume here before that phase.
            yield return endOfFrame;
            var key = view.transform.Find("BreakKey");
            if (key != null && key.GetComponent<Light>().enabled)
                Debug.LogError("BREAK REVIEW: key remained enabled after beat");
            else Debug.Log("BREAK REVIEW: key disabled after beat");
            for (int i = 0; i < lights.Length; i++)
                if (lights[i] != null && lights[i].name != "BreakKey" && !Mathf.Approximately(lights[i].intensity, energy[i]))
                    Debug.LogError("BREAK REVIEW: light did not restore: " + lights[i].name);
            if (Shader.GetGlobalVector("_BreakKeyColor").sqrMagnitude > .0001f)
                Debug.LogError("BREAK REVIEW: actor key did not reset");
            // PNG compression and disk I/O must not consume the animation's capture interval.
            foreach (var shot in shots) File.WriteAllBytes(shot.Path, shot.Texture.EncodeToPNG());
            File.WriteAllText(prefix + "timing.csv", csv.ToString());
            Debug.Log("BREAK REVIEW: complete; actual capture ages in " + prefix + "timing.csv");
        }
        finally
        {
            foreach (var shot in shots) if (shot.Texture != null) Object.Destroy(shot.Texture);
            if (driver != null) driver.Paused = paused;
            _running = false;
        }
    }

    static void TakeShot(List<Shot> shots, StringBuilder csv, string path, float mark,
        float start, float scaledStart, Roi.Arena.ArenaView view, Roi.Arena.ArenaHud hud)
    {
        float elapsed = Time.unscaledTime - start;
        float bannerAge = -1f;
        var banners = hud != null ? Banners.GetValue(hud) as IList : null;
        if (banners != null)
            foreach (var banner in banners)
                if ((long)banner.GetType().GetField("Amount").GetValue(banner) == 999L)
                    bannerAge = (float)banner.GetType().GetField("Age").GetValue(banner);
        float focusAge = (float)FocusAge.GetValue(view);
        csv.AppendFormat(CultureInfo.InvariantCulture, "{0},{1:F4},{2:F4},{3:F4},{4:F4},{5:F4},{6},{7:F4},{8:F4}\n",
            Path.GetFileName(path), mark, elapsed, Time.time - scaledStart, bannerAge,
            focusAge, Time.frameCount, Time.deltaTime, Time.unscaledDeltaTime);
        shots.Add(new Shot { Path = path, Texture = ScreenCapture.CaptureScreenshotAsTexture() });
        if (mark > 0f && elapsed - mark > 0.10f)
            Debug.LogWarning("BREAK REVIEW: capture late by " + (elapsed - mark).ToString("F3", CultureInfo.InvariantCulture) + " s; see actual ages in CSV");
    }
}
