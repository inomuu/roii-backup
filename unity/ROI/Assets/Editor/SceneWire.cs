// Put the boot in the scene, idempotently. The UI is built from code and the scenes hold only what
// cannot be — so this exists rather than a prefab: one object carrying `SessionBoot`, which is what
// makes the scene open on the login instead of on a session it started itself.
//
//   echo exec:SceneWire.AddSessionBoot > unity/ROI/.roi_cmd
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class SceneWire
{
    const string ScenePath = ProbeScene.Path;

    [MenuItem("ROI/Scene/Add session boot", false, 40)]
    public static void AddSessionBoot()
    {
        var scene = EditorSceneManager.OpenScene(ScenePath, OpenSceneMode.Single);
        if (Object.FindFirstObjectByType<Roi.Sim.SessionBoot>() != null)
        {
            Debug.Log("SCENE: session boot already present");
            return;
        }
        var go = new GameObject("Boot");
        go.AddComponent<Roi.Sim.SessionBoot>();
        EditorSceneManager.MarkSceneDirty(scene);
        EditorSceneManager.SaveScene(scene);
        Debug.Log("SCENE: session boot added to " + ScenePath);
    }
}
