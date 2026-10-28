// The SHOP, pressed. Opens it, reads the catalog the sim answered, buys the cheapest thing in each
// category and checks the PURSE actually moved.
//
// That last part is the whole point: `apply_input` is a silent no-op for anything it will not do
// (replay safety — a malformed event must never panic), so a Buy button wired to nothing and a Buy
// button that works look identical on screen and in the log. Only the Gil says which.
//
//   echo exec:ShopProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class ShopProbe
{
    static int _frame;

    public static void Run()
    {
        // A LOCAL session: the shop is the sim's, and the cheats are what make a purse big enough
        // to buy from every category without farming for it first.
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
        if (++_frame < 120) return;
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("SHOP: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(Drive(bar));
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) { Debug.LogError("SHOP: no session"); yield break; }

        var catalog = sim.QueryJson("shop");
        int rows = 0, at = 0;
        while ((at = catalog.IndexOf("\"buy_price\"", at, System.StringComparison.Ordinal)) >= 0) { rows++; at += 11; }
        Debug.Log($"SHOP catalog: {rows} buyable rows, {catalog.Length} bytes");
        if (rows == 0)
        {
            Debug.LogError("SHOP: the sim answered an EMPTY shop — the Editor may still hold the "
                         + "old libsim_ffi.dylib (it loads once per process; restart after staging).");
            yield break;
        }

        // Money, so affordability is not what is under test. The cheat is JSON with an EARN
        // REASON — this sim's Gil always carries one, so the monetary accounting can tell a cheat
        // from a faucet — and a bare "gil 1000000" is simply refused.
        if (!sim.Debug("{\"give_currency\":{\"gil\":1000000}}"))
            Debug.LogError("SHOP: the give_currency cheat was refused — is this session online?");
        bar.CloseAll();
        bar.Toggle("Shop");
        for (int i = 0; i < 120; i++) yield return null;

        foreach (var tab in new[] { "Gear", "Potions", "Stones", "Materials", "Exemplar", "Regalia" })
        {
            var tb = FindLabel(bar, tab);
            if (tb == null) { Debug.LogError($"SHOP: no '{tab}' tab"); continue; }
            tb.onClick.Invoke();
            for (int i = 0; i < 10; i++) yield return null;

            var buy = FindLabel(bar, "Buy");
            // TWO different answers, and conflating them cost a debugging round: no button at all
            // means the category is empty; a disabled one means it drew and the purse says no.
            if (buy == null) { Debug.Log($"SHOP {tab}: no rows in this category"); continue; }
            if (!buy.interactable)
            {
                Debug.LogError($"SHOP {tab}: the Buy button is disabled — Gil {Gil(sim):N0}");
                continue;
            }
            long before = Gil(sim);
            buy.onClick.Invoke();
            for (int i = 0; i < 10; i++) yield return null;
            long after = Gil(sim);
            if (after < before) Debug.Log($"SHOP {tab}: bought — Gil {before:N0} -> {after:N0}");
            else Debug.LogError($"SHOP {tab}: pressed Buy and the purse did not move ({StatusOf(bar)})");
        }

        ScreenCapture.CaptureScreenshot("/tmp/roi_shop.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log("SHOP done");
        EditorApplication.isPlaying = false;
    }

    static long Gil(Roi.Sim.SimSession sim)
    {
        var m = System.Text.RegularExpressions.Regex.Match(sim.QueryJson("purse"), "\"gil\":(-?\\d+)");
        return m.Success ? long.Parse(m.Groups[1].Value) : 0;
    }

    static string StatusOf(Roi.UI.HudBar bar)
    {
        foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "Status" && t.gameObject.activeInHierarchy) return t.text;
        return "";
    }

    static Button FindLabel(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text == label) return b;
        }
        return null;
    }
}
