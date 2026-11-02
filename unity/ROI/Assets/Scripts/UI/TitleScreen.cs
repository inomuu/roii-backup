// THE TITLE SCREEN — the first thing anyone sees, and the shape a gacha player expects: a logo on
// an empty field, one line that says the game is waiting, and a sign-in that arrives only once
// they have touched it.
//
// It is PRESENTATION ONLY. It owns no session, no socket and no seed; it raises `PlayPressed`,
// `DevPressed` and `CreatePressed` and is told what to show. `SessionBoot` owns the sequence —
// the same split as the rest of the UI (CLAUDE.md rule 2), and the reason this file can be
// re-skinned without touching a line of the login.
//
// The landscape landing screen opens the existing account form only through Start Game or
// Account. Utility dialogs are modal and return keyboard focus to Start Game when closed.
//
// THREE DOORS, matching the Godot client's mode picker one for one:
//   PLAY                    — the server. Through Steam where the machine has it, through the dev
//                             door by name otherwise.
//   "sign in without Steam" — the same server, the other door. It appears only on a machine that
//                             HAS Steam, because only there is there a choice to make.
//   "local session (dev)"   — the sim on a client seed with the cheats on. A DEV path, not a
//                             shipping mode (CLAUDE.md: there is no offline version) — it is
//                             labelled as one, sits at ghost weight below the real button, and its
//                             session can never push a save.
//
// The middle one is not a convenience. Before it existed, installing Steam silently removed every
// other way in: `OnlineService` chose the door itself, so a machine with Steam could only ever try
// the Steam door, and a local server with no publisher key locked the whole client out.
using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;

namespace Roi.UI
{
    public sealed partial class TitleScreen : MonoBehaviour
    {
        /// One creatable class as the sim reports it (`classes`), which is also the list the
        /// server builds its genesis candidates from — so a class offered here can always anchor.
        public struct ClassInfo
        {
            public string Id;
        }

        public event Action<string, bool> PlayPressed;        // account name, use Steam
        public event Action DevPressed;                       // local dev session
        public event Action<string, string> CreatePressed;    // class id, party name

        // A canvas of its own, above everything: the bar is 40 and the system menu 50, and while
        // this is up neither is reachable.
        const int SortOrder = 100;

        const float PanelW = 600f, InputH = 88f, ButtonH = 88f;
        const float CreateW = 560f, CreateH = 344f;

        static readonly string[] PartyNames =
        {
            "Aldric", "Luna", "Kael", "Mira", "Soren", "Freya", "Garin", "Nyx",
            "Roland", "Sasha", "Elwin", "Thora", "Cyrus", "Vesna", "Doran", "Iris",
        };

        RectTransform _root, _signIn, _creation;
        readonly List<GameObject> _logoParts = new List<GameObject>();
        Button _tapCatcher;
        TextMeshProUGUI _tapLabel, _status, _version;
        TMP_InputField _nameField, _partyField;
        Button _play, _dev, _begin, _routeSwap, _roll;
        readonly List<Selectable> _signInOrder = new List<Selectable>();
        readonly List<Selectable> _creationOrder = new List<Selectable>();
        string _initialStatus;
        /// Which online door PLAY opens. Steam when the machine has it, because that is the login
        /// for a game that ships on Steam — but it is a DEFAULT, not a lock: `_routeSwap` moves it,
        /// and the whole point of this field existing is that the choice is the player's.
        bool _useSteam;

        /// WHETHER THE TWO DEV DOORS EXIST AT ALL. They are compiled out of a release build rather
        /// than hidden in it: the shipped game signs in through Steam and nothing else (user,
        /// 2026-09-08), and `/auth/dev` reaching a production server is the account-takeover shape
        /// this repo already has a scar from — a name is not a credential, so anyone who can name
        /// somebody else's account can BE them. A runtime flag would still ship the code that opens
        /// that door; this does not.
        static bool DevDoors =>
#if UNITY_EDITOR || DEVELOPMENT_BUILD
            true;
#else
            false;
#endif
        WuwaLoadingBar _busyTrack;

        bool _busy;

        public static TitleScreen Create(Transform parent, string accountName, string endpoint)
        {
            var go = new GameObject("TitleScreen");
            go.transform.SetParent(parent, false);
            var t = go.AddComponent<TitleScreen>();
            t.Build(accountName, endpoint);
            return t;
        }

