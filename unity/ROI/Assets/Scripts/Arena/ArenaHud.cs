// The arena's 2D overlay — HP and cast bars, damage numbers, multi-hit popups, skill banners,
// the loot feed. Ported from godot/proto/arena3d/arena3d_draw.gd, which draws all of it into a
// CanvasLayer and positions it with Camera3D.unproject_position(); the reasoning there is worth
// repeating: putting these in the 3D world makes them fight perspective and lose text crispness.
//
// SCREEN SPACE, PROJECTED. Everything anchors to a world point, is projected once per frame, and
// is drawn in screen pixels. Two conventions flip on the way across and every one of them is a
// bug if missed:
//   * Godot's screen y grows DOWNWARD, Unity's grows UP. Every "- rise" there is "+ rise" here,
//     and a box drawn ABOVE an anchor has its rect built the other way round.
//   * `is_position_behind` becomes `WorldToScreenPoint(...).z <= 0`.
//
// RETAINED, NOT IMMEDIATE. Godot re-draws the whole overlay each frame; uGUI cannot, so the
// element pools below give the same feel — every frame walks the same lists, takes elements off
// a pool, and deactivates whatever it did not use. Nothing is created after the pool warms up.
//
// SDF TEXT, because the numbers MOVE. A raster font is crisp only at whole pixels and on its own
// size grid, so snapping to that grid is what made a rising number step instead of glide, and not
// snapping made it soft. Signed-distance glyphs are sharp at any size and any sub-pixel position —
// which is exactly why the Godot face is MSDF — so the rise is smooth and the number is crisp at
// the same time. The action bar keeps the raster font: nothing there moves.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.Arena
{
    public sealed class ArenaHud : MonoBehaviour
    {
        public bool PresentationHeld;
        // --- the numbers, from render_data.gd + arena3d_scene.gd -------------------------------
        const float PopupLifetime = 0.7f;      // ArenaRenderData.POPUP_LIFETIME
        const float PopupRisePx = 36f;         // ArenaRenderData.POPUP_RISE_PX
        const int PopupFontSize = 26;          // arena3d_scene's own, bigger than the 2D path's 16
        const float PopupPopScale = 3f;        // bursts in at this scale...
        const float PopupPopDur = 0.3f;        // ...easing to 1.0 over this
        const int PopupOutlinePx = 4;
        const float NumberOpticalScale = 1.3f; // match the smaller cap height of the display serif

        // THE BREAK WORD. Sized and timed apart from the popups because it is not one: a popup is a
        // number that gets out of the way, this is a title that stops the frame. It SNAPS DOWN into
        // place rather than bursting up out of nothing — the word arrives already large and settles,
        // which is what makes it land like an announcement instead of another damage number.
        const float BannerLifetime = 1.5f;
        const int BannerFontSize = 112;        // display serif; its cap height is smaller than its em
        const float BannerInitialScale = 1.14f;
        const float BannerPopScale = 1.55f;    // arrives this big...
        const float BannerPopDur = 0.13f;      // ...settling to 1.0 over this
        const float BannerWhitenStart = 0.9f, BannerWhitenEnd = 1.08f;
        const float BannerSoftenStart = 1.0f, BannerSoftenEnd = 1.22f;
        const float BannerFadeStart = 1.18f;   // whiten first, then a shorter luminous fade
        const float BannerDriftPx = 18f;       // one-way rightward drift after the entry settles
        const float BannerDriftStart = 0.25f;
        /// The soft copy drawn behind the word. The HUD canvas is an overlay, so post-processing
        /// never reaches it and the bloom that lights the arena cannot light this — the glow has to
        /// be drawn, not rendered.
        const float BannerGlowScale = 1f;       // soften the SDF edge; never double the glyph silhouette
        const float BannerGlowAlpha = 0.32f;
        const int BannerAmountFontSize = 64;
        /// ABOVE the word, not under it (2026-09-09). The number is the payoff — the word only names
        /// what happened — so it goes where the eye lands first and at a size the swing numbers
        /// beside it cannot be mistaken for. Screen y grows upward here, so this clears the word's
        /// cap height (font 112, drawn 0.30em below the anchor) with a gap.
        const float BannerAmountRisePx = 78f;
        /// Gap between one letter landing and the next starting. The whole word is in before the
        /// hold ends, so the stagger costs the announcement no time — it only gives it a direction
        /// to be read in.
        const float BannerLetterStagger = 0.025f;
        /// How far below its resting place a letter starts. It arrives large and low and settles up
        /// into line, so the word assembles rather than fading on.
        const float BannerLetterRisePx = 18f;
        /// A TRASH BREAK IS NOT AN ANNOUNCEMENT. At full size the word covered the monster it was
        /// about and fired every few seconds; a boss break has to be able to feel bigger than the
        /// one before it, and it cannot if the ordinary one already fills the screen. One
        /// multiplier so the whole block — glyphs, outline, the rise, the number under it — shrinks
        /// together and keeps its proportions.
        const float BannerTrashScale = 0.55f;
        static readonly Color ColBreakWord = new Color(0.045f, 0.075f, 0.13f);
        static readonly Color ColBreakWordLower = new Color(0.18f, 0.31f, 0.40f);
        static readonly Color ColBreakGlow = new Color(0.55f, 0.80f, 1f);

        const float SkillLabelDur = 2f, SkillLabelFade = 0.4f, SkillLabelPop = 0.12f;
        const int SkillLabelSize = 13;
        const float SkillLabelYCells = 0.85f;  // NumberFx.SKILL_LABEL_Y_CELLS
        const float SkillLabelLift = 0.25f;    // arena3d_scene.SKILL_LABEL_LIFT, the 3D-only nudge

        const float LootLife = 3.2f, LootFade = 0.8f, LootLineH = 42f;
        const float LootTop = 116f + 8f;       // UILayout.TOP (tab strip + HUD bar) + 8
        const int LootFont = 18;
        const float LootIconW = 26f;           // reserved gutter; no icon art yet
        const int LootMax = 6;
        static readonly Vector2 LootPillPad = new Vector2(16f, 9f);

        // MultiHitPopup: white per-hit numbers in one column + a yellow running total above.
        const float MhLifetime = 0.9f, MhPunch = 0.9f, MhPunchDur = 0.12f, MhRisePx = 24f;

        // Multi-hit pacing as fractions of the attacker's real swing interval (ASPD).
        const float SwingWithinFrac = 0.22f, SwingBetweenFrac = 0.5f;
        const float SwingGapMin = 0.05f, SwingGapMax = 0.45f, SwingBetweenMax = 0.1f;

        static readonly Color ColEnemyHit = Color.white;
        static readonly Color ColAllyHit = new Color(1.00f, 0.45f, 0.40f);
        static readonly Color ColHeal = new Color(0.45f, 1.00f, 0.55f);
        static readonly Color ColMiss = new Color(1.00f, 0.30f, 0.28f);
        static readonly Color ColDodge = new Color(0.80f, 0.90f, 1.00f);
        static readonly Color ColGuard = new Color(0.55f, 0.75f, 1.00f);
        static readonly Color ColYellow = new Color(1.0f, 0.92f, 0.25f);
        /// Godot's break amber, exactly — the number has to be told apart from the swing beside it.
        static readonly Color ColBreak = new Color(1.00f, 0.80f, 0.25f);

        // Rarity colours DARKENED for a light pill, exactly as the Godot banner does it.
        static readonly Dictionary<string, Color> LootRarity = new Dictionary<string, Color>
        {
            { "Common", new Color(0.28f, 0.28f, 0.33f) }, { "Uncommon", new Color(0.12f, 0.48f, 0.18f) },
            { "Rare", new Color(0.10f, 0.34f, 0.62f) },   { "Mythical", new Color(0.40f, 0.16f, 0.66f) },
            { "Legendary", new Color(0.66f, 0.42f, 0.03f) }, { "Immortal", new Color(0.66f, 0.10f, 0.55f) },
        };
        static readonly Color LootPill = new Color(0.96f, 0.96f, 0.97f, 0.94f);
        static readonly Color LootPillEdge = new Color(0.72f, 0.73f, 0.78f, 0.9f);

        /// The debug switch ArenaRenderData.SHOW_DAMAGE_NUMBERS is: numbers off, status text on.
        public bool ShowDamageNumbers = true;

        Camera _cam;
        TMP_FontAsset _font, _breakFont;
        Material _matPlain, _matOutlined, _matNumbers;
        Material _matBreak, _matBreakGlow, _matBreakFallback, _matBreakFallbackGlow;
        TextMeshProUGUI _measure;
        Canvas _canvas;
        RectTransform _back, _front;   // boxes and bars, then text — uGUI draws in hierarchy order
        readonly List<TextMeshProUGUI> _texts = new();
        readonly List<Image> _images = new();
        int _textCursor, _imageCursor;

        // --- what is on screen ------------------------------------------------------------------
        struct Bar
        {
            public Vector3 Head;        // world point at the top of the sprite
            public Vector3 HeadRight;   // the same point offset half a bar width along camera x
            public float Ratio;
            public float CastFrac;      // < 0 = not casting
            public float Break;         // toughness left, 0-1; < 0 = this body carries no gauge
        }
        readonly List<Bar> _bars = new();

        class Popup { public long Id; public Vector3 World; public string Text; public Color Color; public float Age; public bool IsDamage; }
        readonly List<Popup> _popups = new();
        class Banner
        {
            public long Id; public Vector3 World; public string Text; public long Amount; public float Age;
            /// Each letter's centre, relative to the middle of the word. Measured ONCE — from the
            /// cumulative width of the prefix, so the real advances and kerning are kept — and at
            /// the SETTLED size, so a letter's pop scales it in place instead of shifting the ones
            /// after it. Built on the first frame drawn rather than at spawn: the measuring label
            /// belongs to the HUD's own layout, which need not exist yet when a hit lands.
            public float[] X;
            public bool Serif;
            public float Size = 1f;   // 1 = boss; trash breaks come in smaller
        }
        readonly List<Banner> _banners = new();

        class MultiHit
        {
            public long Id; public Vector3 World;
            public List<(int value, float delay, float age, bool landed)> Hits = new();
            public int Total, Landed, Expected;
            public float AccumAge = 999f, Pop;
        }
        readonly List<MultiHit> _multi = new();
        readonly Dictionary<string, MultiHit> _multiByKey = new();

        class SkillLabel { public long Id; public string Text; public float Age; }
        readonly List<SkillLabel> _skillLabels = new();

        class LootLine { public string Text; public Color Color; public float Age; }
        readonly List<LootLine> _loot = new();

        // The monster under the cursor, named below its feet. Pushed per frame like the bars.
        const float HoverNameDrop = 2f;   // px below the feet before the name's own ascent
        const int HoverNameSize = 13;
        string _hoverName; Vector3 _hoverWorld;

        /// Where an anchored popup should sit NOW: riding the live actor while it exists, falling
        /// back to the frozen world point once it is gone — a corpse despawning must not drag its
        /// last number away with it (arena3d_draw._popup_anchor).
        public System.Func<long, Vector3, Vector3> AnchorOf;

        /// The caster's GROUND position, or false if it is gone. A banner over a despawned actor
        /// is skipped rather than drawn at the origin, which is what a sentinel position would do.
        public delegate bool GroundLookup(long id, out Vector3 ground);
        public GroundLookup TryActorGround;

        public static ArenaHud Create(Transform parent, Camera cam)
        {
            var go = new GameObject("ArenaHud");
            go.transform.SetParent(parent, false);
            var hud = go.AddComponent<ArenaHud>();
            hud.Build(cam);
            return hud;
        }

        void Build(Camera cam)
        {
            _cam = cam;
            // The SAME face the Godot HUD uses (Pixel Operator Bold, CC0), as an SDF asset and out
            // of Resources so a build gets it too.
            _font = Resources.Load<TMP_FontAsset>("Fonts/PixelOperator-Bold SDF");
            if (_font == null)
            {
                Debug.LogWarning("ArenaHud: Resources/Fonts/PixelOperator-Bold SDF missing — falling back");
                _font = TMP_Settings.defaultFontAsset;
            }
            // TWO SHARED materials rather than a per-object instance: setting `outlineWidth` on a
            // TMP_Text instantiates its material, and the pool would end up with one material each.
            _matPlain = _font.material;
            _matOutlined = new Material(_font.material) { name = "ArenaHud outlined" };
            _matOutlined.EnableKeyword(TMPro.ShaderUtilities.Keyword_Outline);
            _matOutlined.SetFloat(TMPro.ShaderUtilities.ID_OutlineWidth, 0.22f);
            _matOutlined.SetColor(TMPro.ShaderUtilities.ID_OutlineColor, Color.black);

            // The licensed display face is used by BREAK and combat numerals. A localized word that
            // is absent from its atlas keeps the normal HUD font and its established fallbacks.
            _breakFont = Resources.Load<TMP_FontAsset>("Fonts/CrimsonText-Break SDF");
            _matBreakFallback = CreateBreakMaterial(_font, false);
            _matBreakFallbackGlow = CreateBreakMaterial(_font, true);
            if (_breakFont != null)
            {
                _matBreak = CreateBreakMaterial(_breakFont, false);
                _matBreakGlow = CreateBreakMaterial(_breakFont, true);
            }

            // The same licensed serif, with its own light-face numeric material. Never reuse the
            // navy BREAK-word material or change the regular HUD's outline.
            _matNumbers = new Material((_breakFont != null ? _breakFont : _font).material)
                { name = "Arena damage numbers" };
            _matNumbers.EnableKeyword(ShaderUtilities.Keyword_Outline);
            _matNumbers.SetColor(ShaderUtilities.ID_FaceColor, Color.white);
            _matNumbers.SetColor(ShaderUtilities.ID_OutlineColor, new Color(.06f, .065f, .08f, .95f));
            _matNumbers.SetFloat(ShaderUtilities.ID_OutlineWidth, .12f);
            _matNumbers.SetFloat(ShaderUtilities.ID_OutlineSoftness, .015f);
            _matNumbers.SetFloat(ShaderUtilities.ID_FaceDilate, .035f);
            _matNumbers.EnableKeyword("UNDERLAY_ON");
            _matNumbers.SetColor("_UnderlayColor", new Color(.035f, .04f, .055f, .6f));
            _matNumbers.SetFloat("_UnderlayOffsetX", .06f);
            _matNumbers.SetFloat("_UnderlayOffsetY", -.10f);
            _matNumbers.SetFloat("_UnderlayDilate", .025f);
            _matNumbers.SetFloat("_UnderlaySoftness", .10f);

            var canvasGo = new GameObject("HudCanvas");
            canvasGo.transform.SetParent(transform, false);
            _canvas = canvasGo.AddComponent<Canvas>();
            _canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            // NOT pixelPerfect: it snaps every element to whole pixels, which is what made a
            // rising number step. SDF glyphs do not need the snap to stay sharp.
            _canvas.pixelPerfect = false;
            // WITH THE WORLD, at 0: these decorate the fight, so a window (10) covers them and
            // the action bar (40) is never covered. The convention is Godot's own layer order.
            _canvas.sortingOrder = 0;
            // Screen pixels ARE canvas pixels: the projection below hands over raw screen
            // coordinates, and a scaler would silently multiply them.
            var scaler = canvasGo.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ConstantPixelSize;
            scaler.scaleFactor = 1f;
            // Two layers rather than one: elements come off pools in whatever order a frame emits
            // them, and uGUI draws by sibling index. Splitting them is what guarantees a loot pill
            // never lands on top of the name written on it.
            _back = NewLayer(canvasGo.transform, "Back");
            _front = NewLayer(canvasGo.transform, "Front");
        }

        static Material CreateBreakMaterial(TMP_FontAsset font, bool glow)
        {
            var mat = new Material(font.material) { name = glow ? "BREAK soft edge" : "BREAK silver outline" };
            mat.EnableKeyword(ShaderUtilities.Keyword_Outline);
            mat.SetColor(ShaderUtilities.ID_FaceColor, Color.white);
            mat.SetColor(ShaderUtilities.ID_OutlineColor, glow ? Color.white : new Color(0.94f, 0.99f, 1f));
            mat.SetFloat(ShaderUtilities.ID_OutlineWidth, glow ? 0.18f : 0.10f);
            mat.SetFloat(ShaderUtilities.ID_OutlineSoftness, glow ? 0.55f : 0.015f);
            mat.SetFloat(ShaderUtilities.ID_FaceDilate, 0.015f);
            if (!glow)
            {
                mat.EnableKeyword("UNDERLAY_ON");
                mat.SetColor("_UnderlayColor", new Color(0.035f, 0.08f, 0.16f, 0.7f));
                mat.SetFloat("_UnderlayOffsetX", 0.12f);
                mat.SetFloat("_UnderlayOffsetY", -0.20f);
                mat.SetFloat("_UnderlayDilate", 0.15f);
                mat.SetFloat("_UnderlaySoftness", 0.18f);
            }
            return mat;
        }

        // ArenaView keeps simulating while Companion/Home hide this object. LateUpdate then
        // stops, so transient presentation data must neither survive nor accumulate while hidden.
        void OnDisable()
        {
            _bars.Clear(); _popups.Clear(); _banners.Clear();
            _multi.Clear(); _multiByKey.Clear(); _skillLabels.Clear(); _loot.Clear();
            _hoverName = null;
            _textCursor = _imageCursor = 0;
            foreach (var text in _texts) if (text != null) text.gameObject.SetActive(false);
            foreach (var image in _images) if (image != null) image.gameObject.SetActive(false);
        }

        void OnDestroy()
        {
            // All are HUD-owned clones; the font assets and their shared materials are untouched.
            if (_matOutlined != null) Destroy(_matOutlined);
            if (_matNumbers != null) Destroy(_matNumbers);
            if (_matBreak != null) Destroy(_matBreak);
            if (_matBreakGlow != null) Destroy(_matBreakGlow);
            if (_matBreakFallback != null) Destroy(_matBreakFallback);
            if (_matBreakFallbackGlow != null) Destroy(_matBreakFallbackGlow);
        }

        static RectTransform NewLayer(Transform parent, string name)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var rt = (RectTransform)go.transform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.zero; rt.offsetMax = Vector2.zero;
            return rt;
        }

        public void SetCamera(Camera cam) => _cam = cam;

        // --- what the arena feeds in --------------------------------------------------------

        /// One live actor's bar, pushed every frame before the layout pass (the list is cleared
        /// by the layout, so a dead or despawned actor simply stops being pushed).
        public void PushBar(Vector3 head, Vector3 headRight, float ratio, float castFrac, float breakFrac)
        {
            if (!isActiveAndEnabled) return;
            _bars.Add(new Bar { Head = head, HeadRight = headRight, Ratio = ratio,
                                CastFrac = castFrac, Break = breakFrac });
        }

        public void SpawnDamage(long id, Vector3 world, long value, bool onPlayer)
        {
            if (!isActiveAndEnabled) return;
            _popups.Add(new Popup { Id = id, World = world, Text = value.ToString(), IsDamage = true,
                                   Color = onPlayer ? ColAllyHit : ColEnemyHit });
        }

        public void SpawnText(long id, Vector3 world, string label, Color color)
        {
            if (!isActiveAndEnabled) return;
            _popups.Add(new Popup { Id = id, World = world, Text = label, Color = color });
        }

        /// The BREAK announcement over `id`. One at a time per target: a monster whose gauge breaks
        /// twice in a frame is one break as far as the screen is concerned.
        public void SpawnBreakBanner(long id, Vector3 world, string text, long amount, bool boss)
        {
            if (!isActiveAndEnabled) return;
            for (int i = _banners.Count - 1; i >= 0; i--) if (_banners[i].Id == id) _banners.RemoveAt(i);
            _banners.Add(new Banner { Id = id, World = world, Text = text, Amount = amount,
                                      Size = boss ? 1f : BannerTrashScale });
        }

        public void SpawnHeal(long id, Vector3 world, long amount)
        {
            if (!isActiveAndEnabled) return;
            _popups.Add(new Popup { Id = id, World = world, Text = "+" + amount, Color = ColHeal });
        }

        public static Color MissColor => ColMiss;
        public static Color DodgeColor => ColDodge;
        public static Color GuardColor => ColGuard;
        public static Color BreakColor => ColBreak;

        /// Get or start the popup for one volley key and add a hit to it. `expected > 0` is a
        /// known multi-hit (the yellow total shows from the first number); 0 is an incremental
        /// feeder — bolts, damage fields — where the total waits for a second number.
        public void FeedMultiHit(string key, long id, Vector3 world, int value, int expected, float delay = 0f)
        {
            if (!isActiveAndEnabled) return;
            if (!_multiByKey.TryGetValue(key, out var m) || m == null)
            {
                m = new MultiHit { Id = id, World = world, Expected = expected };
                _multiByKey[key] = m;
                _multi.Add(m);
            }
            m.World = world;
            if (expected > 0) m.Expected = expected;
            m.Hits.Add((value, delay, 0f, false));
        }

        /// RO-style banner: the skill name holds above the caster's head, and a re-cast refreshes
        /// the SAME banner rather than stacking a second one.
        public void SpawnSkillLabel(long id, string text)
        {
            if (!isActiveAndEnabled) return;
            foreach (var l in _skillLabels)
                if (l.Id == id) { l.Text = text; l.Age = 0f; return; }
            _skillLabels.Add(new SkillLabel { Id = id, Text = text, Age = 0f });
        }

        /// Newest at the top; the oldest line drops off rather than filling the screen.
        public void PushLoot(string name, int quantity, string rarity, Color? tint = null)
        {
            if (!isActiveAndEnabled) return;
            var col = tint ?? (LootRarity.TryGetValue(rarity ?? "", out var c) ? c : Color.white);
            col.a = 1f;
            _loot.Insert(0, new LootLine { Text = quantity > 1 ? $"{name} x{quantity}" : name, Color = col });
            while (_loot.Count > LootMax) _loot.RemoveAt(_loot.Count - 1);
        }

        /// The monster the cursor is over, or null for none. Cleared by the layout, so a frame
        /// that does not push one simply has no name.
        public void PushHoverName(string name, Vector3 groundWorld)
        {
            if (!isActiveAndEnabled) return;
            _hoverName = name; _hoverWorld = groundWorld;
        }

        /// Pacing for one multi-hit group, from the attacker's effective swing interval.
        public static void SwingGaps(long effAtkSpeedMilli, out float within, out float between)
        {
            float swing = effAtkSpeedMilli <= 0 ? 0.4f : Mathf.Clamp(100f / effAtkSpeedMilli, 0.1f, 2f);
            within = Mathf.Clamp(swing * SwingWithinFrac, SwingGapMin, SwingGapMax);
            between = Mathf.Clamp(swing * SwingBetweenFrac, SwingGapMin, SwingBetweenMax);
        }

        // --- the frame ------------------------------------------------------------------------

        void LateUpdate()
        {
            if (PresentationHeld) return;
            float dt = Time.deltaTime;
            Age(dt);
            _textCursor = 0; _imageCursor = 0;
            if (_cam != null)
            {
                DrawBars();
                DrawLoot();
                DrawHoverName();
                DrawPopups();
                DrawMultiHits();
                DrawSkillLabels();
                DrawBanners();      // last: the announcement goes over the numbers, not under them
            }
            _bars.Clear();
            _hoverName = null;
            for (int i = _textCursor; i < _texts.Count; i++) _texts[i].gameObject.SetActive(false);
            for (int i = _imageCursor; i < _images.Count; i++) _images[i].gameObject.SetActive(false);
        }

        void Age(float dt)
        {
            for (int i = _popups.Count - 1; i >= 0; i--)
            {
                _popups[i].Age += dt;
                if (_popups[i].Age >= PopupLifetime) _popups.RemoveAt(i);
            }
            for (int i = _banners.Count - 1; i >= 0; i--)
            {
                // Match the focus/light clock even when a slow frame caps Time.deltaTime.
                _banners[i].Age += Time.unscaledDeltaTime;
                if (_banners[i].Age >= BannerLifetime) _banners.RemoveAt(i);
            }
            for (int i = _skillLabels.Count - 1; i >= 0; i--)
            {
                _skillLabels[i].Age += dt;
                if (_skillLabels[i].Age >= SkillLabelDur) _skillLabels.RemoveAt(i);
            }
            for (int i = _loot.Count - 1; i >= 0; i--)
            {
                _loot[i].Age += dt;
                if (_loot[i].Age >= LootLife) _loot.RemoveAt(i);
            }
            for (int i = _multi.Count - 1; i >= 0; i--)
            {
                var m = _multi[i];
                m.AccumAge += dt;
                m.Pop = Mathf.Max(0f, m.Pop - dt * 4f);
                for (int h = m.Hits.Count - 1; h >= 0; h--)
                {
                    var hit = m.Hits[h];
                    hit.age += dt;
                    // A hit LANDS once its stagger delay elapses: it shows and joins the total.
                    if (!hit.landed && hit.age >= hit.delay)
                    {
                        hit.landed = true;
                        m.Total += hit.value; m.Landed++; m.AccumAge = 0f; m.Pop = 1f;
                    }
                    if (hit.landed && hit.age >= hit.delay + MhLifetime) m.Hits.RemoveAt(h);
                    else m.Hits[h] = hit;
                }
                if (m.Hits.Count == 0 && m.AccumAge > MhLifetime)
                {
                    _multi.RemoveAt(i);
                    var dead = new List<string>();
                    foreach (var kv in _multiByKey) if (kv.Value == m) dead.Add(kv.Key);
                    foreach (var k in dead) _multiByKey.Remove(k);
                }
            }
        }

        bool Project(Vector3 world, out Vector2 screen)
        {
            var sp = _cam.WorldToScreenPoint(world);
            screen = new Vector2(sp.x, sp.y);
            return sp.z > 0f;   // Godot's is_position_behind
        }

        void DrawBars()
        {
            foreach (var b in _bars)
            {
                if (!Project(b.Head, out var p)) continue;
                Project(b.HeadRight, out var right);
                // The bar tracks the PROJECTED half-width, so it shrinks with distance the way a
                // world-space bar would, without actually being in the world.
                float w = Mathf.Max(8f, Mathf.Abs(right.x - p.x) * 2f);
                const float h = 4f;
                Rect(new Vector2(p.x, p.y - 1f), new Vector2(w + 2f, h + 2f), new Color(0.05f, 0.05f, 0.05f), new Vector2(0.5f, 0f));
                Rect(new Vector2(p.x, p.y), new Vector2(w, h), new Color(0.10f, 0.10f, 0.10f), new Vector2(0.5f, 0f));
                // The same green -> red ramp the 2D renderer uses.
                var fg = Color.Lerp(new Color(0.35f, 0.85f, 0.35f), new Color(0.95f, 0.30f, 0.25f), 1f - b.Ratio);
                Rect(new Vector2(p.x - w * 0.5f, p.y), new Vector2(w * b.Ratio, h), fg, new Vector2(0f, 0f));
                // THE BREAK GAUGE, under the HP bar. Yellow, thinner, and drawn even when full:
                // it is the only thing on screen that says how close a break is, and a bar that
                // only appears once it is already draining answers the question too late. While it
                // is EMPTY the target is broken and the same bar is the recovery timer refilling —
                // one bar, two readings, which is why the sim paces the refill across recovery
                // rather than snapping it back (`tick::refill_toughness`).
                if (b.Break >= 0f)
                {
                    const float bh = 3f;
                    float by = p.y - bh - 2f;
                    Rect(new Vector2(p.x, by), new Vector2(w, bh), new Color(0.10f, 0.09f, 0.05f), new Vector2(0.5f, 0f));
                    if (b.Break > 0f)
                        Rect(new Vector2(p.x - w * 0.5f, by), new Vector2(w * b.Break, bh),
                             new Color(1.00f, 0.82f, 0.20f), new Vector2(0f, 0f));
                }
                if (b.CastFrac >= 0f)
                {
                    // Thin blue, just above the HP bar, filling as the wind-up completes.
                    const float ch = 3f;
                    float cy = p.y + h + 3f;
                    Rect(new Vector2(p.x, cy), new Vector2(w, ch), new Color(0.08f, 0.08f, 0.12f), new Vector2(0.5f, 0f));
                    Rect(new Vector2(p.x - w * 0.5f, cy), new Vector2(w * b.CastFrac, ch), new Color(0.45f, 0.75f, 1f), new Vector2(0f, 0f));
                }
            }
        }

        void DrawPopups()
        {
            foreach (var p in _popups)
            {
                // The switch hides DAMAGE. Status text (Miss / Dodged / Guarded) and heals carry
                // information rather than damage, so they stay — the same split Godot's
                // `p.has("value")` makes.
                if (p.IsDamage && !ShowDamageNumbers) continue;
                var world = AnchorOf != null ? AnchorOf(p.Id, p.World) : p.World;
                if (!Project(world, out var sp)) continue;
                float k = Mathf.Clamp01(p.Age / PopupLifetime);
                sp.y += PopupRisePx * k;                    // Godot subtracts; its y points down
                var col = p.Color; col.a = 1f - k;
                float scale = p.Age < PopupPopDur ? Mathf.Lerp(PopupPopScale, 1f, p.Age / PopupPopDur) : 1f;
                if (p.IsDamage) NumberLabel(sp, p.Text, PopupFontSize, col, scale);
                else Label(sp, p.Text, PopupFontSize, col, scale, PopupOutlinePx);
            }
        }

        void DrawBanners()
        {
            foreach (var b in _banners)
            {
                var world = AnchorOf != null ? AnchorOf(b.Id, b.World) : b.World;
                if (!Project(world, out var sp)) continue;
                if (b.X == null) LayOutBanner(b);
                float size = BannerFontSize * b.Size;
                // The whole title drifts gently right; it never reverses direction while fading.
                float whiten = Mathf.SmoothStep(0f, 1f, Mathf.InverseLerp(BannerWhitenStart, BannerWhitenEnd, b.Age));
                float soften = Mathf.SmoothStep(0f, 1f, Mathf.InverseLerp(BannerSoftenStart, BannerSoftenEnd, b.Age));
                float fade = Mathf.InverseLerp(BannerFadeStart, BannerLifetime, b.Age);
                float drift = Mathf.Clamp01((b.Age - BannerDriftStart) / (BannerLifetime - BannerDriftStart));
                sp += new Vector2(BannerDriftPx * Mathf.SmoothStep(0f, 1f, drift), 18f) * b.Size;
                float alpha = 1f - Mathf.SmoothStep(0f, 1f, fade);

                // Crossfade aligned crisp/soft glyphs using vertex state, not shared-material
                // mutations: simultaneous BREAKs can be at different points in their exit.
                float crispAlpha = (1f - soften) * alpha;
                float softAlpha = Mathf.Lerp(BannerGlowAlpha, .85f, soften) * alpha;
                for (int i = 0; i < b.Text.Length; i++)
                {
                    float la = b.Age - i * BannerLetterStagger;
                    if (la < 0f) break;              // this letter and every one after it is still to come
                    float e = Mathf.Clamp01(la / BannerPopDur);
                    float ease = 1f - (1f - e) * (1f - e);            // out-quad: fast in, soft landing
                    float scale = Mathf.Lerp(BannerPopScale, 1f, ease);
                    var at = sp + new Vector2(b.X[i], -BannerLetterRisePx * b.Size * (1f - ease));
                    // All glyphs keep their spacing and share the same one-way drift through the exit.
                    float initial = b.Serif && i == 0 && b.Text[0] == 'B' ? BannerInitialScale : 1f;
                    at.y -= size * 0.30f;
                    string ch = b.Text[i].ToString();
                    BreakGlyph(at, ch, size * initial, scale * BannerGlowScale,
                        softAlpha, 0f, b.Serif, true, whiten);
                    BreakGlyph(at, ch, size * initial, scale, crispAlpha, 0f, b.Serif, false, whiten);
                }

                // The break's own damage, ABOVE the word that names it — and only once the word is
                // finished, so the two land in the order they are read. Drawn HERE rather than
                // pushed through the popup pool so they share one lifetime and one fade; a popup
                // has its own shorter clock and the pair visibly comes apart.
                if (b.Amount > 0 && b.Age >= b.Text.Length * BannerLetterStagger)
                {
                    float ae = Mathf.Clamp01((b.Age - b.Text.Length * BannerLetterStagger) / BannerPopDur);
                    float aease = 1f - (1f - ae) * (1f - ae);
                    var amt = ColBreak; amt.a = alpha;
                    NumberLabel(sp + new Vector2(0f, BannerAmountRisePx * b.Size), b.Amount.ToString(),
                          (int)(BannerAmountFontSize * b.Size), amt,
                          Mathf.Lerp(BannerPopScale, 1f, aease));
                }
            }
        }

        void LayOutBanner(Banner b)
        {
            int n = b.Text.Length;
            b.X = new float[n];
            b.Serif = _breakFont != null && _breakFont.HasCharacters(b.Text);
            TMP_FontAsset font = b.Serif ? _breakFont : _font;
            float px = BannerFontSize * b.Size;
            float total = Measure(BannerPrefix(b, n), px, font).x;
            float run = 0f;
            for (int i = 0; i < n; i++)
            {
                float upto = Measure(BannerPrefix(b, i + 1), px, font).x;
                b.X[i] = run + (upto - run) * 0.5f - total * 0.5f;
                run = upto;
            }
        }

        static string BannerPrefix(Banner b, int count)
        {
            string prefix = b.Text.Substring(0, count);
            return b.Serif && count > 0 && b.Text[0] == 'B'
                ? "<size=114%>B</size>" + prefix.Substring(1) : prefix;
        }

        void DrawMultiHits()
        {
            if (!ShowDamageNumbers) return;
            foreach (var m in _multi)
            {
                var world = AnchorOf != null ? AnchorOf(m.Id, m.World) : m.World;
                if (!Project(world, out var sp)) continue;
                foreach (var h in m.Hits)
                {
                    if (!h.landed) continue;     // still waiting its turn (staggered)
                    float localAge = h.age - h.delay;
                    float k = Mathf.Clamp01(localAge / MhLifetime);
                    float punch = 1f + MhPunch * Mathf.Max(0f, 1f - localAge / MhPunchDur);
                    var c = Color.white; c.a = 1f - k;
                    NumberLabel(sp + new Vector2(0f, MhRisePx * k), h.value.ToString(), PopupFontSize, c, punch);
                }
                // The yellow total: for an incremental feeder it waits for a 2nd number (one bolt
                // is just its own number, not a "total"); a known multi-hit shows it from the 1st.
                bool showTotal = (m.Expected == 0 && m.Landed >= 2) || (m.Expected >= 2 && m.Landed >= 1);
                if (m.AccumAge < MhLifetime && showTotal)
                {
                    float k2 = Mathf.Clamp01(m.AccumAge / MhLifetime);
                    var c2 = ColYellow; c2.a = 1f - k2;
                    float fs = PopupFontSize * (1.4f + 0.35f * m.Pop);
                    var at = sp + new Vector2(0f, MhRisePx * k2 + 1.7f * PopupFontSize);
                    NumberLabel(at, m.Total.ToString(), Mathf.RoundToInt(fs), c2, 1f);
                }
            }
        }

        void DrawSkillLabels()
        {
            foreach (var l in _skillLabels)
            {
                if (TryActorGround == null || !TryActorGround(l.Id, out var world)) continue;
                // Measured from the entity's GROUND position and lifted along the CAMERA's up
                // axis — a full billboard leans with it, so world-up drifts off at the edges.
                world += _cam.transform.up * (SkillLabelYCells + SkillLabelLift);
                if (!Project(world, out var sp)) continue;
                float a = l.Age > SkillLabelDur - SkillLabelFade
                    ? Mathf.Clamp01((SkillLabelDur - l.Age) / SkillLabelFade) : 1f;
                float sc = l.Age < SkillLabelPop ? Mathf.Lerp(0.55f, 1f, l.Age / SkillLabelPop) : 1f;
                var size = Measure(l.Text, SkillLabelSize);
                var pad = new Vector2(6f, 3f);
                var box = new Vector2(size.x + pad.x * 2f, SkillLabelSize + pad.y * 2f) * sc;
                Rect(sp, box, new Color(0.09f, 0.09f, 0.11f, 0.75f * a), new Vector2(0.5f, 0f));
                Label(sp + new Vector2(0f, box.y * 0.5f), l.Text, SkillLabelSize,
                      new Color(1f, 1f, 1f, a), sc, 0);
            }
        }

        /// Below the feet, as a SCREEN offset. Dropping in WORLD along the camera's up axis and
        /// then adding the ascent stacks two drops and pushes the name well under the sprite —
        /// the 2D path only ever offsets on screen, because 2D IS screen space.
        void DrawHoverName()
        {
            if (string.IsNullOrEmpty(_hoverName)) return;
            if (!Project(_hoverWorld, out var sp)) return;
            var at = sp - new Vector2(0f, HoverNameDrop + HoverNameSize);
            // No box — white text with a 1px dark shadow, so it reads over any tile.
            Label(at - new Vector2(-1f, 1f), _hoverName, HoverNameSize, new Color(0.05f, 0.05f, 0.05f, 0.9f), 1f, 0);
            Label(at, _hoverName, HoverNameSize, Color.white, 1f, 0);
        }

        void DrawLoot()
        {
            // The CANVAS's size, not Screen's: in the Editor the game view can render at a
            // different resolution than the window reports, and the banner then centres on the
            // wrong middle.
            var canvasSize = _back != null ? _back.rect.size : new Vector2(Screen.width, Screen.height);
            float w = canvasSize.x, hgt = canvasSize.y;
            for (int i = 0; i < _loot.Count; i++)
            {
                var l = _loot[i];
                float a = l.Age > LootLife - LootFade ? Mathf.Clamp01((LootLife - l.Age) / LootFade) : 1f;
                var size = Measure(l.Text, LootFont);
                float inner = LootIconW + size.x;
                // Godot lays this out from the TOP of the screen with y growing down.
                float top = LootTop + i * LootLineH;
                float centreY = hgt - top - (LootFont * 0.5f + LootPillPad.y);
                var pill = new Vector2(inner + LootPillPad.x * 2f, LootFont + LootPillPad.y * 2f);
                var edge = LootPillEdge; edge.a *= a;
                var body = LootPill; body.a *= a;
                Rect(new Vector2(w * 0.5f, centreY), pill + new Vector2(2f, 2f), edge, new Vector2(0.5f, 0.5f));
                Rect(new Vector2(w * 0.5f, centreY), pill, body, new Vector2(0.5f, 0.5f));
                var col = l.Color; col.a = a;
                // The icon gutter is reserved and empty: no item icon art yet, and holding the
                // space now means adding it is one draw rather than a re-layout.
                float textLeft = w * 0.5f - pill.x * 0.5f + LootPillPad.x + LootIconW;
                LabelLeft(new Vector2(textLeft, centreY), l.Text, LootFont, col);
            }
        }

        // --- element pools --------------------------------------------------------------------

        void Rect(Vector2 pos, Vector2 size, Color col, Vector2 pivot)
        {
            var img = TakeImage();
            var rt = img.rectTransform;
            rt.pivot = pivot;
            rt.sizeDelta = size;
            rt.anchoredPosition = pos;
            rt.localScale = Vector3.one;
            img.color = col;
        }

        void Label(Vector2 pos, string text, int size, Color col, float scale, int outline)
            => Emit(pos, text, size, col, scale, outline, TextAnchor.MiddleCenter, new Vector2(0.5f, 0.5f));

        void LabelLeft(Vector2 pos, string text, int size, Color col)
            => Emit(pos, text, size, col, 1f, 0, TextAnchor.MiddleLeft, new Vector2(0f, 0.5f));

        void Emit(Vector2 pos, string text, int size, Color col, float scale, int outline,
                  TextAnchor anchor, Vector2 pivot)
        {
            var t = TakeText();
            var rt = t.rectTransform;
            // The pop punch is a FONT SIZE, not a transform scale: scaling the transform stretches
            // whatever the glyph was rendered at, and with SDF the honest size costs nothing.
            float px = Mathf.Max(1f, size * scale);
            // A pool entry may have drawn a BREAK glyph last frame. Restore all display-only
            // overrides so ordinary damage, skill labels and localized text retain their look.
            t.font = _font;
            t.enableVertexGradient = false;
            t.fontStyle = FontStyles.Normal;
            rt.localRotation = Quaternion.identity;
            t.text = text;
            t.fontSize = px;
            t.color = col;
            t.alignment = anchor == TextAnchor.MiddleLeft ? TextAlignmentOptions.Left : TextAlignmentOptions.Center;
            t.fontSharedMaterial = outline > 0 ? _matOutlined : _matPlain;
            rt.pivot = pivot;
            rt.sizeDelta = new Vector2(Measure(text, px).x + px * 2f, px * 2.2f);
            // NO ROUNDING. The rise is sub-pixel and SDF stays sharp through it.
            rt.anchoredPosition = pos;
            rt.localScale = Vector3.one;
        }

        void NumberLabel(Vector2 pos, string text, int size, Color semanticColor, float scale)
        {
            var t = TakeText();
            var rt = t.rectTransform;
            var font = _breakFont != null ? _breakFont : _font;
            float px = Mathf.Max(1f, size * scale * NumberOpticalScale);
            t.font = font;
            t.fontStyle = FontStyles.Normal; // the bundled numeric face is already italic
            t.fontSharedMaterial = _matNumbers;
            t.text = text;
            t.fontSize = px;
            t.color = new Color(1f, 1f, 1f, semanticColor.a);
            semanticColor.a = 1f; // lifetime alpha belongs to the vertex color, applied once
            var top = Color.Lerp(semanticColor, Color.white, .24f);
            t.enableVertexGradient = true;
            t.colorGradient = new VertexGradient(top, top, semanticColor, semanticColor);
            t.alignment = TextAlignmentOptions.Center;
            rt.pivot = new Vector2(.5f, .5f);
            rt.sizeDelta = new Vector2(Measure(text, px, font).x + px * 2f, px * 2.2f);
            rt.anchoredPosition = pos;
            rt.localScale = Vector3.one;
            rt.localRotation = Quaternion.identity;
        }

        void BreakGlyph(Vector2 pos, string text, float size, float scale, float alpha,
                        float rotation, bool serif, bool glow, float whiten)
        {
            var t = TakeText();
            var rt = t.rectTransform;
            var font = serif ? _breakFont : _font;
            float px = Mathf.Max(1f, size * scale);
            t.font = font;
            t.fontStyle = FontStyles.Normal; // the supplied face is already italic
            t.fontSharedMaterial = serif ? (glow ? _matBreakGlow : _matBreak)
                                         : (glow ? _matBreakFallbackGlow : _matBreakFallback);
            t.text = text;
            t.fontSize = px;
            var tint = glow ? Color.Lerp(ColBreakGlow, Color.white, whiten) : Color.white;
            tint.a = alpha;
            t.color = tint;
            t.enableVertexGradient = !glow;
            var face = Color.Lerp(ColBreakWord, Color.white, whiten);
            var lower = Color.Lerp(ColBreakWordLower, Color.white, whiten);
            t.colorGradient = new VertexGradient(face, face, lower, lower);
            t.alignment = TextAlignmentOptions.Baseline;
            rt.pivot = new Vector2(0.5f, 0.5f);
            rt.sizeDelta = new Vector2(Measure(text, px, font).x + px * 2f, px * 2.2f);
            rt.anchoredPosition = pos;
            rt.localScale = Vector3.one;
            rt.localRotation = Quaternion.Euler(0f, 0f, rotation);
        }

        /// One hidden text object does every measurement, so nothing has to be laid out twice or
        /// emitted before the box that sits behind it.
        Vector2 Measure(string text, float size, TMP_FontAsset font = null)
        {
            if (string.IsNullOrEmpty(text)) return Vector2.zero;
            if (_measure == null)
            {
                var go = new GameObject("measure", typeof(RectTransform));
                go.transform.SetParent(_front, false);
                _measure = go.AddComponent<TextMeshProUGUI>();
                _measure.font = _font;
                _measure.textWrappingMode = TextWrappingModes.NoWrap;
                _measure.raycastTarget = false;
                go.SetActive(false);
            }
            _measure.font = font != null ? font : _font;
            _measure.fontSize = size;
            _measure.text = text;
            return new Vector2(_measure.preferredWidth, size);
        }

        TextMeshProUGUI TakeText()
        {
            if (_textCursor < _texts.Count)
            {
                var t0 = _texts[_textCursor++];
                t0.gameObject.SetActive(true);
                return t0;
            }
            var go = new GameObject("t", typeof(RectTransform));
            go.transform.SetParent(_front, false);
            var t = go.AddComponent<TextMeshProUGUI>();
            t.font = _font;
            t.raycastTarget = false;
            t.textWrappingMode = TextWrappingModes.NoWrap;
            t.overflowMode = TextOverflowModes.Overflow;
            var rt = t.rectTransform;
            rt.anchorMin = rt.anchorMax = Vector2.zero;   // anchored bottom-left = screen pixels
            _texts.Add(t);
            _textCursor++;
            return t;
        }

        Image TakeImage()
        {
            if (_imageCursor < _images.Count)
            {
                var i0 = _images[_imageCursor++];
                i0.gameObject.SetActive(true);
                return i0;
            }
            var go = new GameObject("r", typeof(RectTransform));
            go.transform.SetParent(_back, false);
            var img = go.AddComponent<Image>();
            img.raycastTarget = false;
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = Vector2.zero;
            _images.Add(img);
            _imageCursor++;
            return img;
        }
    }
}
