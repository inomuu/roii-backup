// JUST PLAY IT. Enters play mode on the zone scene with no scripted door, so the title screen comes
// up and the game is the game — no probe driving it, nothing exiting play mode when a check passes.
//
//   echo exec:PlayNow.Run > unity/ROI/.roi_cmd
using UnityEditor;
using UnityEditor.SceneManagement;

public static class PlayNow
{
    public static void Run()
    {
        // THE DEV DOOR, straight into the game. "No door" left it sitting on the title screen
        // waiting for a click — which is correct for a player and useless for looking at combat,
        // and quietly made three measurement runs report zero hits because nothing was playing.
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        if (EditorApplication.isPlaying) return;
        EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
        EditorApplication.EnterPlaymode();
    }
}