        // --- state the boot drives ---------------------------------------------------------

        public void SetStatus(string text)
        {
            _initialStatus = text ?? "";
            if (_status != null) _status.text = _initialStatus;
        }

        /// Working. Every control is refused rather than hidden — a form that vanishes mid-login
        /// reads as a crash, and a second press on PLAY would open a second session.
        public void SetBusy(bool busy, string text = null)
        {
            _busy = busy;
            if (_play != null) _play.interactable = !busy;
            if (_dev != null) _dev.interactable = !busy;
            if (_begin != null) _begin.interactable = !busy;
            if (_nameField != null) _nameField.interactable = !busy && !_useSteam;
            if (_routeSwap != null) _routeSwap.interactable = !busy;
            if (_partyField != null) _partyField.interactable = !busy;
            if (_roll != null) _roll.interactable = !busy;
            if (_accountBack != null) _accountBack.interactable = !busy;
            foreach (var control in _landingOrder) if (control != null) control.interactable = !busy;
            if (_busyTrack != null) _busyTrack.gameObject.SetActive(busy);
            if (text != null) SetStatus(text);
        }

        /// A fresh account names its fixed starter party; legacy argument retained for probes.
        public void ShowCreation(List<ClassInfo> classes)
        {
            HideLanding();
            if (_signIn != null) _signIn.gameObject.SetActive(false);
            if (_creation != null) _creation.gameObject.SetActive(true);
            OpenPanelMotion(_creation);
            foreach (var part in _logoParts) part.SetActive(false);
            if (_partyField != null && string.IsNullOrEmpty(_partyField.text))
                _partyField.text = PartyNames[UnityEngine.Random.Range(0, PartyNames.Length)];
            SetStatus("");
            if (_busyTrack != null)
                _busyTrack.rectTransform.anchoredPosition = new Vector2(0f, -284f);
            FocusFirst(_creationOrder);
        }

        public void Dismiss() => Destroy(gameObject);

        // --- build --------------------------------------------------------------------------

        void Build(string accountName, string endpoint)
        {
            Loc.Language=PlayerPrefs.GetString("roi.opt.language",Loc.Language);
            AudioListener.volume=PlayerPrefs.GetInt("roi.opt.master",100)/100f;
            ApplyTitleAudio();
            if(PlayerPrefs.HasKey("roi.opt.shadows"))QualitySettings.shadows=PlayerPrefs.GetInt("roi.opt.shadows")==0?ShadowQuality.Disable:ShadowQuality.All;
            if(PlayerPrefs.HasKey("roi.opt.framecap")){QualitySettings.vSyncCount=0;Application.targetFrameRate=PlayerPrefs.GetInt("roi.opt.framecap");}
            EnsureEventSystem();
            var canvasGo = new GameObject("TitleCanvas");
            canvasGo.transform.SetParent(transform, false);
            var canvas = canvasGo.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = SortOrder;
            canvas.pixelPerfect = true;
            var scaler = canvasGo.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.screenMatchMode = CanvasScaler.ScreenMatchMode.MatchWidthOrHeight;
            scaler.matchWidthOrHeight = 1f;
            canvasGo.AddComponent<GraphicRaycaster>();
            _root = (RectTransform)canvasGo.transform;

            BuildBackdrop();
            BuildSafeArea();
            BuildLogo();
            BuildTapGate();
            BuildLandingTools(endpoint);
            BuildModalShade();
            BuildSignIn(accountName);
            BuildCreation();
            BuildFooter(endpoint);
            LayoutLanding();
            PlayIntro();
        }

        void BuildBackdrop()
        {
            var go = new GameObject("TitleBackdrop", typeof(RectTransform));
            go.transform.SetParent(_root, false);
            _backdrop = go.AddComponent<RawImage>();
            _backdrop.texture = Resources.Load<Texture2D>("TitleUI/Backdrop-Moonlit-v5");
            _backdrop.color = Color.white;
            _backdrop.raycastTarget = true;
            Stretch(_backdrop.rectTransform);
            go.AddComponent<UiBackdropDrift>();
            MenuAtmosphere.Create(_root);   // drifting motes over the painting
            var shade = UiTheme.Flat(_root,"BackdropShade",new Color(.01f,.02f,.04f,.06f));
            Stretch(shade.rectTransform); shade.raycastTarget=false;
            var fade=UiTheme.Flat(_root,"Floor",Color.white);
            fade.sprite=UiTheme.Fade(); fade.raycastTarget=false;
            var fr=fade.rectTransform;
            fr.anchorMin=Vector2.zero; fr.anchorMax=new Vector2(1,0);
            fr.pivot=new Vector2(.5f,0); fr.sizeDelta=new Vector2(0,300); fr.anchoredPosition=Vector2.zero;
        }

