// THE ALBUM'S FOUR MOVES, pressed: file a card, take it back, raise its star, bind it.
//
// Each one is a different SHAPE of change, and only the first is a move between two lists:
//   deposit / withdraw — a count on both sides,
//   feed  — the stack SHRINKS and the survivor's star rises,
//   bind  — the stack shrinks and every party member gains a permanent modifier.
// So each leg asserts the thing that actually moved, read off the sim. The screen is never the
// evidence: `apply_input` refuses in silence, and every one of these looks identical when it does.
//
//   echo exec:AlbumProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class AlbumProbe
{
    // A card with a BIND effect and star ranks, so all four legs have something to act on.
    const string CardDef = "card_greenling";

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
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("ALBUM: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        UnityEditor.PlayModeWindow.SetCustomRenderingResolution(2560, 1440, "ROI capture");
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
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
        if (sim == null) { Debug.LogError("ALBUM: no session"); yield break; }

        // FOUR copies: one to withdraw and put back, two for a feed, and enough left for the
        // first bind (which costs one). Cards are stackable, so these are one bag slot.
        for (int i = 0; i < 4; i++) sim.Debug("{\"grant_item\":{\"def_id\":\"" + CardDef + "\"}}");
        for (int i = 0; i < 10; i++) yield return null;
        // MAKE THE ROOM THIS NEEDS. A full bag refuses a grant in SILENCE, and the probe then dies
        // on "no bag card to file" — which names the symptom and blames the screen. The bag is
        // shared with every other probe and with the party's own looting, so headroom is not
        // something to assume; it is something to make. A dev session is the place to spend an
        // item to get it.
        for (int tries = 0; tries < 30 && Held(sim, CardDef) == 0; tries++)
        {
            sim.ApplyInput("{\"DiscardItem\":{\"inv_idx\":0}}");
            for (int i = 0; i < 3; i++) yield return null;
            sim.Debug("{\"grant_item\":{\"def_id\":\"" + CardDef + "\"}}");
            for (int i = 0; i < 3; i++) yield return null;
        }
        if (Held(sim, CardDef) == 0)
        {
            Debug.LogError("ALBUM: could not get a card into the bag even after clearing rows — "
                         + "the grant is being refused for some other reason than space");
            yield break;
        }
        for (int i = 0; i < 10; i++) yield return null;

        bar.CloseAll();
        bar.Toggle("Album");
        for (int i = 0; i < 90; i++) yield return null;

        // --- DEPOSIT ---------------------------------------------------------------------------
        int album0 = Stock(sim, "card_album");
        if (!TapCardIn(bar, "Bag")) { Debug.LogError("ALBUM: no bag card to file"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        int album1 = Stock(sim, "card_album");
        Report("deposit", album1 > album0, $"album {album0} -> {album1}");

        // --- WITHDRAW, and straight back ---------------------------------------------------------
        if (!TapCardIn(bar, "Album")) { Debug.LogError("ALBUM: no album card to select"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        // READ THE ALBUM, NOT THE BAG. Two things make the bag the wrong side to count. Cards
        // STACK: a withdrawn card merges into the stack it came from, so the number of bag rows is
        // unchanged and a working withdraw looked like a refusal. And the bag is the sim's — the
        // party is still fighting and looting through these frames, so its total moves for reasons
        // that have nothing to do with the button. The album only holds what this probe filed.
        int albumBefore = Stock(sim, "card_album");
        if (!Press(bar, "Withdraw")) { Debug.LogError("ALBUM: no Withdraw button"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        int albumAfter = Stock(sim, "card_album");
        // DIRECTION, NOT AN EXACT COUNT. Withdraw takes the selected ROW, and a row is a stack —
        // so the album can lose one card or six depending on what had merged into it. What the
        // button promises is that cards leave the album, and that is what this asserts.
        Report("withdraw", albumAfter < albumBefore,
               $"album {albumBefore} -> {albumAfter}");
        if (!TapCardIn(bar, "Bag")) { Debug.LogError("ALBUM: the withdrawn card did not come back"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;

        ScreenCapture.CaptureScreenshot("/tmp/roi_album.png");
        for (int i = 0; i < 20; i++) yield return null;

        // --- ★ UP ---------------------------------------------------------------------------
        // A FEED EATS A COPY, so the album needs two of them. The legs above deposit one and take
        // it back out, and how many are left is a running total this leg was reading off faith.
        // File copies until there are enough — the bag was granted four at the top for exactly this.
        for (int tries = 0; tries < 6 && Stock(sim, "card_album") < 2; tries++)
        {
            if (!TapCardIn(bar, "Bag")) break;
            for (int i = 0; i < 25; i++) yield return null;
        }
        if (Stock(sim, "card_album") < 2)
        {
            Debug.LogError($"ALBUM: only {Stock(sim, "card_album")} in the album — a feed needs two, "
                         + "and the bag had nothing left to file");
            yield break;
        }
        // FEED A CARD THAT CAN STILL GAIN. `FeedCandidates` needs `star < 5`, and this album has
        // been fed by every earlier run of this probe, so its first row is usually capped and the
        // button opens nothing at all. Pick a row that is under the cap and tap THAT one.
        var rows = All(sim, "card_album");
        int feedRow = -1;
        for (int r = 0; rows != null && r < rows.Length; r++)
            if (rows[r].star < 5 && (rows[r].quantity >= 2 || Same(rows, r) > 1)) { feedRow = r; break; }
        if (feedRow < 0)
        {
            Debug.LogError("ALBUM: every album row is at the 5-star cap or has no second copy — "
                         + "there is no feed left to press");
            yield break;
        }
        if (!TapCardIn(bar, "Album", feedRow)) { Debug.LogError("ALBUM: nothing to feed"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        var t0 = rows[feedRow];
        if (t0 == null) { Debug.LogError("ALBUM: the album emptied before the feed"); yield break; }
        int star0 = t0.star, qty0 = t0.quantity;
        if (!Press(bar, "Level Up")) { Debug.LogError("ALBUM: no Level Up button"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        ScreenCapture.CaptureScreenshot("/tmp/roi_album_feed.png");
        for (int i = 0; i < 20; i++) yield return null;
        if (!PressConfirm("Feed")) { Debug.LogError("ALBUM: the feed was never confirmed"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        // A feed SPLITS the stack: two copies are consumed and ONE promoted copy is banked as its
        // own row, because a 1★ card and a 0★ card of the same def are not the same stack. So the
        // reading is the album as a whole — a row at the new star, and one fewer copy in total.
        bool promoted = false;
        foreach (var c in All(sim, "card_album")) if (c.star == star0 + 1) promoted = true;
        int total1 = Copies(sim, "card_album");
        Report("level up", promoted && total1 == qty0 - 1,
               $"a {star0 + 1}★ row {(promoted ? "appeared" : "did NOT appear")}, "
             + $"copies {qty0} -> {total1}");

        // --- BIND ---------------------------------------------------------------------------
        if (!TapCardIn(bar, "Album")) { Debug.LogError("ALBUM: nothing to bind"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        var before = sim.QueryJson("bind_info:0");
        if (!Press(bar, "Bind")) { Debug.LogError("ALBUM: no Bind button (or it was refused)"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        if (!PressConfirm("Bind")) { Debug.LogError("ALBUM: the bind was never confirmed"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        var after = sim.QueryJson("bind_info:0");
        Report("bind", Done(after) == Done(before) + 1, $"binds done {Done(before)} -> {Done(after)}");

        // --- THE PICKER, which only appears when the choice is real -----------------------------
        // A FOIL copy is a second candidate: it cannot merge onto a plain stack (they are
        // different identities), and feeding it carries the foil across. So the screen has to ask
        // WHICH copy — the one path where a confirmation alone cannot express the question.
        // Two plain copies AND a foil one. The plain pair is what makes the stack able to feed
        // ITSELF; the foil is the second candidate. Two candidates is the whole condition for the
        // picker appearing — the legs above deliberately never met it.
        for (int i = 0; i < 2; i++) sim.Debug("{\"grant_item\":{\"def_id\":\"" + CardDef + "\"}}");
        sim.Debug("{\"grant_item\":{\"def_id\":\"" + CardDef + "\",\"foil\":true}}");
        for (int i = 0; i < 10; i++) yield return null;
        // Reopen: nothing polls, so a card that arrived while the window was already open is not
        // on screen until something asks the sim again. That is the design (a bag nobody is
        // looking at costs nothing), and a probe has to honour it rather than assume a repaint.
        bar.CloseAll();
        bar.Toggle("Album");
        for (int i = 0; i < 60; i++) yield return null;
        // Both bag stacks, filed one at a time — a deposit moves ONE stack, and the plain copies
        // and the foil one are two stacks because a foil never merges onto a plain row.
        for (int f = 0; f < 2; f++)
        {
            if (!TapCardIn(bar, "Bag")) { Debug.LogError("ALBUM: a granted copy never reached the bag"); yield break; }
            for (int i = 0; i < 30; i++) yield return null;
        }

        int plain = -1;
        var stacks = All(sim, "card_album");
        for (int i = 0; i < stacks.Length; i++) if (!stacks[i].foil && stacks[i].quantity >= 2) plain = i;
        if (plain < 0) { Debug.LogError("ALBUM: no plain stack to feed the foil into"); yield break; }
        if (!TapCardAt(bar, "Album", plain)) { Debug.LogError("ALBUM: could not select the plain stack"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        if (!Press(bar, "Level Up")) { Debug.LogError("ALBUM: no Level Up button for the picker leg"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        var picker = Object.FindFirstObjectByType<Roi.UI.UiPicker>();
        if (picker == null) { Debug.LogError("ALBUM: the picker never opened with two candidates"); yield break; }
        ScreenCapture.CaptureScreenshot("/tmp/roi_album_picker.png");
        for (int i = 0; i < 20; i++) yield return null;
        int options = 0;
        foreach (var b in picker.GetComponentsInChildren<Button>(true))
            if (b.gameObject.name == "Option") options++;
        if (!PressIn(picker.gameObject, "Foil")) { Debug.LogError("ALBUM: the foil candidate was not offered"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        if (!PressConfirm("Feed")) { Debug.LogError("ALBUM: the picked feed was never confirmed"); yield break; }
        for (int i = 0; i < 30; i++) yield return null;
        bool gotFoil = false;
        foreach (var c in All(sim, "card_album")) if (c.foil && c.star > 0) gotFoil = true;
        Report("picker", options >= 2 && gotFoil,
               $"{options} candidates offered, a foil promoted row {(gotFoil ? "exists" : "does NOT exist")}");

        Debug.Log("ALBUM done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"ALBUM OK — {what}: {detail}");
        else Debug.LogError($"ALBUM FAILED — {what}: {detail}");
    }

    /// The number of STACKS in a list (one `def_id` each), not the copies in them.
    /// How many of ONE def the bag holds, rows counted. Enough to answer "did the grant land",
    /// which is the only question the setup has.
    static int Held(Roi.Sim.SimSession sim, string defId)
    {
        var j = sim.QueryJson("inventory");
        var needle = "\"def_id\":\"" + defId + "\"";
        int n = 0, i = 0;
        while ((i = j.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }

    /// HOW MANY CARDS, counting the STACKS. Cards are stackable, so a deposit into a def the album
    /// already holds merges and the number of ROWS does not move — three separate legs of this
    /// probe reported a working move as a refusal for that one reason. Summing `quantity` is the
    /// count the moves actually change.
    static int Stock(Roi.Sim.SimSession sim, string key)
    {
        var j = sim.QueryJson(key);
        const string needle = "\"quantity\":";
        int total = 0, i = 0;
        while ((i = j.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0)
        {
            i += needle.Length;
            int k = i;
            while (k < j.Length && char.IsDigit(j[k])) k++;
            if (k > i && int.TryParse(j.Substring(i, k - i), out int v)) total += v;
            i = k;
        }
        return total;
    }

    /// The FIRST stack of a list, through the same DTO the screen reads. Not a hand-rolled
    /// search: `card_album` carries a nested `card_star_ranks` whose entries have a `star` of
    /// their own, and the first `"star":` in the text belongs to one of THOSE — which reported a
    /// fresh card as already 1★ and made a working feed look like a no-op.
    /// How many rows share row `r`'s def — a feeder may be another row of the same card.
    static int Same(Roi.UI.ItemView[] rows, int r)
    {
        int n = 0;
        foreach (var c in rows) if (c.def_id == rows[r].def_id) n++;
        return n;
    }

    static Roi.UI.ItemView First(Roi.Sim.SimSession sim, string key)
    {
        var list = JsonUtility.FromJson<Roi.UI.ItemList>("{\"items\":" + sim.QueryJson(key) + "}");
        return list != null && list.items != null && list.items.Length > 0 ? list.items[0] : null;
    }

    static Roi.UI.ItemView[] All(Roi.Sim.SimSession sim, string key)
    {
        var list = JsonUtility.FromJson<Roi.UI.ItemList>("{\"items\":" + sim.QueryJson(key) + "}");
        return list != null && list.items != null ? list.items : new Roi.UI.ItemView[0];
    }

    /// COPIES, not stacks — the number a feed actually spends.
    static int Copies(Roi.Sim.SimSession sim, string key)
    {
        int n = 0;
        foreach (var c in All(sim, key)) n += Mathf.Max(1, c.quantity);
        return n;
    }

    static int Done(string bindInfoJson) => ReadInt(bindInfoJson, "done");

    static int ReadInt(string json, string field)
    {
        if (string.IsNullOrEmpty(json)) return -1;
        int i = json.IndexOf("\"" + field + "\":", System.StringComparison.Ordinal);
        if (i < 0) return -1;
        i += field.Length + 3;
        int end = i;
        while (end < json.Length && (char.IsDigit(json[end]) || json[end] == '-')) end++;
        return int.TryParse(json.Substring(i, end - i), out var v) ? v : -1;
    }

    /// Tap the first card cell inside the album window's named COLUMN. The two columns hold the
    /// same kind of cell over two different lists, so pressing "a card" without saying which side
    /// would file and unfile the same card forever.
    static bool TapCardIn(Roi.UI.HudBar bar, string column) => TapCardIn(bar, column, 0);

    /// The `nth` drawn cell of a column. The feed leg needs a SPECIFIC row rather than whichever
    /// is drawn first: the album is a dev session's and it accumulates, so the first row is often
    /// a card earlier runs already fed to the 5-star cap — and a capped card has no feed to offer,
    /// so the button opens nothing and the probe reported the bench broken.
    static bool TapCardIn(Roi.UI.HudBar bar, string column, int nth)
    {
        var win = Object.FindFirstObjectByType<Roi.UI.AlbumWindow>();
        if (win == null) return false;
        foreach (Transform child in win.transform)
        {
            if (child.name != column) continue;
            int seen = 0;
            foreach (Transform cell in child)
            {
                var b = cell.GetComponent<Button>();
                if (b == null || !b.isActiveAndEnabled) continue;
                if (seen++ != nth) continue;
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    /// The CONFIRM's own button, not the one behind it. The album's Bind and the confirmation's
    /// Bind carry the same word, and pressing "the first Bind on screen" twice just reopens the
    /// modal — which reads exactly like a bind the sim refused.
    static bool PressConfirm(string label)
    {
        var modal = Object.FindFirstObjectByType<Roi.UI.UiConfirm>();
        if (modal == null) return false;
        foreach (var b in modal.GetComponentsInChildren<Button>(true))
        {
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    /// The Nth cell of a column — the picker leg needs a SPECIFIC stack, not the first one.
    static bool TapCardAt(Roi.UI.HudBar bar, string column, int n)
    {
        var win = Object.FindFirstObjectByType<Roi.UI.AlbumWindow>();
        if (win == null) return false;
        foreach (Transform child in win.transform)
        {
            if (child.name != column) continue;
            int i = 0;
            foreach (Transform cell in child)
            {
                var b = cell.GetComponent<Button>();
                if (b == null || !b.isActiveAndEnabled) continue;
                if (i++ != n) continue;
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    /// A button whose label CONTAINS `word`, inside one subtree.
    static bool PressIn(GameObject root, string word)
    {
        foreach (var b in root.GetComponentsInChildren<Button>(true))
        {
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.Contains(word))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in Object.FindObjectsByType<Button>(
                     FindObjectsInactive.Exclude, FindObjectsSortMode.None))
        {
            if (!b.gameObject.activeInHierarchy || !b.interactable) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }
}
