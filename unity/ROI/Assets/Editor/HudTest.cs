// Forces every part of the arena overlay on screen at once, so the layout can be judged without
// waiting for a fight to produce one of each.
//   echo exec:HudTest.All > .roi_cmd      (or ROI > HUD > ...)
using Roi.Arena;
using UnityEditor;
using UnityEngine;

public static class HudTest
{
    [MenuItem("ROI/HUD/Show one of everything", false, 60)]
    public static void All()
    {
        var hud = Object.FindFirstObjectByType<ArenaHud>();
        var view = Object.FindFirstObjectByType<ArenaView>();
        if (hud == null || view == null) { Debug.LogWarning("HUD: enter play mode first"); return; }

        var ids = view.ActorIds();
        if (ids.Count == 0) { Debug.LogWarning("HUD: no actors"); return; }
        long a = ids[0], b = ids.Count > 1 ? ids[1] : ids[0], c = ids.Count > 2 ? ids[2] : ids[0];

        hud.SpawnDamage(a, view.ActorAnchor(a), 1234, onPlayer: true);
        hud.SpawnText(b, view.ActorAnchor(b), "Miss", ArenaHud.MissColor);
        hud.SpawnText(c, view.ActorAnchor(c), "Guarded", ArenaHud.GuardColor);
        hud.SpawnHeal(a, view.ActorAnchor(a), 88);

        // A Sonic Blow shape: eight numbers in one column plus the yellow running total.
        var key = "hudtest:" + Time.frameCount;
        ArenaHud.SwingGaps(150, out float within, out _);
        for (int i = 0; i < 8; i++) hud.FeedMultiHit(key, b, view.ActorAnchor(b), 137, 8, i * within);

        hud.SpawnSkillLabel(a, "Sonic Blow !!");
        hud.PushLoot("Iron Sword", 1, "Common");
        hud.PushLoot("Elder Branch", 3, "Uncommon");
        hud.PushLoot("Runed Cuirass", 1, "Rare");
        hud.PushLoot("Sunless Crown", 1, "Legendary");
        Debug.Log($"HUDTEST fired on actors {a}/{b}/{c}");
    }

    [MenuItem("ROI/HUD/Toggle damage numbers", false, 61)]
    public static void ToggleNumbers()
    {
        var hud = Object.FindFirstObjectByType<ArenaHud>();
        if (hud == null) return;
        hud.ShowDamageNumbers = !hud.ShowDamageNumbers;
        Debug.Log("HUD damage numbers " + hud.ShowDamageNumbers);
    }
}