        void BuildLogo()
        {
            _logo=Rect(_root,"GameLogo"); _logoParts.Add(_logo.gameObject);
            var title=UiTheme.Label(_logo,"Title","REIGN OF\nIDLERS",14,UiTheme.TextHigh,TextAlignmentOptions.Center);
            if(UiTheme.DisplayFont!=null)title.font=UiTheme.DisplayFont;
            title.characterSpacing=0;
            title.fontStyle=FontStyles.Bold;
            Stretch(title.rectTransform);
        }

        void BuildTapGate()
        {
            _landing=Rect(_root,"Landing"); Stretch(_landing);
            _tapCatcher=QuietButton(_landing,"TapGate",Loc.Get("UI_TITLE_START_GAME"),OpenSignIn);
            _tapLabel=_tapCatcher.GetComponentInChildren<TextMeshProUGUI>();
            _tapLabel.fontSize=20;
            // Gold accent marks this as the screen's one primary action.
            _tapLabel.color=UiTheme.Accent;
            // All decoration shares the button centre and symmetric bounds.
            var backing=Rect(_tapCatcher.transform,"StartBacking").gameObject.AddComponent<TitleEdgeFade>();
            backing.color=new Color(.06f,.07f,.08f,.55f);
            backing.raycastTarget=false; backing.transform.SetAsFirstSibling();
            Stretch(backing.rectTransform);
            Pin(_tapLabel.rectTransform,Vector2.zero,new Vector2(480,64));
            _tapLabel.alignment=TextAlignmentOptions.Midline;
            foreach(float y in new[]{-18f,18f}) {
                var line=Rect(_tapCatcher.transform,y>0?"StartTop":"StartRule").gameObject.AddComponent<TitleEdgeFade>();
                line.color=UiTheme.WithAlpha(UiTheme.Accent,.45f);
                line.raycastTarget=false;
                var lr=line.rectTransform; lr.anchorMin=new Vector2(0,.5f); lr.anchorMax=new Vector2(1,.5f); lr.sizeDelta=new Vector2(0,1.5f); lr.anchoredPosition=new Vector2(0,y);
            }
            var mark=TitleGlyph.Add(_tapCatcher.transform,TitleGlyph.Symbol.Star);
            mark.color=UiTheme.WithAlpha(UiTheme.Accent,.85f);
            Pin(mark.rectTransform,new Vector2(-135,0),new Vector2(16,16));
            var twin=TitleGlyph.Add(_tapCatcher.transform,TitleGlyph.Symbol.Star);twin.color=mark.color;
            Pin(twin.rectTransform,new Vector2(135,0),new Vector2(16,16));
            _startStars=new[]{mark,twin};
        }

