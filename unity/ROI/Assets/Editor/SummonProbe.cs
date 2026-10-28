// Drive the SUMMON screen end to end against the local server, through its real controls.
//
// The gacha is the one screen whose substance is entirely the server's, so a screenshot proves even
// less here than usual: the buttons could be perfectly wired to an endpoint that refuses them. This
// connects, presses Summon, walks the 402 QUOTE that a first pull always produces (the account
// holds no coupons), presses Confirm, and checks that pulls came back.
//
// It runs as a real Unity coroutine on `OnlineService`, which is a MonoBehaviour in play mode —
// the editor's own update loop cannot await a web request without the stack-walking runner, and
// there is no reason to use that when a live scene is right there.
//
//   echo exec:SummonProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class SummonProbe
{
    static int _frame;

    public static void Run()
    {
        // ONLINE from the boot, so the session runs on the server's seed and its saves can anchor.
        // Connecting from inside the Summon screen instead would leave the sim on a client
        // seed, and every push would be refused as an unanchored start.
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
        // The bar arrives WITH the session, and the session now waits on a login — a sign-in
        // crosses the network, so this waits for the chrome instead of assuming a frame count.
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 3600) return;
            EditorApplication.update -= Tick;
            Debug.LogError("SUMMON: no bar — the login never produced a session");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
    }

    /// LEAVE PLAY MODE, whichever way the run ended. Half of this probe's exits were bare
    /// `yield break`s that left the Editor playing, and a stuck play mode does not fail — it
    /// silently runs the NEXT probe against the previous probe's session.
    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        // FUND IT FIRST. A pull costs Crystals, the wallet is the SERVER's, and this probe used to
        // assume some earlier run had left a balance — so it passed for weeks and then reported
        // "not enough gems", which reads as a broken screen rather than an empty purse.
        yield return Roi.Net.OnlineService.Instance.Client.DevGrantCrystals(3000, ok =>
        {
            if (!ok) Debug.LogWarning("SUMMON: could not top the wallet up — /dev/gacha-gems is "
                                    + "dev-gated, so this only works against a local server");
        });
        bar.Toggle("Summon");
        yield return null;

        var connect = Find(bar, "Connect to the local server");
        if (connect == null) { Debug.Log("SUMMON: already connected"); }
        else { connect.onClick.Invoke(); Debug.Log("SUMMON pressed Connect"); }

        var net = Roi.Net.OnlineService.Instance;
        for (int i = 0; i < 600 && !net.LoggedIn; i++) yield return null;
        if (!net.LoggedIn) { Debug.LogError($"SUMMON: never connected — {net.Status}"); yield break; }
        Debug.Log($"SUMMON connected: {net.Status}");

        // The screen refreshes itself on connect; give the state call a moment to land.
        for (int i = 0; i < 240 && Find(bar, "Excavate ×1") == null; i++) yield return null;
        var one = Find(bar, "Excavate ×1");
        // The label carries the price, so the button is matched by prefix rather than by equality.
        if (one == null) one = FindPrefix(bar, "Excavate ×1");
        if (one == null) { Debug.LogError("SUMMON: no pull button after connecting"); yield break; }
        Debug.Log("SUMMON pressing Excavate x1");
        one.onClick.Invoke();

        // A pull quotes only when the account has NO COUPON to spend: the server answers 402 with a
        // price and charges nothing, and the Confirm button appearing IS the quote being handled.
        //
        // This used to say a FIRST pull always quotes. That was true of the one aged dev account
        // every probe shared and false of a real new one: the schedule machine hands a brand-new
        // account a `newcomer_gacha` coupon on first login, so its first pull spends the coupon,
        // costs nothing and never quotes. The premise held for weeks and broke the day sign-in
        // started producing fresh accounts — so the coupon is SPENT first and the quote asserted on
        // the pull after it, which is the only pull guaranteed to need paying for.
        for (int i = 0; i < 600 && FindPrefix(bar, "Confirm") == null; i++)
        {
            if (StatusOf(bar) is string st && st.Contains("collected")) break;  // the coupon path
            yield return null;
        }
        var confirm = FindPrefix(bar, "Confirm");
        if (confirm == null)
        {
            Debug.Log("SUMMON: no quote — a coupon paid for that pull. Pulling again for the quote.");
            for (int i = 0; i < 600 && FindPrefix(bar, "Excavate ×1") == null; i++) yield return null;
            var again = FindPrefix(bar, "Excavate ×1");
            if (again == null) { Debug.LogError("SUMMON: the pull button never came back"); yield break; }
            again.onClick.Invoke();
            for (int i = 0; i < 600 && FindPrefix(bar, "Confirm") == null; i++) yield return null;
            confirm = FindPrefix(bar, "Confirm");
        }
        if (confirm == null) { Debug.LogError("SUMMON: no Confirm — the 402 quote was not surfaced"); yield break; }
        Debug.Log("SUMMON got the quote, pressing Confirm");
        confirm.onClick.Invoke();

        for (int i = 0; i < 600; i++)
        {
            var status = StatusOf(bar);
            // The reward must reach the SAVE, not just the mailbox. "added to your inventory" is
            // the screen saying it claimed its own mail and applied the event.
            // "collected", not "added to your inventory". The screen changed its wording when the
            // Excavation stopped claiming a Regalia lands in the bag — it does not, it banks into
            // an account-bound list — and this probe kept waiting for a sentence nothing says.
            if (status != null && status.Contains("collected"))
            {
                Debug.Log($"SUMMON OK — {status}");
                // Photograph the RESULT state, which is the only one worth looking at: the wallet
                // has moved, the pity ladder has moved, and the pulls are on screen.
                UnityEngine.ScreenCapture.CaptureScreenshot("/tmp/roi_summon.png");
                for (int k = 0; k < 30; k++) yield return null;

                // The same connection serves the ledgers, so they are checked in the same run
                // rather than paying for a second play-mode entry and a second login.
                // THE CHAIN, end to end: ClaimMail -> verified save -> consume_claim. Applying
                // the event only puts the reward in the SAVE; the server does not release the mail
                // until a save carrying that event has been verified. So the mailbox is expected to
                // still hold it until we push — asserting otherwise (as this did) fails a design
                // that is working.
                // WHERE DID IT LAND? An Exemplar or a Regalia must never enter the bag — the sim
                // banks them into their own uncapped, account-bound collections so a pull can
                // never be lost to a full inventory. Worth asserting rather than assuming: the
                // reward path is new and the bag is the obvious place for it to go wrong.
                var sim0 = Object.FindFirstObjectByType<Roi.Sim.SimDriver>()?.Session;
                if (sim0 != null)
                {
                    Debug.Log($"SUMMON collections — inv={Rows(sim0, "inventory")} "
                            + $"regalia={Rows(sim0, "owned_regalia")} "
                            + $"exemplar_units={Rows(sim0, "exemplar_units")} "
                            + $"album={Rows(sim0, "card_album")}");
                }

                yield return SettleSave();

                foreach (var screen in new[] { "Mail", "Daily", "Weekly" })
                {
                    bar.CloseAll();
                    bar.Toggle(screen);
                    for (int k = 0; k < 300 && StatusOf(bar) == null; k++) yield return null;
                    for (int k = 0; k < 180; k++) yield return null;   // let the fetch land
                    if (screen == "Mail")
                    {
                        // Whatever is left here is mail no verified save has settled yet — earlier
                        // runs' rewards included. The number is reported, not asserted: the claim
                        // this run made was proved by the save above.
                        Debug.Log($"SUMMON Mail — {StatusOf(bar)}");
                        UnityEngine.ScreenCapture.CaptureScreenshot("/tmp/roi_mail.png");
                        for (int k = 0; k < 20; k++) yield return null;
                        continue;
                    }
                    var claim = FindPrefix(bar, "Claim") ?? FindPrefix(bar, "Nothing to claim");
                    if (claim == null) { Debug.LogError($"SUMMON: {screen} never drew its claim button"); continue; }
                    Debug.Log($"SUMMON {screen} ok — claim button says '{claim.GetComponentInChildren<TMPro.TextMeshProUGUI>().text}', "
                            + $"interactable={claim.interactable}");
                    for (int k = 0; k < 10; k++) yield return null;
                    UnityEngine.ScreenCapture.CaptureScreenshot($"/tmp/roi_{screen.ToLower()}.png");
                    for (int k = 0; k < 20; k++) yield return null;
                }
                EditorApplication.isPlaying = false;
                yield break;
            }
            if (status != null && status.StartsWith("refused"))
            {
                Debug.LogError($"SUMMON refused: {status}");
                yield break;
            }
            yield return null;
        }
        Debug.LogError($"SUMMON timed out. status={StatusOf(bar)}");
    }

    /// Push the session so the server can VERIFY it and consume the claims it carries. This is
    /// the half that turns "the reward is in my save" into "the server agrees it is mine", and it
    /// is the only thing that releases the mail.
    static IEnumerator SettleSave()
    {
        var net = Roi.Net.OnlineService.Instance;
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null || !net.LoggedIn) { Debug.LogError("SUMMON: cannot settle — no session"); yield break; }
        var bundle = sim.ExportLog();
        if (string.IsNullOrEmpty(bundle)) { Debug.LogError("SUMMON: export_log produced nothing"); yield break; }
        bool done = false;
        yield return net.Client.PutSave(bundle, (ok, resp, msg) =>
        {
            if (ok)
            {
                sim.CommitAnchor();
                Debug.Log($"SUMMON save VERIFIED at tick {resp.final_tick} — the claim is settled");
            }
            else Debug.LogError($"SUMMON save REJECTED: {msg}");
            done = true;
        });
        while (!done) yield return null;
    }

    static int Rows(Roi.Sim.SimSession s, string key)
    {
        var j = s.QueryJson(key);
        int n = 0, i = 0;
        while ((i = j.IndexOf("\"def_id\"", i, System.StringComparison.Ordinal)) >= 0) { n++; i += 8; }
        return n;
    }

    static string StatusOf(Roi.UI.HudBar bar)
    {
        foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "Status" && t.gameObject.activeInHierarchy) return t.text;
        return null;
    }

    static Button Find(Roi.UI.HudBar bar, string label) => Match(bar, label, false);
    static Button FindPrefix(Roi.UI.HudBar bar, string label) => Match(bar, label, true);

    static Button Match(Roi.UI.HudBar bar, string label, bool prefix)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t == null) continue;
            if (prefix ? t.text.StartsWith(label) : t.text == label) return b;
        }
        return null;
    }
}
