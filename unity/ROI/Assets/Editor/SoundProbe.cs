// DOES ANYTHING ACTUALLY MAKE A NOISE. There is no way to hear a probe, so this asserts the two
// things that stand between a wired sound and a silent one:
//   the CLIPS DECODED (a file missing from StreamingAssets, or an MP3 Unity would not read, is a
//     warning in a log nobody re-reads and silence in the game), and
//   a VOICE ACTUALLY PLAYING after a real hit landed in the arena — which is the difference
//     between a table that is correct and a table nothing consults.
//
//   echo exec:SoundProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class SoundProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _frame = 0;
        if (!EditorApplication.isPlaying)
        {
            EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
            EditorApplication.playModeStateChanged += OnState;
            EditorApplication.EnterPlaymode();
        }
        else EditorApplication.update += Tick;
    }

    static void OnState(PlayModeStateChange s)
    {
        if (s != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= OnState;
        EditorApplication.update += Tick;
    }

    static void Tick()
    {
        if (++_frame < 150) return;
        var fx = Object.FindFirstObjectByType<Roi.Audio.SoundFx>();
        if (fx == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("SOUND: the audio layer was never created");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(fx)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.Audio.SoundFx fx)
    {
        // Decoding is a coroutine per file over a web request, even for a local one. Six files,
        // one of them four megabytes — give them a real budget rather than a guessed frame count.
        int waited = 0;
        while (waited < 900 && !AllLoaded(fx)) { waited++; yield return null; }
        foreach (var key in Keys())
            if (!fx.Has(key)) Debug.LogError($"SOUND FAILED — '{key}' never decoded");
        if (AllLoaded(fx)) Debug.Log($"SOUND OK — all {CountKeys()} clips decoded in {waited} frames");

        // A LISTENER. Without one every source still reports `isPlaying` and the game is silent,
        // so this assertion is the only thing between "the audio is wired" and "the audio is
        // audible" — and the first version of this probe passed while the scene had none.
        Report("there is an audio listener", Object.FindFirstObjectByType<AudioListener>() != null);

        // MUSIC: the arena asks for the band's track every tick, and `PlayBgm` de-dups. If the
        // pending-track path is broken (the ask happens while the file is still decoding) this is
        // where it shows.
        // ASK FOR THE TRACK RATHER THAN WAITING TO OVERHEAR ONE. The arena does ask every tick, so
        // waiting worked whenever the session happened to be somewhere that had asked recently —
        // and reported the music system broken when it was not. What this assertion is for is
        // whether a track that has been ASKED for reaches a looping voice, so the probe asks.
        string bgm = null;
        foreach (var k in Roi.Audio.SoundTable.Bgm.Keys) { bgm = k; break; }
        if (bgm == null) Debug.LogError("SOUND FAILED — the table declares no music at all");
        else fx.PlayBgm(bgm);

        var sources = fx.GetComponents<AudioSource>();
        bool music = false;
        for (int i = 0; i < 240 && !music; i++)
        {
            foreach (var s in sources) if (s.loop && s.isPlaying) music = true;
            yield return null;
        }
        Report($"the music plays when asked ({bgm})", music);

        // SFX: wait for the party to actually hit something. A one-shot voice is playing for the
        // length of its clip, so the check is "did any non-looping source start".
        bool oneShot = false;
        for (int i = 0; i < 3000 && !oneShot; i++)
        {
            foreach (var s in sources) if (!s.loop && s.isPlaying) oneShot = true;
            yield return null;
        }
        Report("a hit fired a one-shot voice", oneShot);

        Debug.Log("SOUND done");
    }

    static System.Collections.Generic.IEnumerable<string> Keys()
    {
        foreach (var k in Roi.Audio.SoundTable.Paths.Keys) yield return k;
        foreach (var k in Roi.Audio.SoundTable.Bgm.Keys) yield return k;
    }

    static int CountKeys()
    {
        int n = 0;
        foreach (var _ in Keys()) n++;
        return n;
    }

    static bool AllLoaded(Roi.Audio.SoundFx fx)
    {
        foreach (var k in Keys()) if (!fx.Has(k)) return false;
        return true;
    }

    static void Report(string what, bool ok)
    {
        if (ok) Debug.Log($"SOUND OK — {what}");
        else Debug.LogError($"SOUND FAILED — {what}");
    }
}