        void BuildSignIn(string accountName)
        {
            var panel = UiTheme.Box(_root, "SignIn", UiTheme.Surface, 10);
            StyleTitleDialog(panel);
            _signIn = panel.rectTransform;
            Pin(_signIn, new Vector2(0f, 0f), new Vector2(PanelW, 520f));
            _signIn.gameObject.SetActive(false);

            var head = UiTheme.Caps(_signIn, "Head", Loc.Get("UI_TITLE_SIGN_IN"), 28f, UiTheme.TextHigh,
                                    TextAlignmentOptions.Center);
            Anchor(head.rectTransform, new Vector2(0f, -25f), new Vector2(PanelW - 120f, 45f));

            // ON STEAM THE NAME FIELD IS NOT A LOGIN. The account is whichever Steam account is
            // signed in — there is nothing to type, and a field that looks like a credential but
            // is not would be the screen telling its first lie. It becomes the Steam persona,
            // shown so the player can see WHO they are about to play as.
            bool steam = Roi.Net.SteamAuth.Available;
            var nameLabel = UiTheme.Label(_signIn, "AccountLabel", Loc.Get("UI_TITLE_COMMANDER_NAME"),
                UiTheme.BodySize, UiTheme.TextMid, TextAlignmentOptions.Left);
            Anchor(nameLabel.rectTransform, new Vector2(0f, -90f), new Vector2(PanelW - 64f, 35f));
            _nameField = MakeInput(_signIn, Loc.Get("UI_TITLE_COMMANDER_NAME"),
                                   steam ? Roi.Net.SteamAuth.Persona : accountName,
                                   PanelW - 48f, InputH);
            Anchor(_nameField.GetComponent<RectTransform>(), new Vector2(0f, -130f),
                   new Vector2(PanelW - 48f, InputH));
            if (steam) _nameField.interactable = false;

            // A RELEASE BUILD HAS NO OTHER DOOR, so it never starts on one. Written as `steam ||
            // !DevDoors` rather than plain `steam` because the plain version had a hole worth
            // naming: with Steam closed, `steam` is false, and PLAY in a SHIPPED build would have
            // signed in through /auth/dev — against production, where a name is not a credential
            // and naming somebody else's account is being them. The gate that removed the dev
            // BUTTONS did not cover the primary button falling back to the dev DOOR.
            _useSteam = steam || !DevDoors;
            _play = MakePrimary(_signIn, Loc.Get("UI_TITLE_PLAY"), PanelW - 48f, ButtonH, () =>
            {
                if (_busy) return;
                var who = _nameField != null ? _nameField.text.Trim() : "";
                // Steam asked for and not there: say so and stop. Falling through to any other
                // door is the whole failure this is written against.
                if (_useSteam && !Roi.Net.SteamAuth.Available)
                {
                    SetStatus(Loc.Get("UI_TITLE_STEAM_REQUIRED"));
                    return;
                }
                // The name is only a credential on the DEV door. On Steam the ticket is the
                // credential and this string is never sent, so an empty one is not a problem.
                if (!_useSteam && string.IsNullOrEmpty(who))
                {
                    SetStatus(Loc.Get("UI_TITLE_NEED_A_NAME"));
                    return;
                }
                PlayPressed?.Invoke(who, _useSteam);
            });
            Anchor((RectTransform)_play.transform, new Vector2(0f, -226f),
                   new Vector2(PanelW - 48f, ButtonH));

            // THE THIRD DOOR, and the reason it is a button rather than an inference: a machine
            // with Steam is not the same thing as a player who wants to sign in through it. This
            // repo is worked on daily against a local server with no publisher key, where the Steam
            // door cannot open at all — and until this button existed, having Steam installed meant
            // there was no way to say so. It only appears where there is a choice to make.
            if (steam && DevDoors)
            {
                _routeSwap = HudBar.MakeGhostButton(_signIn, Loc.Get("UI_TITLE_WITHOUT_STEAM"),
                                                    PanelW - 48f, 80f, SwapRoute, UiTheme.BodySize);
                // A STABLE NAME, because MakeGhostButton derives its GameObject name from the
                // TRANSLATED label — a probe that found this button by its name would pass in
                // English and fail the moment anyone ran the game in Thai.
                _routeSwap.gameObject.name = "RouteSwap";
                Anchor((RectTransform)_routeSwap.transform, new Vector2(0f, -322f),
                       new Vector2(PanelW - 48f, 80f));
            }

            // Ghost weight, and it says what it is. It starts the sim on a client seed with the
            // cheats on: nothing it produces can be pushed, which is exactly what a dev session is.
            if (DevDoors)
            {
                _dev = HudBar.MakeGhostButton(_signIn, Loc.Get("UI_TITLE_LOCAL_SESSION_DEV"), PanelW - 48f, 80f,
                                              () => { if (!_busy) DevPressed?.Invoke(); },
                                              UiTheme.BodySize);
                Anchor((RectTransform)_dev.transform, new Vector2(0f, steam ? -418f : -322f),
                       new Vector2(PanelW - 48f, 80f));
            }

            // A shipped build with no Steam has nowhere to go, and saying so on the panel is the
            // difference between a game that explains itself and one that appears broken.
            if (!steam && !DevDoors) SetStatus(Loc.Get("UI_TITLE_STEAM_REQUIRED"));

            BuildBusyBar();
            RegisterFocus(_signInOrder, _nameField, _play, _routeSwap, _dev);
            _accountBack = AddClose(_signIn, _signInOrder);
            StyleFantasySignIn(panel, head, nameLabel, steam);
            _nameField.onSubmit.AddListener(_ => { if (!_busy && !_useSteam) _play.onClick.Invoke(); });
        }

