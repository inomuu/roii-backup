// The COMMODITY MARKET, round-tripped: list a stack you actually hold, see the order appear under
// your own listings, cancel it.
//
// That path crosses every boundary this screen has — a sim event (`DepositCommodity`), a save the
// server must VERIFY before the order exists at all, a fetch of the server's ledger, and a direct
// server op for the cancel. A screenshot proves none of it: the units leave the bag optimistically,
// so a listing that the server silently refused looks exactly like one it accepted.
//
// It waits for a DROP rather than cheating a stack in, because the session is online and cheats are
// refused there by design — a cheated state stops verifying, which is the whole reason the door is
// shut.
//
//   echo exec:TradeProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class TradeProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "online");
        // A FRESH account each run. The gear leg has to earn about 100 Gil by selling drops, and an
        // account that has been farmed by an earlier run can be sitting at a wall where it wipes
        // more than it kills — that is the sim's auto-retreat working, but it is not a test bed.
        SessionState.SetString("ROI_NAME", "unity-trade-" + System.DateTime.Now.ToString("MMdd-HHmm"));
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
            if (_frame < 3600) return;
            EditorApplication.update -= Tick;
            Debug.LogError("TRADE: no bar — the login never produced a session");
            return;
        }
        EditorApplication.update -= Tick;
        // ALWAYS leave play mode, including on a failed leg. A probe that bails with `yield break`
        // used to leave the Editor running — and the Editor cannot recompile while it plays, so the
        // NEXT run silently exercised the previous build. That cost a debugging round.
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    /// WAIT FOR THE SERVER TO TAKE IT, not for a number of frames to pass.
    ///
    /// `ReplaySpan` is the distance from the session's anchor; the boot only moves that anchor when
    /// the server has VERIFIED the segment. So a span that grows and then falls is the push landing,
    /// and it is the only thing that says the next leg may start from this base.
    ///
    /// This probe used to wait 600 frames and hope. That held while it spent minutes farming its own
    /// Gil — the first push always settled somewhere in there — and broke the moment it stopped
    /// farming: the second List went out on a base the server had not anchored yet and came back
    /// `start snapshot is not anchored (forged/unknown base)`, with the account CHEAT-FLAGGED for
    /// forgery. Making the probe faster did not cause that; it removed the accident that was hiding
    /// it, which is worth saying because the fix is not "wait longer".
    static IEnumerator WaitAnchored(Roi.Sim.SimSession sim, string what)
    {
        if (sim == null) yield break;
        long peak = 0;
        for (int i = 0; i < 3600; i++)
        {
            long span = sim.ReplaySpan();
            if (span > peak) peak = span;
            if (peak > 0 && span < peak)
            {
                Debug.Log($"TRADE {what}: the server anchored the save (span {peak} -> {span})");
                yield break;
            }
            yield return null;
        }
        Debug.LogError($"TRADE {what}: the save never anchored (span peaked at {peak}) — "
                     + "anything pushed after this is anchored on a base the server does not have");
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) { Debug.LogError("TRADE: no session"); yield break; }

        // Something stackable to sell. The party is fighting while this waits, so a Material or a
        // Junk drop is the natural source — and it is what a player would be listing anyway.
        // POLLED EVERY TENTH FRAME, not every frame. `QueryJson("inventory")` crosses the FFI and
        // serialises the whole bag, and the sim ticks at ten a second — so nine of every ten answers
        // were the previous one. Worth doing; NOT why this probe is slow, which was the first guess
        // and it was wrong. The wait is for a real drop: the party has to kill something, and that
        // takes the time it takes at 10 Hz no matter how fast the Editor draws.
        string held = null;
        for (int i = 0; i < 3600 && held == null; i++)
        {
            if (i % 10 == 0) held = FirstStackable(sim.QueryJson("inventory"));
            yield return null;
        }
        if (held == null)
        {
            Debug.LogError("TRADE: nothing stackable dropped in a minute of play — cannot list");
            yield break;
        }
        Debug.Log($"TRADE holding {held} (kills so far {Kills(sim)})");

        bar.CloseAll();
        bar.Toggle("Trade");
        for (int i = 0; i < 120; i++) yield return null;

        if (!Press(bar, "Sell")) { Debug.LogError("TRADE: no Sell tab"); yield break; }
        for (int i = 0; i < 60; i++) yield return null;

        // By OBJECT name (`Chip_<def_id>`), not by label: the label is a display name derived from
        // the id, and matching it here would mean reimplementing the naming rules in a test.
        var chip = FindNamed(bar, "Chip_" + held);
        if (chip == null) { Debug.LogError($"TRADE: no chip for {held}"); yield break; }
        chip.onClick.Invoke();
        for (int i = 0; i < 180; i++) yield return null;   // the book fetch

        if (!Press(bar, "List")) { Debug.LogError("TRADE: no List button (or it is disabled)"); yield break; }
        Debug.Log("TRADE pressed List — pushing the save");
        yield return WaitAnchored(sim, "junk");
        for (int i = 0; i < 120; i++) yield return null;   // the refetch after it lands
        Debug.Log($"TRADE after listing: {StatusOf(bar)}");
        ScreenCapture.CaptureScreenshot("/tmp/roi_trade.png");
        for (int i = 0; i < 20; i++) yield return null;

        if (!Press(bar, "My listings")) { Debug.LogError("TRADE: no My listings tab"); yield break; }
        for (int i = 0; i < 300; i++) yield return null;
        var cancel = FindPrefix(bar, "Cancel");
        if (cancel == null)
        {
            Debug.LogError($"TRADE: the order never appeared under My listings — {StatusOf(bar)}");
            yield break;
        }
        Debug.Log("TRADE the order is on the server's ledger — cancelling");
        cancel.onClick.Invoke();
        for (int i = 0; i < 300; i++) yield return null;
        Debug.Log($"TRADE after cancel: {StatusOf(bar)}");

        // --- the GEAR half of the same window ------------------------------------------------
        // Equipment is per-instance, so this needs an actual piece in the bag. The account is
        // online (no cheats), so the honest source is the shop — and if it cannot afford one, the
        // leg is SKIPPED rather than reported as a failure.
        // The FASTEST honest source of a listable piece: take one off the character. The party
        // starts equipped, and an unequipped piece lands in the bag — no Gil, no farming, and it is
        // a thing a player does. (Earning it instead is a 17-minute grind: band-1 monsters pay 0
        // Gil by design, and selling junk yields about 6 Gil a minute.)
        if (!HasGear(sim))
        {
            long lead = LeadId(sim);
            for (uint slot = 0; slot < 8 && !HasGear(sim); slot++)
            {
                sim.ApplyInput($"{{\"UnequipItem\":{{\"entity_id\":{lead},\"equipped_slot\":{slot}}}}}");
                for (int i = 0; i < 3; i++) yield return null;
            }
            Debug.Log(HasGear(sim)
                ? $"TRADE unequipped a piece to list (lead {lead})"
                : $"TRADE could not unequip anything (lead {lead})");
        }
        if (!HasGear(sim))
        {
            // FUNDED BY MAIL, not farmed. This used to earn the price the way the game does —
            // band-1 monsters pay 0 Gil, so the early economy is selling the junk they drop — and
            // that read well and cost this probe everything. Junk sells for 1 Gil, the cheapest
            // listable piece is 100, so the leg was waiting on about a hundred and twenty drops
            // through a loop with no logging in it. It ran past 600 s, then past 1200 s, and how
            // long it took depended on what the PREVIOUS run had already sold out of the bag — so
            // it failed differently each time and never said why.
            //
            // `/dev/mail-send` is how GearProbe and SummonProbe were already solving the same
            // problem: pay the account the way a server pays a player, and spend the probe's time
            // on the screen under test. What is lost is incidental coverage of the bag's sell path,
            // which is BagProbe's job and is asserted there properly.
            bool mailed = false;
            yield return Roi.Net.OnlineService.Instance.Client.DevMailGil(100000, ok => mailed = ok);
            if (!mailed) { Debug.LogError("TRADE: could not mail the account its Gil"); yield break; }
            bar.CloseAll();
            bar.Toggle("Mail");
            for (int i = 0; i < 180; i++) yield return null;
            long before = Gil(sim);
            // EVERY unclaimed mail, not the first one — GearProbe's loop, copied rather than
            // remembered. This probe fills its OWN inbox as it goes: each cancelled listing ends
            // with "the unsold units are in your mailbox", so by the time it wants paying the top
            // row is usually an item mail carrying no Gil at all. Claiming once and waiting
            // reported "the mailed Gil never reached the save" for a claim that worked perfectly.
            bool paid = false;
            for (int round = 0; round < 12 && !paid; round++)
            {
                if (!Press(bar, "Claim")) break;
                for (int i = 0; i < 90; i++) yield return null;
                paid = Gil(sim) > before;
            }
            if (!paid)
            {
                Debug.LogError($"TRADE: no mail in the inbox paid Gil — {before:N0} -> {Gil(sim):N0}");
                yield break;
            }
            Debug.Log($"TRADE funded by mail — Gil {before:N0} -> {Gil(sim):N0}");

            bar.CloseAll();
            bar.Toggle("Shop");
            for (int i = 0; i < 120; i++) yield return null;
            Press(bar, "Gear");
            for (int i = 0; i < 30; i++) yield return null;
            var buy = FindExact(bar, "Buy");
            if (buy != null && buy.interactable)
            {
                buy.onClick.Invoke();
                for (int i = 0; i < 60; i++) yield return null;
            }
        }
        if (!HasGear(sim))
        {
            Debug.Log("TRADE gear leg SKIPPED — no gear in the bag and none affordable");
            Debug.Log("TRADE done");
            EditorApplication.isPlaying = false;
            yield break;
        }

        bar.CloseAll();
        bar.Toggle("Trade");
        for (int i = 0; i < 120; i++) yield return null;
        if (!Press(bar, "Gear")) { Debug.LogError("TRADE: no Gear category button"); yield break; }
        for (int i = 0; i < 120; i++) yield return null;
        if (!Press(bar, "Sell")) { Debug.LogError("TRADE gear: no Sell tab"); yield break; }
        for (int i = 0; i < 60; i++) yield return null;

        // The first gear row IS a button (the whole row selects), so press the first one that is
        // not one of the screen's own controls.
        var pick = FirstRowButton(bar);
        if (pick == null) { Debug.LogError("TRADE gear: no listable piece drawn"); yield break; }
        pick.onClick.Invoke();
        for (int i = 0; i < 60; i++) yield return null;
        if (!Press(bar, "List")) { Debug.LogError("TRADE gear: no List button"); yield break; }
        Debug.Log("TRADE gear pressed List — pushing the save");
        yield return WaitAnchored(sim, "gear");
        for (int i = 0; i < 120; i++) yield return null;
        Debug.Log($"TRADE gear after listing: {StatusOf(bar)}");
        ScreenCapture.CaptureScreenshot("/tmp/roi_itemmarket.png");
        for (int i = 0; i < 20; i++) yield return null;

        if (!Press(bar, "My listings")) { Debug.LogError("TRADE gear: no My listings tab"); yield break; }
        for (int i = 0; i < 300; i++) yield return null;
        var gcancel = FindPrefix(bar, "Cancel");
        if (gcancel == null)
        {
            Debug.LogError($"TRADE gear: the listing never reached the server — {StatusOf(bar)}");
            yield break;
        }
        gcancel.onClick.Invoke();
        for (int i = 0; i < 300; i++) yield return null;
        Debug.Log($"TRADE gear after cancel: {StatusOf(bar)}");

        Debug.Log("TRADE done");
    }

    /// The lead member's entity id. PARSED, not pattern-matched: an entity carries nested objects,
    /// so a regex spanning two of its fields runs straight through the brace that ends it.
    static long LeadId(Roi.Sim.SimSession sim)
    {
        var view = JsonUtility.FromJson<ViewDTO>(sim.ViewJson());
        foreach (var e in view?.grid?.entities ?? new EntDTO[0])
            if (e.team == "Player" && !e.is_pet && e.alive) return e.id;
        return 0;
    }

    [System.Serializable] class EntDTO { public long id; public string team; public bool is_pet, alive; }
    [System.Serializable] class GridDTO { public EntDTO[] entities; }
    [System.Serializable] class ViewDTO { public GridDTO grid; }

    static long Kills(Roi.Sim.SimSession sim)
    {
        var m = System.Text.RegularExpressions.Regex.Match(sim.QueryJson("record"), "\"kills\":(\\d+)");
        return m.Success ? long.Parse(m.Groups[1].Value) : -1;
    }

    static long Gil(Roi.Sim.SimSession sim)
    {
        var m = System.Text.RegularExpressions.Regex.Match(sim.QueryJson("purse"), "\"gil\":(-?\\d+)");
        return m.Success ? long.Parse(m.Groups[1].Value) : 0;
    }

    /// Does the bag hold anything the ITEM market would take (non-stackable equipment)?
    static bool HasGear(Roi.Sim.SimSession sim)
    {
        foreach (System.Text.RegularExpressions.Match m in
                 System.Text.RegularExpressions.Regex.Matches(
                     sim.QueryJson("inventory"), "\"kind\":\"(Weapon|Armor)\""))
            return true;
        return false;
    }

    /// A row in the sell list: a Button whose own label is empty (the row IS the control), which is
    /// what distinguishes it from the screen's named buttons.
    static Button FirstRowButton(Roi.UI.HudBar bar)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy || !b.name.StartsWith("I", System.StringComparison.Ordinal))
                continue;
            return b;
        }
        return null;
    }

    /// The first Material/Junk/Consumable/Card row in the bag, by def id.
    static string FirstStackable(string invJson)
    {
        foreach (System.Text.RegularExpressions.Match m in
                 System.Text.RegularExpressions.Regex.Matches(
                     invJson, "\"def_id\":\"([^\"]+)\"[^}]*?\"kind\":\"(Material|Junk|Consumable|Card)\""))
            return m.Groups[1].Value;
        // The two keys can arrive in either order, so try the mirror before giving up.
        foreach (System.Text.RegularExpressions.Match m in
                 System.Text.RegularExpressions.Regex.Matches(
                     invJson, "\"kind\":\"(Material|Junk|Consumable|Card)\"[^}]*?\"def_id\":\"([^\"]+)\""))
            return m.Groups[2].Value;
        return null;
    }

    /// The nth ACTIVE button with this object name — the grid's cells all share one.
    static Button FindNamedNth(Roi.UI.HudBar bar, string objectName, int n)
    {
        int seen = 0;
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy || b.gameObject.name != objectName) continue;
            if (seen++ == n) return b;
        }
        return null;
    }

    static Button FindNamed(Roi.UI.HudBar bar, string objectName)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
            if (b.gameObject.activeInHierarchy && b.gameObject.name == objectName) return b;
        return null;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        var b = FindExact(bar, label);
        if (b == null || !b.interactable) return false;
        b.onClick.Invoke();
        return true;
    }

    static string StatusOf(Roi.UI.HudBar bar)
    {
        foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "Status" && t.gameObject.activeInHierarchy) return t.text;
        return "";
    }

    static Button FindExact(Roi.UI.HudBar bar, string label) => Match(bar, label, false);
    static Button FindPrefix(Roi.UI.HudBar bar, string label) => Match(bar, label, true);

    static Button Match(Roi.UI.HudBar bar, string label, bool prefix)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t == null) continue;
            if (prefix ? t.text.StartsWith(label, System.StringComparison.Ordinal) : t.text == label)
                return b;
        }
        return null;
    }
}
