// Eclipse typography: Marcellus for names/titles, Bai Jamjuree for readable Latin/Thai UI.
// International fallback and arena damage typography remain separate.
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public static class UiTheme
    {
        // --- type ---------------------------------------------------------------------------
        // WHOLE NUMBERS, all of them. A fractional point size puts glyph advances on fractional
        // pixels, so the same letter is sampled differently in two labels and the text reads as
        // slightly melted. It costs nothing to round, and it is a visible difference at 11 pt.
        public const float TitleSize = 21f;   // a window's name
        public const float HeadSize = 16f;    // a section head, a button
        public const float BodySize = 15f;    // rows, item names
        public const float CaptionSize = 13f; // counts, footnotes, the quiet half of a row

        static TMP_FontAsset _font;
        /// The UI face. Loaded once; every screen asks here rather than carrying its own path.
        /// The UI face is a SEPARATE asset from the arena's, and separate from a general-purpose
        /// Noto asset, because it is built for the sizes this UI actually draws: sampled at 48 pt
        /// rather than 90, and rendered SDFAA_HINTED so each outline is grid-fitted before the
        /// distance field is built. Hinting is the single biggest lever for text under ~16 pt.
        public static TMP_FontAsset Font
            => _font != null ? _font : (_font = Resources.Load<TMP_FontAsset>("Fonts/EclipseUI SDF") ?? Resources.Load<TMP_FontAsset>("Fonts/NotoSans UI SDF"));

        static Material _mat;
        /// ONE shared material for every label, with the SDF SHARPENED.
        ///
        /// A signed-distance glyph is reconstructed from a gradient, and TMP's default filtering is
        /// tuned for text at 24 pt and up. At the 11–13 pt a list actually uses, the same settings
        /// eat the thin parts of a letterform — the stem of an "l", the join of an "a" — and the
        /// result reads as text that is coming apart rather than as text that is small. Sharpness
        /// pulls the alpha ramp tighter around the outline and puts those strokes back.
        ///
        /// Shared rather than per-label for a second reason: touching `material` on a TMP_Text
        /// instantiates a copy, so a screen of forty labels would otherwise be forty materials and
        /// forty draw calls.
        public static Material Material
        {
            get
            {
                if (_mat != null) return _mat;
                var f = Font;
                if (f == null) return null;
                _mat = new Material(f.material)
                { name = "UI text (sharpened)", hideFlags = HideFlags.HideAndDontSave };
                _mat.SetFloat(ShaderUtilities.ID_Sharpness, 0.15f);
                // A HAIR of dilate. Light text on a near-black ground loses weight to the way the
                // eye and the gamma curve treat a thin bright stroke; two hundredths puts it back
                // without the letterforms going soft at the joins.
                _mat.SetFloat(ShaderUtilities.ID_FaceDilate, 0.02f);
                // The perspective filter softens glyphs that are viewed at an angle. Nothing here
                // is: this is a screen-space overlay, so it is pure blur.
                if (_mat.HasProperty(ShaderUtilities.ID_PerspectiveFilter))
                    _mat.SetFloat(ShaderUtilities.ID_PerspectiveFilter, 0f);
                return _mat;
            }
        }

        // --- palette ------------------------------------------------------------------------
        /// The window ground. Near-black with a blue cast — a neutral grey reads as unfinished.
        public static readonly Color Panel = new Color(.045f,.049f,.058f,1f);
        /// A raised surface: a title bar, a filled slot, a button at rest.
        public static readonly Color Surface = new Color(.095f,.102f,.115f,1f);
        public static readonly Color SurfaceHover = new Color(.16f,.17f,.185f,1f);
        /// SELECTED — an open window's button, the chosen member tab, the picked equipment slot.
        ///
        /// Deliberately not the accent: a dozen lit buttons would spend the one warm colour
        /// everywhere and it would stop meaning anything. But it was also far too LIGHT — nearly
        /// twice `Surface`, with a blue cast, on a near-black UI. The selected member tab is
        /// visible on every screen, so that one pale pill was the brightest thing in the interface
        /// and the first thing the eye landed on, which is not what "selected" should cost.
        ///
        /// Selection is carried by the ACCENT TICK and the stronger border instead. The fill only
        /// has to separate this cell from its neighbours, and one step is enough to do that.
        public static readonly Color SurfaceActive = new Color(.205f,.215f,.23f,1f);
        // One material palette for every mesh-backed menu, dialog, card and action.
        public static readonly Color FrameTop = new Color(.115f,.125f,.14f,.97f);
        public static readonly Color FrameBottom = new Color(.065f,.072f,.085f,.97f);
        public static readonly Color FramePrimary = new Color(.24f,.23f,.19f,1f);
        public static readonly Color FrameSelected = new Color(.22f,.225f,.235f,1f);
        public static readonly Color FrameEdge = new Color(.78f,.79f,.80f,.22f);
        public static readonly Color FrameRaisedEdge = new Color(.83f,.84f,.85f,.45f);
        public static Color WithAlpha(Color color,float alpha) { color.a=alpha; return color; }
        /// Borders sit at low alpha. A bright hairline is the cheapest-looking thing in a UI.
        public static readonly Color Line = new Color(0.74f, 0.78f, 0.79f, 0.12f);
        public static readonly Color LineStrong = new Color(0.74f, 0.78f, 0.79f, 0.32f);
        /// The one warm accent, for the thing that matters on a screen — and nothing else.
        public static readonly Color Accent = new Color(.88f,.82f,.64f);
        public static readonly Color TextHigh = new Color(.95f,.95f,.93f);
        public static readonly Color TextMid = new Color(.79f,.80f,.81f);
        // Muted still means readable: captions and input hints carry information. Keep this
        // above 4.5:1 on Panel, Surface, SurfaceHover and SurfaceActive in the sRGB palette.
        public static readonly Color TextLow = new Color(.68f,.70f,.73f);
        /// Behind a modal. Deeper than a dim: it has to read as "the screen is not live".
        public static readonly Color Scrim = new Color(.015f,.035f,.035f,.78f);

        public static readonly Color Header = new Color(.06f,.067f,.08f,.96f);
        public static readonly Color BackdropTop = new Color(.075f,.085f,.105f,1f);
        public static readonly Color BackdropBottom = new Color(.17f,.19f,.22f,1f);

        // --- spacing ------------------------------------------------------------------------
        public const float PadLarge = 24f, Pad = 16f, PadSmall = 12f, Gap = 8f;
        public const float TitleBarH = 54f, ButtonH = 40f;
        public const float HoverDuration = 0.09f;
        public static readonly Color OnAccent = new Color(0.075f, 0.086f, 0.110f);
        public static readonly Color DisabledCaption = new Color(.80f,.82f,.85f,1);
        public static readonly Color Warning = new Color(.95f,.61f,.49f,1);
        public const float PageDuration=.16f, PageStartAlpha=.72f;
        // Shared interaction colours: selected state is a gold edge, never a pale blue slab.
        public static ColorBlock ButtonColors => new ColorBlock {
            normalColor=Color.white, highlightedColor=new Color(1.15f,1.15f,1.12f),
            selectedColor=Color.white, pressedColor=new Color(.80f,.83f,.85f),
            disabledColor=new Color(.52f,.55f,.58f),colorMultiplier=1,fadeDuration=HoverDuration
        };
        public static ColorBlock GhostColors => new ColorBlock {
            normalColor=WithAlpha(Surface,.55f),highlightedColor=WithAlpha(SurfaceHover,.72f),
            selectedColor=WithAlpha(SurfaceActive,.75f),pressedColor=WithAlpha(Panel,.80f),
            disabledColor=WithAlpha(Surface,.35f),colorMultiplier=1,fadeDuration=HoverDuration
        };

        // --- shape --------------------------------------------------------------------------
        static Sprite _fill8, _fill4, _frame8, _frame4, _frame2;
        static Sprite _noticePaper, _noticePill;
        public static Sprite NoticePaper => _noticePaper ? _noticePaper : (_noticePaper=Rounded(16,0));
        public static Sprite NoticePill => _noticePill ? _noticePill : (_noticePill=Rounded(40,0));

        /// A filled rounded rect, 9-sliced. `Image.type = Sliced` stretches only the middle, so
        /// one small texture serves a 12 px chip and a 1600 px window at the same corner radius.
        public static Sprite Fill(int radius = 8)
            => radius >= 8 ? (_fill8 ??= Rounded(8, 0f)) : (_fill4 ??= Rounded(4, 0f));

        /// The same rounded rect as an OUTLINE — a hairline frame, drawn over a fill.
        public static Sprite Frame(int radius = 8, float thickness = 1f)
        {
            if (thickness >= 2f) return _frame2 ??= Rounded(8, 2f);
            return radius >= 8 ? (_frame8 ??= Rounded(8, 1f)) : (_frame4 ??= Rounded(4, 1f));
        }

        /// Builds the 9-slice. `thickness` 0 = solid; > 0 = a ring that many pixels wide.
        ///
        /// Coverage comes from the signed distance to the rounded rect rather than from an
        /// inside/outside test — an aliased corner is exactly the tell that makes a UI look cheap,
        /// and one clamp is the whole cost of avoiding it.
        static Sprite Rounded(int radius, float thickness)
        {
            int n = radius * 2 + 4;                       // 2 px of straight edge in the middle
            // HideAndDontSave, and this is a real defect it fixes rather than a precaution.
            // A texture created at runtime is DESTROYED on a scene load. The sprite caching it is
            // a static, so it survives — leaving a Sprite pointing at a dead texture, which uGUI
            // draws as an untextured white rect. The whole UI came back mid-grey with square
            // corners and a solid gold block where a selected cell's hairline should have been:
            // every border, being a 9-slice ring, had become a filled rectangle.
            var tex = new Texture2D(n, n, TextureFormat.RGBA32, false, true)
            {
                filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Clamp,
                hideFlags = HideFlags.HideAndDontSave,
            };
            var px = new Color32[n * n];
            float half = n * 0.5f, inner = half - radius;  // centre of the corner arc
            for (int y = 0; y < n; y++)
                for (int x = 0; x < n; x++)
                {
                    // Signed distance to a rounded rectangle.
                    var q = new Vector2(Mathf.Abs(x + .5f - half) - inner, Mathf.Abs(y + .5f - half) - inner);
                    float d = new Vector2(Mathf.Max(q.x, 0), Mathf.Max(q.y, 0)).magnitude + Mathf.Min(Mathf.Max(q.x, q.y), 0) - radius;
                    float a = Mathf.Clamp01(0.5f - d);                        // outer edge
                    if (thickness > 0f) a *= Mathf.Clamp01(d + thickness + 0.5f); // hollow it
                    px[y * n + x] = new Color32(255, 255, 255, (byte)(a * 255f));
                }
            tex.SetPixels32(px);
            tex.Apply(false, true);
            float b = radius + 1f;                        // slice just outside the arc
            var sprite = Sprite.Create(tex, new Rect(0, 0, n, n), new Vector2(0.5f, 0.5f), 100f, 0,
                                       SpriteMeshType.FullRect, new Vector4(b, b, b, b));
            sprite.hideFlags = HideFlags.HideAndDontSave;
            return sprite;
        }

        static TMP_FontAsset _displayFont;
        public static TMP_FontAsset DisplayFont => _displayFont != null ? _displayFont :
            (_displayFont=Resources.Load<TMP_FontAsset>("Fonts/EclipseDisplay SDF") ?? Resources.Load<TMP_FontAsset>("HomeUI/HomeSerif") ?? Font);
        public static void Display(TextMeshProUGUI label)
        {
            _displayFont = DisplayFont;
            if(_displayFont==null)return;
            label.font=_displayFont;label.fontSharedMaterial=_displayFont.material;
            label.characterSpacing=.2f;
        }
        static Sprite _cutFill8,_cutFill4,_cutFrame8,_cutFrame4,_cutFrame2;
        public static Sprite CutFill(int radius=8) => radius>=8?(_cutFill8??=Chamfer(8,0)):(_cutFill4??=Chamfer(4,0));
        public static Sprite CutFrame(int radius=8,float thickness=1) => thickness>=2?(_cutFrame2??=Chamfer(8,2)):(radius>=8?(_cutFrame8??=Chamfer(8,1)):(_cutFrame4??=Chamfer(4,1)));
        static Sprite Chamfer(int cut,float thickness)
        {
            int n=cut*2+4;float half=n*.5f;
            var texture=new Texture2D(n,n,TextureFormat.RGBA32,false,true){filterMode=FilterMode.Bilinear,wrapMode=TextureWrapMode.Clamp,hideFlags=HideFlags.HideAndDontSave};
            var pixels=new Color32[n*n];
            for(int y=0;y<n;y++)for(int x=0;x<n;x++) {
                float dx=Mathf.Abs(x+.5f-half),dy=Mathf.Abs(y+.5f-half);
                float distance=Mathf.Max(Mathf.Max(dx-half,dy-half),(dx+dy-(2*half-cut))*.7071068f);
                float coverage=Mathf.Clamp01(.5f-distance);
                if(thickness>0)coverage*=Mathf.Clamp01(distance+thickness+.5f);
                pixels[y*n+x]=new Color32(255,255,255,(byte)(coverage*255));
            }
            texture.SetPixels32(pixels);texture.Apply(false,true);
            float border=cut+1;
            var sprite=Sprite.Create(texture,new Rect(0,0,n,n),new Vector2(.5f,.5f),100,0,SpriteMeshType.FullRect,new Vector4(border,border,border,border));
            sprite.hideFlags=HideFlags.HideAndDontSave;return sprite;
        }

        // --- widgets ------------------------------------------------------------------------

        /// A rounded surface. Every panel, button and cell in the UI comes from here, so the
        /// corner radius is a single number rather than thirty.
        public static Image Box(Transform parent, string name, Color col, int radius = 8)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var img = go.AddComponent<Image>();
            img.sprite = Fill(radius);
            img.type = Image.Type.Sliced;
            img.color = col;
            img.raycastTarget = true;   // a panel eats clicks meant for what is under it
            return img;
        }

        static Sprite _fade, _combatFade;
        /// A VERTICAL FADE, transparent at the top and dark at the foot.
        ///
        /// It replaces the solid strip the chrome used to sit on. Two opaque bands stacked at the
        /// bottom of the screen — the party bar and the action bar — read as a wall (the user's
        /// word was อึดอัด, cramped), and on a phone that is a third of the display spent on a
        /// colour. A fade keeps the controls legible against bright grass without taking the world
        /// away: the game runs all the way to the bottom edge and the chrome floats on it.
        ///
        /// The ramp is eased rather than linear. A straight ramp has a visible top edge where it
        /// leaves zero, which is the hard band again, only fainter.
        public static Sprite Fade(bool combat = false)
        {
            var cached = combat ? _combatFade : _fade;
            if (cached != null) return cached;
            const int n = 64;
            var tex = new Texture2D(1, n, TextureFormat.RGBA32, false, true)
            {
                filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Clamp,
                hideFlags = HideFlags.HideAndDontSave,   // survives a scene load — see Rounded()
            };
            var px = new Color32[n];
            for (int y = 0; y < n; y++)
            {
                float t = 1f - y / (float)(n - 1);          // 0 at the top row, 1 at the bottom
                float a = combat ? Mathf.SmoothStep(0f, .94f, t)
                                 : Mathf.Pow(t, 1.6f) * .62f;
                px[y] = new Color32(4, 4, 8, (byte)(a * 255f));
            }
            tex.SetPixels32(px);
            tex.Apply(false, true);
            var sprite = Sprite.Create(tex, new Rect(0, 0, 1, n), new Vector2(0.5f, 0.5f), 100f);
            sprite.hideFlags = HideFlags.HideAndDontSave;
            if (combat) _combatFade = sprite; else _fade = sprite;
            return sprite;
        }

        /// A plain rectangle — for the pieces a radius would only distort: a 1 px rule, a full-
        /// bleed strip welded to the screen edge, a 3 px accent tick. A 9-slice narrower than its
        /// own corners does not render as a smaller rounded box; it renders as a squashed one.
        public static Image Flat(Transform parent, string name, Color col)
        {
            var img = Box(parent, name, col);
            img.sprite = null;
            img.type = Image.Type.Simple;
            return img;
        }

        /// A hairline over a surface. Its own object so the border does not tint with the fill —
        /// a border that dims when the button is pressed looks like a rendering bug.
        public static Image Border(RectTransform on, Color col, int radius = 8, float thickness = 1f)
        {
            var go = new GameObject("Border", typeof(RectTransform));
            go.transform.SetParent(on, false);
            var img = go.AddComponent<Image>();
            img.sprite = Frame(radius, thickness);
            img.type = Image.Type.Sliced;
            img.color = col;
            img.raycastTarget = false;
            var rt = img.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.zero; rt.offsetMax = Vector2.zero;
            return img;
        }

        /// A text element on the theme. Every label in the UI goes through here, so changing the
        /// face or the colour ramp is one edit rather than thirty.
        public static TextMeshProUGUI Label(Transform parent, string name, string text, float size,
                                            Color col, TextAlignmentOptions align)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var t = go.AddComponent<TextMeshProUGUI>();
            t.font = Font;
            var mat = Material;
            if (mat != null) t.fontSharedMaterial = mat;
            t.text = text;
            t.fontSize = size;
            t.color = col;
            t.alignment = align;
            t.raycastTarget = false;
            t.textWrappingMode = TextWrappingModes.NoWrap;
            t.overflowMode = TextOverflowModes.Ellipsis;
            // Keep compact labels and Thai combining marks naturally spaced.
            t.characterSpacing = 0;
            return t;
        }

        /// An all-caps label — a section head, a window name. Wide tracking is what makes caps
        /// read as a heading instead of as shouting.
        public static TextMeshProUGUI Caps(Transform parent, string name, string text, float size,
                                           Color col, TextAlignmentOptions align)
        {
            var t = Label(parent, name, text.ToUpperInvariant(), size, col, align);
            t.characterSpacing = 1f;
            return t;
        }
    }
}