        /// Move PLAY between the two online doors. The name field follows: on Steam the ticket is
        /// the credential and a typed name would be a lie about what is being sent, so it is greyed
        /// and shows the persona; off Steam it is the credential and has to be typeable again.
        void SwapRoute()
        {
            if (_busy) return;
            _useSteam = !_useSteam;
            if (_nameField != null)
            {
                _nameField.interactable = !_useSteam;
                if (_useSteam) _nameField.text = Roi.Net.SteamAuth.Persona;
                else if (_nameField.text == Roi.Net.SteamAuth.Persona) _nameField.text = "";
            }
            SetLabel(_routeSwap, Loc.Get(_useSteam ? "UI_TITLE_WITHOUT_STEAM" : "UI_TITLE_WITH_STEAM"));
            SetStatus(Loc.Get(_useSteam ? "UI_TITLE_ROUTE_STEAM" : "UI_TITLE_ROUTE_NAME"));
        }

        /// Both button builders put their text in a child, so a label swap has to go find it rather
        /// than assume a component on the button itself.
        static void SetLabel(Button b, string text)
        {
            if (b == null) return;
            var t = b.GetComponentInChildren<TMPro.TMP_Text>();
            if (t != null) t.text = text;
        }

        /// An indeterminate bar under the panel. A login crosses a cold-started server and a
        /// database wake-up; without a moving thing the wait reads as a hang.
        void BuildBusyBar()
        {
            var go = new GameObject("BusyTrack", typeof(RectTransform));
            go.transform.SetParent(_root, false);
            _busyTrack = go.AddComponent<WuwaLoadingBar>();
            _busyTrack.raycastTarget = false;
            Pin(_busyTrack.rectTransform, new Vector2(0f, -276f), new Vector2(PanelW - 48f, 12f));
            _busyTrack.gameObject.SetActive(false);
        }

        void BuildCreation()
        {
            var panel = UiTheme.Box(_root, "Creation", UiTheme.Surface, 10);
            DialogSurface.Apply(panel);
            _creation = panel.rectTransform;
            Pin(_creation, Vector2.zero, new Vector2(CreateW, CreateH));
            _creation.gameObject.SetActive(false);

            var head = UiTheme.Label(_creation, "CreationHeading", Loc.Get("UI_TITLE_FOUND_YOUR_PARTY"), 28f, UiTheme.TextHigh,
                                    TextAlignmentOptions.Center);
            Anchor(head.rectTransform, new Vector2(0f, -24f), new Vector2(CreateW - 64f, 42f));

            var sub = UiTheme.Label(_creation, "Sub",
                                    Loc.Get("UI_TITLE_STARTER_PAIR"),
                                    UiTheme.BodySize, UiTheme.TextMid, TextAlignmentOptions.Center);
            Anchor(sub.rectTransform, new Vector2(0f, -70f), new Vector2(CreateW - 64f, 28f));

            var pair = UiTheme.Label(_creation, "StarterPair", Loc.Get("UI_TITLE_STARTER_NAMES"), 30f,
                UiTheme.TextHigh, TextAlignmentOptions.Center);
            Anchor(pair.rectTransform, new Vector2(0f, -108f), new Vector2(CreateW - 64f, 42f));

            var partyLabel = UiTheme.Label(_creation, "PartyLabel", Loc.Get("UI_TITLE_PARTY_NAME"),
                UiTheme.BodySize, UiTheme.TextMid, TextAlignmentOptions.Left);
            Anchor(partyLabel.rectTransform, new Vector2(0f, -170f), new Vector2(CreateW - 64f, 22f));
            _partyField = MakeInput(_creation, Loc.Get("UI_TITLE_PARTY_NAME"), "", 374f, 48f);
            Anchor(_partyField.GetComponent<RectTransform>(), new Vector2(-61f, -200f),
                   new Vector2(374f, 48f));
            _roll = HudBar.MakeGhostButton(_creation, Loc.Get("UI_TITLE_RANDOM"), 110f, 48f,
                () => { if (!_busy && _partyField != null) _partyField.text = PartyNames[UnityEngine.Random.Range(0, PartyNames.Length)]; },
                UiTheme.CaptionSize);
            Anchor((RectTransform)_roll.transform, new Vector2(193f, -200f), new Vector2(110f, 48f));

            _begin = MakePrimary(_creation, Loc.Get("UI_TITLE_BEGIN"), 496f, 48f, () =>
            {
                if (_busy) return;
                var party = _partyField != null ? _partyField.text.Trim() : "";
                if (string.IsNullOrEmpty(party)) { SetStatus(Loc.Get("UI_TITLE_NEED_PARTY_NAME")); _partyField.Select(); return; }
                CreatePressed?.Invoke("seraphine", party);
            });
            Anchor((RectTransform)_begin.transform, new Vector2(0f, -272f), new Vector2(496f, 48f));
            var closeCreation=DialogSurface.CloseIcon(_creation,ReturnToLanding);
            RegisterFocus(_creationOrder, _partyField, _roll, _begin, closeCreation);
            _partyField.onSubmit.AddListener(_ => { if (!_busy) _begin.onClick.Invoke(); });
        }

