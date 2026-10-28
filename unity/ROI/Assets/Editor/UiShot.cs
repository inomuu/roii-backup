// Capture the UI as the player sees it — the bar, the Menu grid, and each ported screen — from
// the OPEN Editor, one PNG per state.
//
// It has to be a play-mode screenshot and not a camera read-back: the UI draws on
// ScreenSpaceOverlay canvases, which are composited after every camera, so grabbing the arena
// camera's target texture returns the world with no interface on it at all. `CaptureScreenshot`
// is the only path that sees the overlay, and it finishes at the END of a frame, hence the wait
// between shots rather than a straight loop.
//   echo exec:UiShot.Run > unity/ROI/.roi_cmd
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class UiShot
{
    const string OutDir = "/tmp";
    const int Settle = 90;    // floor: the zone has to build and the sim to spin up
    const int SettleMax = 5400; // ceiling, in case nothing ever drops — shoot anyway
    const int Between = 24;   // frames per state — the screenshot lands a frame or two late

    // "" = the bar alone; "*menu" = the Menu grid popped open; "*slots" = the synthetic cell
    // sheet; anything else = that window.
    static readonly string[] States =
    {
        "Debug", "*menu", "Character", "Party", "Map",
        // The screens added since: two collections that are NOT the bag, the Gil shop, and the
        // lifetime record. All four are sim-side, so they draw fully in a local session.
        "Exemplar", "Regalia", "Shop", "Record", "Options", "Bestiary", "*tooltip",
        // "" = the arena with only its own chrome: the roster, the action bar and the zone
        // readout. LAST, not first: the capture is written at the end of a frame, so a state with
        // no window of its own gets overwritten by whatever opens next.
        "",
    };

    static int _frame, _index = -1, _armed = -1;
    static readonly List<string> _written = new();

    public static void Run()
    {
        // Straight past the login: this shoots the CHROME, and a title screen has none of it.
        SessionState.SetString("ROI_BOOT", "dev");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        if (!EditorApplication.isPlaying)
        {
            EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
            EditorApplication.playModeStateChanged += OnState;
            EditorApplication.EnterPlaymode();
        }
        else Begin();
    }

    static void OnState(PlayModeStateChange s)
    {
        if (s != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= OnState;
        Begin();
    }

    /// Render at the resolution the DISPLAY actually has, not the one the Game view defaults to.
    ///
    /// This is the difference between photographing the UI and photographing a photograph of it.
    /// The Game view was rendering 1280x720 and stretching it 1.67x inside a window on a 2x Retina
    /// panel, so every glyph was resampled twice before anyone saw it — and no font setting can
    /// survive that. 2560x1440 is the backing store of a 1280x720-point window on such a display,
    /// which is what a real build renders into.
    const uint ShotW = 2560, ShotH = 1440;

    static void Begin()
    {
        UnityEditor.PlayModeWindow.SetCustomRenderingResolution(ShotW, ShotH, "ROI capture");
        _frame = 0; _index = -1; _armed = -1; _written.Clear();
        // Sweep leftovers BY NAME, not by the static handle. A recompile between runs clears the
        // static while play mode keeps running, so the previous run's overlay was still on screen
        // with nothing left pointing at it — and it sat on top of every shot that followed.
        foreach (var stale in Object.FindObjectsByType<Canvas>(FindObjectsSortMode.None))
            if (stale.gameObject.name == "SlotPreview") Object.Destroy(stale.gameObject);
        _preview = null;
        EditorApplication.update += Tick;
        Debug.Log("UI SHOTS begin");
    }

    static void Tick()
    {
        if (!EditorApplication.isPlaying) { Finish("play mode left"); return; }
        _frame++;
        if (_frame < Settle) return;
        // Wait for the bag to have something in it. An empty grid is a picture of the LAYOUT and
        // not of the screen — every slot the same black cell, no rarity, no name, no count — and
        // a fresh party needs a minute of fighting before its first drop lands.
        if (_armed < 0)
        {
            if (_frame < SettleMax && !HasLoot()) return;
            _armed = _frame;
            Debug.Log($"UI SHOTS armed at frame {_frame} (loot={HasLoot()})");
        }
        int step = (_frame - _armed) % Between;
        int want = (_frame - _armed) / Between;
        if (want >= States.Length) { Finish($"{_written.Count} written"); return; }

        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null) { if (_frame % 60 == 0) Debug.LogWarning("UI SHOTS: no HudBar yet"); return; }

        if (want != _index && step == 0)
        {
            _index = want;
            Apply(bar, States[want]);
        }
        // Two thirds of the way through the state's frames: late enough that the layout has run
        // and TMP has rasterised any glyph the atlas did not already hold.
        if (step == Between * 2 / 3)
        {
            var name = States[_index] == "" ? "bar" : States[_index].TrimStart('*').ToLowerInvariant();
            var path = Path.Combine(OutDir, $"roi_ui_{_index}_{name}.png");
            ScreenCapture.CaptureScreenshot(path);
            _written.Add(path);
        }
    }

    static bool HasLoot()
    {
        var d = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var s = d != null ? d.Session : null;
        if (s == null) return false;
        var json = s.QueryJson("inventory");
        return !string.IsNullOrEmpty(json) && json.Trim().Length > 2;
    }

    /// Drive the bar the way a player would. `SetMenuOpen` is private — reflection here rather
    /// than widening the API for a screenshot tool.
    static void Apply(Roi.UI.HudBar bar, string state)
    {
        var setMenu = typeof(Roi.UI.HudBar).GetMethod(
            "SetMenuOpen", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
        setMenu?.Invoke(bar, new object[] { false });
        bar.CloseAll();
        if (_preview != null) Object.Destroy(_preview);
        if (state == "*menu") setMenu?.Invoke(bar, new object[] { true });
        else if (state == "*slots") _preview = BuildSlotSheet();
        // The picker is half of what the Provisions screen IS — the no-drag flow is tap a slot,
        // tap an item — so it gets photographed too, by pressing the slot the way a player would
        // rather than by calling the method behind it.
        else if (state == "*provpick") { bar.Toggle("Provisions"); TapFirstEmptySlot(); }
        // The TOOLTIP, shown by hand. It is a hover, and a probe has no pointer to hover with —
        // but what is under test is what the panel says about an item, not whether Unity raises
        // PointerEnter.
        else if (state == "*tooltip") { bar.Toggle("Inventory"); ShowFirstTooltip(); }
        else if (state != "") bar.Toggle(state);
    }

    static void ShowFirstTooltip()
    {
        var grid = Object.FindFirstObjectByType<Roi.UI.ItemGridWindow>();
        if (grid == null) { Debug.LogWarning("UI SHOTS: no bag window"); return; }
        foreach (var slot in grid.GetComponentsInChildren<Roi.UI.ItemSlot>(true))
        {
            var item = slot.Item;
            if (item == null || string.IsNullOrEmpty(item.def_id)) continue;
            var rt = (RectTransform)slot.transform;
            var pos = RectTransformUtility.WorldToScreenPoint(null, rt.position);
            Roi.UI.ItemTooltip.Show(item, pos);
            Debug.Log($"UI SHOTS tooltip on {item.def_id}");
            return;
        }
        Debug.LogWarning("UI SHOTS: the bag is empty, nothing to describe");
    }

    static void TapFirstEmptySlot()
    {
        var win = Object.FindFirstObjectByType<Roi.UI.ProvisionsWindow>();
        if (win == null) { Debug.LogWarning("UI SHOTS: no provisions window"); return; }
        foreach (var b in win.GetComponentsInChildren<UnityEngine.UI.Button>(true))
        {
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text == "+") { b.onClick.Invoke(); return; }
        }
        Debug.LogWarning("UI SHOTS: no empty provision slot found");
    }

    static GameObject _preview;

    /// EVERY CASE OF A CELL AT ONCE. A live bag shows whatever the RNG has handed the party — one
    /// piece of junk, an hour in — so the rarity ramp, the foil override, the bound mark, refine,
    /// stacks and the stat line would each be judged on a screenshot taken days apart, if at all.
    /// Synthetic rows put the whole design on one screen and make a regression in it obvious.
    static GameObject BuildSlotSheet()
    {
        var root = new GameObject("SlotPreview");
        var canvas = root.AddComponent<Canvas>();
        canvas.renderMode = RenderMode.ScreenSpaceOverlay;
        canvas.sortingOrder = 60;
        var scaler = root.AddComponent<UnityEngine.UI.CanvasScaler>();
        scaler.uiScaleMode = UnityEngine.UI.CanvasScaler.ScaleMode.ConstantPixelSize;

        var bg = Roi.UI.UiTheme.Flat(root.transform, "Sheet", Roi.UI.UiTheme.Panel);
        var br = bg.rectTransform;
        br.anchorMin = new Vector2(0f, 0f); br.anchorMax = new Vector2(1f, 1f);
        br.offsetMin = new Vector2(0f, 80f); br.offsetMax = new Vector2(0f, -60f);

        var head = Roi.UI.UiTheme.Caps(br, "Head", "Cell states", Roi.UI.UiTheme.HeadSize,
                                       Roi.UI.UiTheme.TextMid, TMPro.TextAlignmentOptions.TopLeft);
        var hr = head.rectTransform;
        hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
        hr.pivot = new Vector2(0f, 1f);
        hr.sizeDelta = new Vector2(400f, 28f);
        hr.anchoredPosition = new Vector2(40f, -28f);

        var items = new List<Roi.UI.ItemView>
        {
            null,
            new Roi.UI.ItemView { def_id = "junk_moss_gel", name = "Moss Gel Bead", kind = "Junk",
                                  rarity = "Common", quantity = 37 },
            new Roi.UI.ItemView { def_id = "sword_iron", name = "Iron Longsword", kind = "Weapon",
                                  rarity = "Uncommon", atk_bonus = 24, graded_atk = 31, grade_step = 1, star = 2 },
            new Roi.UI.ItemView { def_id = "staff_oak", name = "Oakheart Staff", kind = "Weapon",
                                  rarity = "Rare", matk_bonus = 41, graded_matk = 54, grade_step = 2, star = 3, bound = true },
            new Roi.UI.ItemView { def_id = "plate_dawn", name = "Dawnward Plate", kind = "Armor",
                                  rarity = "Mythical", def_bonus = 88, graded_def = 193, grade_step = 3, star = 5 },
            new Roi.UI.ItemView { def_id = "blade_solar", name = "Solar Edge of the First Watch",
                                  kind = "Weapon", rarity = "Legendary", atk_bonus = 210, graded_atk = 600, grade_step = 4 },
            new Roi.UI.ItemView { def_id = "relic_void", name = "Voidsong Relic", kind = "Armor",
                                  rarity = "Immortal", def_bonus = 340, bound = true, star = 5 },
            new Roi.UI.ItemView { def_id = "card_slime", name = "Meadow Slime", kind = "Card",
                                  rarity = "Rare", foil = true, quantity = 2 },
        };
        float x = 40f;
        foreach (var it in items)
        {
            var slot = Roi.UI.ItemSlot.Create(br);
            slot.SetItem(it);
            var rt = (RectTransform)slot.transform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.anchoredPosition = new Vector2(x, -72f);
            x += Roi.UI.ItemSlot.SlotW + 14f;
        }

        // The CARD cells, for the same reason: a card drops at a weight of 1 in 50,000, so no play
        // session will hand one over to be looked at.
        Roi.UI.UiTheme.Caps(br, "Head2", "Card states", Roi.UI.UiTheme.HeadSize,
                            Roi.UI.UiTheme.TextMid, TMPro.TextAlignmentOptions.TopLeft);
        var h2 = br.Find("Head2").GetComponent<TMPro.TextMeshProUGUI>().rectTransform;
        h2.anchorMin = new Vector2(0f, 1f); h2.anchorMax = new Vector2(1f, 1f);
        h2.pivot = new Vector2(0f, 1f);
        h2.sizeDelta = new Vector2(400f, 28f);
        h2.anchoredPosition = new Vector2(40f, -200f);

        var cards = new List<Roi.UI.ItemView>
        {
            new Roi.UI.ItemView { def_id = "card_pinkling", name = "Pinkling Card", kind = "Card",
                                  rarity = "Common", card_slot = "Weapon", star = 1 },
            new Roi.UI.ItemView { def_id = "card_chomb", name = "Chomb Card", kind = "Card",
                                  rarity = "Rare", card_slot = "Armor", star = 3, quantity = 4 },
            new Roi.UI.ItemView { def_id = "card_wraith", name = "Duskmourn Wraith Card", kind = "Card",
                                  rarity = "Mythical", card_slot = "Accessory1", star = 5, foil = true },
        };
        float cx = 40f;
        foreach (var c in cards)
        {
            Roi.UI.CardsWindow.Card(br, cx, 240f, c, c.quantity > 1 ? "\u00d7" + c.quantity : null, null);
            cx += 114f;
        }
        Roi.UI.CardsWindow.EmptyCard(br, cx, 240f, "empty");
        Roi.UI.CardsWindow.EmptyCard(br, cx + 114f, 240f, "no socket");
        return root;
    }

    static void Finish(string why)
    {
        EditorApplication.update -= Tick;
        if (_preview != null) Object.Destroy(_preview);
        _preview = null;
        Debug.Log($"UI SHOTS done: {why}");
        // Leave play mode. A session left running keeps fighting, and the next run then starts
        // from a party that has been at it for ten minutes — or has wiped, which is how a capture
        // came back with an empty arena and an empty party.
        if (EditorApplication.isPlaying) EditorApplication.isPlaying = false;
    }
}
