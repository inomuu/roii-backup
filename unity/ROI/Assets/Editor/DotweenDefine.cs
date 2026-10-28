// Set ROI_DOTWEEN when DOTween is actually in the project, and clear it when it is not.
//
// DOTween ships as an Asset Store .unitypackage, not a UPM package, so nothing in the manifest
// records whether it is present and the tree has to compile either way. The alternative to this
// file is asking whoever imports it to also remember a Scripting Define Symbol — a manual step
// whose only failure mode is silence: the code compiles, the fallback runs, and the library sits
// there unused.
using System;
using System.Linq;
using UnityEditor;
using UnityEditor.Build;

[InitializeOnLoad]
static class DotweenDefine
{
    const string Symbol = "ROI_DOTWEEN";

    static DotweenDefine()
    {
        bool present = AppDomain.CurrentDomain.GetAssemblies()
            .Any(a => a.GetType("DG.Tweening.DOTween") != null);
        var target = NamedBuildTarget.FromBuildTargetGroup(
            BuildPipeline.GetBuildTargetGroup(EditorUserBuildSettings.activeBuildTarget));
        var defines = PlayerSettings.GetScriptingDefineSymbols(target)
            .Split(';').Where(d => !string.IsNullOrWhiteSpace(d)).ToList();
        bool set = defines.Contains(Symbol);
        if (present == set) return;
        if (present) defines.Add(Symbol); else defines.Remove(Symbol);
        PlayerSettings.SetScriptingDefineSymbols(target, string.Join(";", defines));
        UnityEngine.Debug.Log($"DOTWEEN {(present ? "found — ROI_DOTWEEN set" : "gone — ROI_DOTWEEN cleared")}");
    }
}