        void BuildFooter(string endpoint)
        {
            _status = UiTheme.Label(_root, "Status", _initialStatus ?? "", 22f, UiTheme.TextMid,
                                    TextAlignmentOptions.Center);
            _status.overflowMode = TextOverflowModes.Overflow;
            _status.textWrappingMode = TextWrappingModes.Normal;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0.5f, 0f); sr.anchorMax = new Vector2(0.5f, 0f);
            sr.pivot = new Vector2(0.5f, 0f);
            sr.anchoredPosition = new Vector2(0f, 20f);
            sr.sizeDelta = new Vector2(760f, 86f);

            // The build, where a support question can read it off a screenshot. A version skew is
            // the single likeliest reason a login fails, so it is on the login screen.
            _version = UiTheme.Label(_root, "Version",
                                     Roi.Sim.SimSession.SimVersion(), 13f,
                                     UiTheme.TextLow, TextAlignmentOptions.Left);
            var vr = _version.rectTransform;
            vr.anchorMin = vr.anchorMax = new Vector2(1f, 1f);
            vr.pivot = new Vector2(1f, 1f);
            vr.anchoredPosition = new Vector2(-28f, -20f);
            vr.sizeDelta = new Vector2(400f, 24f); _version.alignment=TextAlignmentOptions.Right;


        }

        // --- behaviour ----------------------------------------------------------------------

        void OpenSignIn()
        {
            if (_busy || _signIn == null || _signIn.gameObject.activeSelf || _popup != null) return;
            HideLanding();
            _signIn.gameObject.SetActive(true);
            OpenPanelMotion(_signIn);
            if (_tapLabel != null) _tapLabel.gameObject.SetActive(false);
            if (_tapCatcher != null) _tapCatcher.gameObject.SetActive(false);
            FocusFirst(_signInOrder);
        }

        void Update()
        {
            UpdateTitleMotion();
            var keyboard = Keyboard.current;
            if (UpdateLandingInput(keyboard)) return;
            if (!_busy && keyboard != null && keyboard.tabKey.wasPressedThisFrame)
                MoveFocus(_creation != null && _creation.gameObject.activeSelf ? _creationOrder : _signInOrder,
                    keyboard.shiftKey.isPressed ? -1 : 1);
        }

        // Scope keyboard navigation to the visible title form, never to the HUD behind it.
        static void RegisterFocus(List<Selectable> order, params Selectable[] controls)
        {
            foreach (var control in controls)
            {
                if (control == null) continue;
                control.navigation = new Navigation { mode = Navigation.Mode.None };
                control.gameObject.AddComponent<UiFocusRing>();
                order.Add(control);
            }
        }

        static void FocusFirst(List<Selectable> order)
        {
            foreach (var control in order)
                if (control != null && control.IsActive() && control.IsInteractable())
                { control.Select(); return; }
        }

        static void MoveFocus(List<Selectable> order, int direction)
        {
            if (EventSystem.current == null || order.Count == 0) return;
            var selected = EventSystem.current.currentSelectedGameObject;
            int index = order.FindIndex(control => control != null && control.gameObject == selected);
            if (index < 0) index = direction > 0 ? -1 : 0;
            for (int step = 1; step <= order.Count; step++)
            {
                var next = order[(index + direction * step + order.Count * 2) % order.Count];
                if (next != null && next.IsActive() && next.IsInteractable())
                { next.Select(); return; }
            }
        }

        // --- widgets ------------------------------------------------------------------------

