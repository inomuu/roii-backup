// The GEAR half of the market, on the SIM side.
//
// Why it is separate from TradeProbe's online run: band-1 monsters pay 0 Gil (`gil_reward` is 0
// across the whole starting band), so an online account needs roughly ten minutes of farming and
// selling before it can afford the cheapest listable piece. The risky half of this screen is not
// the network — the commodity leg already proved the push-then-refetch path — it is the INDEX: a
// listing takes an inventory slot number, and a screen that filters its rows can hand the sim the
// wrong one. That is checkable in a local session in seconds.
//
// So this buys a piece, lists it, and asserts the sim MOVED it: out of the bag, into escrow.
//
// It runs ONLINE even though the assertion is sim-side, and that is not a change of subject: the
// Trade screen draws a "connect to the server" prompt in place of its content when there is no
// session, so an offline run never reaches a row to press. The ten-minute farm this file was
// written to avoid is avoided by the Gil cheat below, not by staying offline.
//
//   echo exec:GearProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class GearProbe
{
    /// The def the shop's Gear tab sells first — and so the one this lists. Named rather than
    /// inferred: the assertion is "the PICKED piece left", and a probe that discovers which piece
    /// it picked from the same list it is testing proves nothing.
    const string Piece = "shield_s1";

    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "online");
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
            Debug.LogError("GEAR: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
    }

    /// The purse, off the per-tick view — the one number the claim has to move.
    static long GilOf(Roi.Sim.SimSession sim)
    {
        var j = sim.ViewJson();
        int i = j.IndexOf("\"gil\":", System.StringComparison.Ordinal);
        if (i < 0) return -1;
        i += 6;
        int end = i;
        while (end < j.Length && char.IsDigit(j[end])) end++;
        return long.TryParse(j.Substring(i, end - i), out var v) ? v : -1;
    }

    /// How many of `defId` a list holds, counted as OCCURRENCES of the def rather than as stacks:
    /// a listable piece is never stackable, so one occurrence is one piece.
    static int CountOf(Roi.Sim.SimSession sim, string key, string defId)
    {
        var j = sim.QueryJson(key);
        var needle = "\"def_id\":\"" + defId + "\"";
        int n = 0, i = 0;
        while ((i = j.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) { Debug.LogError("GEAR: no session"); yield break; }
        // FUNDED BY MAIL, not by a cheat. The sim refuses every debug edit once the client is
        // logged in — a cheated save stops verifying — so an online probe has to be paid the way
        // a player is. `/dev/mail-send` puts a server-authored reward in the inbox and the Mail
        // screen claims it, which means this leg now presses that screen too.
        bool mailed = false;
        yield return Roi.Net.OnlineService.Instance.Client.DevMailGil(100000, ok => mailed = ok);
        if (!mailed) { Debug.LogError("GEAR: could not mail the account its Gil"); yield break; }
        bar.CloseAll();
        bar.Toggle("Mail");
        for (int i = 0; i < 180; i++) yield return null;
        long gilBefore = GilOf(sim);
        // EVERY unclaimed mail, not the first one. A dev account accumulates a backlog — an
        // Excavation reward from an earlier run sits at the top of the inbox — and claiming only
        // the first row reported "the claim did not pay" for a claim that worked perfectly on a
        // mail that carried an item rather than Gil.
        bool paid = false;
        for (int round = 0; round < 12 && !paid; round++)
        {
            if (!Press(bar, "Claim")) break;
            for (int i = 0; i < 90; i++) yield return null;
            paid = GilOf(sim) > gilBefore;
        }
        if (!paid)
        {
            Debug.LogError($"GEAR: no mail in the inbox paid Gil — {gilBefore} -> {GilOf(sim)}");
            yield break;
        }
        Debug.Log($"GEAR funded by mail — Gil {gilBefore} -> {GilOf(sim)}");

        bar.CloseAll();
        bar.Toggle("Shop");
        for (int i = 0; i < 120; i++) yield return null;
        if (!Press(bar, "Gear")) { Debug.LogError("GEAR: no Gear tab in the shop"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        var buy = FindExact(bar, "Buy");
        if (buy == null || !buy.interactable) { Debug.LogError("GEAR: nothing buyable in Gear"); yield break; }
        buy.onClick.Invoke();
        for (int i = 0; i < 60; i++) yield return null;
        int bagBefore = Count(sim, "inventory");
        int pieceBefore = CountOf(sim, "inventory", Piece);
        Debug.Log($"GEAR bought — bag {bagBefore}, holding {pieceBefore} {Piece}");

        bar.CloseAll();
        bar.Toggle("Trade");
        for (int i = 0; i < 120; i++) yield return null;
        if (!Press(bar, "Gear")) { Debug.LogError("GEAR: no Gear category"); yield break; }
        for (int i = 0; i < 60; i++) yield return null;
        if (!Press(bar, "Sell")) { Debug.LogError("GEAR: no Sell tab"); yield break; }
        for (int i = 0; i < 60; i++) yield return null;

        var pick = LastRowButton(bar);
        if (pick == null)
        {
            Debug.LogError("GEAR: no listable piece drawn");
            Debug.Log("GEAR inventory: " + sim.QueryJson("inventory"));
            var names = new System.Text.StringBuilder();
            foreach (var b in bar.GetComponentsInChildren<Button>(true))
                if (b.gameObject.activeInHierarchy) names.Append(b.gameObject.name).Append(' ');
            Debug.Log("GEAR active buttons: " + names);
            yield break;
        }
        pick.onClick.Invoke();
        for (int i = 0; i < 30; i++) yield return null;
        if (!Press(bar, "List")) { Debug.LogError("GEAR: no List button"); yield break; }
        for (int i = 0; i < 120; i++) yield return null;

        int bagAfter = Count(sim, "inventory");
        int pieceAfter = CountOf(sim, "inventory", Piece);
        // THE ASSERTION: the piece that was PICKED is the one that left.
        //
        // Not "escrow gained one" — that was this file's first assertion and it was wrong about
        // where a listing goes. `deposit_item` removes the item from the bag and records a
        // `MarketOp::DepositItem`; `state.escrow` is the DUNGEON's loot hold, and drops.rs asserts
        // it stays empty. The old check could only ever have passed by accident.
        //
        // Counting the picked def_id is the check the file was written for anyway: the risk here
        // is the INDEX, and a wrong index removes some OTHER item while leaving this one — which
        // a bag count alone reports as a success.
        //
        // The BAG TOTAL is deliberately not part of the assertion. It was, and it made this probe
        // race the game: the party keeps fighting through the listing, so a drop landing in the
        // same window holds the total at 5 while the picked piece really did leave (1->0). That
        // reads as "the listing did nothing", which is the opposite of what happened. A total that
        // moves for reasons other than the thing under test is not a second check, it is a second
        // source of failure — so it stays as CONTEXT in the message and out of the condition.
        if (pieceAfter == pieceBefore - 1)
            Debug.Log($"GEAR OK — listed: {Piece} {pieceBefore}->{pieceAfter} (bag {bagBefore}->{bagAfter}, "
                    + $"which the sim also moves). Screen says: {StatusOf(bar)}");
        else
            Debug.LogError($"GEAR: the listing did not move the PICKED item ({Piece} {pieceBefore}->"
                         + $"{pieceAfter}, bag {bagBefore}->{bagAfter}) — {StatusOf(bar)}");

        ScreenCapture.CaptureScreenshot("/tmp/roi_itemmarket.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log("GEAR done");
    }

    static int Count(Roi.Sim.SimSession sim, string key)
    {
        var j = sim.QueryJson(key);
        int n = 0, i = 0;
        while ((i = j.IndexOf("\"def_id\"", i, System.StringComparison.Ordinal)) >= 0) { n++; i += 8; }
        return n;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        var b = FindExact(bar, label);
        if (b == null || !b.interactable) return false;
        b.onClick.Invoke();
        return true;
    }

    /// The LAST sell row, not the first.
    ///
    /// A purchase appends to the bag, so the piece this probe just bought is the last inventory
    /// index and therefore the last row — which makes the assertion deterministic on an account
    /// that has been used before. Pressing the first row listed whatever some earlier run had left
    /// lying in the bag, and then "the picked item did not leave" was true of a listing that
    /// worked perfectly.
    ///
    /// It is also the sharper index test: the row furthest from zero is the one a filter that
    /// renumbers would get most wrong.
    static Button LastRowButton(Roi.UI.HudBar bar)
    {
        Button last = null;
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
            if (b.gameObject.activeInHierarchy && b.gameObject.name.StartsWith("I", System.StringComparison.Ordinal))
                last = b;
        return last;
    }

    static string StatusOf(Roi.UI.HudBar bar)
    {
        foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "Status" && t.gameObject.activeInHierarchy) return t.text;
        return "";
    }

    static Button FindExact(Roi.UI.HudBar bar, string label)
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