        /// The one ACCENT-filled button in the client. A gacha title has exactly one thing to press
        /// and it should look like it; every other button in the game stays on the quiet surface.
        static Button MakePrimary(Transform parent, string label, float w, float h, Action onClick)
        {
            var img = UiTheme.Box(parent, "Primary_" + label, UiTheme.Accent);
            TacticalUi.Primary(img);
            var btn = img.gameObject.AddComponent<Button>();
            btn.targetGraphic = img;
            btn.navigation = new Navigation { mode = Navigation.Mode.None };
            var colors = btn.colors;
            colors.highlightedColor = new Color(1.12f, 1.12f, 1.12f, 1f);
            colors.pressedColor = new Color(0.86f, 0.86f, 0.86f, 1f);
            colors.disabledColor = new Color(0.45f, 0.45f, 0.45f, 1f);
            colors.fadeDuration = 0.08f;
            btn.colors = colors;
            if (onClick != null) btn.onClick.AddListener(() => onClick());
            var rt = img.rectTransform;
            rt.sizeDelta = new Vector2(w, h);

            // Dark type on the accent: the panel colour, so the button reads as a cut-out of the
            // screen rather than as a sticker on it.
            var t = UiTheme.Caps(rt, "Label", label, 25f, UiTheme.Panel,
                                 TextAlignmentOptions.Center);
            Stretch(t.rectTransform);
            return btn;
        }

        /// A text field, built from parts because there is no prefab anywhere in this UI.
        static TMP_InputField MakeInput(Transform parent, string placeholder, string value,
                                        float w, float h)
        {
            var img = UiTheme.Box(parent, "Input_" + placeholder, UiTheme.Panel);
            var rt = img.rectTransform;
            rt.sizeDelta = new Vector2(w, h);
            UiTheme.Border(rt, UiTheme.Line);

            var area = new GameObject("TextArea", typeof(RectTransform));
            area.transform.SetParent(rt, false);
            var art = (RectTransform)area.transform;
            art.anchorMin = Vector2.zero; art.anchorMax = Vector2.one;
            art.offsetMin = new Vector2(UiTheme.PadSmall, 1f);
            art.offsetMax = new Vector2(-UiTheme.PadSmall, -1f);
            area.AddComponent<RectMask2D>();

            var ph = UiTheme.Label(art, "Placeholder", placeholder, 25f,
                                   UiTheme.TextLow, TextAlignmentOptions.Left);
            Stretch(ph.rectTransform);
            var txt = UiTheme.Label(art, "Text", "", 25f, UiTheme.TextHigh,
                                    TextAlignmentOptions.Left);
            // OVERFLOW, not ellipsis. A single-line rect with Ellipsis renders NOTHING when the
            // line does not fit — the same trap that ate the item cells and the paper-doll labels.
            txt.overflowMode = TextOverflowModes.Overflow;
            Stretch(txt.rectTransform);

            var field = img.gameObject.AddComponent<TMP_InputField>();
            field.textViewport = art;
            field.textComponent = txt;
            field.placeholder = ph;
            field.targetGraphic = img;
            field.lineType = TMP_InputField.LineType.SingleLine;
            field.characterLimit = 24;
            field.customCaretColor = true;
            field.caretColor = UiTheme.Accent;
            field.selectionColor = new Color(UiTheme.Accent.r, UiTheme.Accent.g, UiTheme.Accent.b, 0.28f);
            field.text = value ?? "";
            return field;
        }

        static void EnsureEventSystem()
        {
            if (UnityEngine.EventSystems.EventSystem.current != null) return;
            var go = new GameObject("EventSystem");
            go.AddComponent<UnityEngine.EventSystems.EventSystem>();
            go.AddComponent<UnityEngine.InputSystem.UI.InputSystemUIInputModule>();
        }

        static void Stretch(RectTransform rt)
        {
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.zero; rt.offsetMax = Vector2.zero;
        }

        /// Centred on the SCREEN, offset from its middle — the logo, the panels, the status line.
        static void Pin(RectTransform rt, Vector2 offset, Vector2 size)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(0.5f, 0.5f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            rt.sizeDelta = size;
            rt.anchoredPosition = offset;
        }

        /// Centred on the TOP EDGE of its parent panel: everything inside a panel is placed from
        /// its top, so a taller panel grows downward and nothing above it moves.
        static void Anchor(RectTransform rt, Vector2 offset, Vector2 size)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(0.5f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = size;
            rt.anchoredPosition = offset;
        }
    }
}
