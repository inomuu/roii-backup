// A speaker, a line, and something that waits — over the running game. Ported from
// godot/views/dialogue_box.gd.
//
// Used by tutorial hints, quest prompts, event notices, a character speaking mid-fight, and story
// scenes. The sim keeps ticking underneath: this touches no save and no replay.
//
// NOT A SINGLETON, and that is the design. Two conversations can be live at once — a story scene
// while a tutorial waits over another screen — so each mode owns its own instance with its own
// cursor. They share no state and raise no global event, which is the whole reason this is a plain
// view rather than a service.
//
// Its canvas sits at 25: above popups (20) so a hint clears an open picker, but BELOW the action
// bar (40) — a hint that says "open your inventory" must not cover the button it points at.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.Story
{
    public sealed class DialogueBox : MonoBehaviour
    {
        public const int CanvasLayer = 25;
        const float CharsPerSec = 45f;
        const float PanelMaxW = 900f, BodyMinH = 58f, GapAboveBar = 16f;

        /// Raised once per script, whichever branch it ended on.
        public event System.Action<string> ScriptFinished;
        /// `choiceKey` is the chosen line's loc key — stable and unique, so the server can use
        /// (account, scriptId, choiceKey) as the idempotency key for whatever `effect` buys.
        /// `effect` is opaque here: this node never parses it, it only carries it out.
        public event System.Action<string, string, string> ChoiceMade;
        /// A `@ name args` line was reached. The host decides which cues exist and what they do.
        /// This node only relays them, so the set of things a script can cause is whatever the
        /// host chose to listen for and nothing else.
        public event System.Action<string, string, string[]> CueReached;

        sealed class Entry
        {
            public string Id;
            public DialogueScript Script;
            public string EntryBlock;
            public bool Modal;
        }

        readonly List<Entry> _queue = new();
        Entry _cur;
        string _title = DialogueScript.Entry;
        int _index;
        float _typed;
        bool _held, _modeActive = true;

        Image _dim;
        RectTransform _panel, _choicesRoot;
        TextMeshProUGUI _speaker, _body, _more;
        readonly List<GameObject> _choiceBtns = new();

        public static DialogueBox Create(Transform parent)
        {
            var go = new GameObject("DialogueBox");
            go.transform.SetParent(parent, false);
            var box = go.AddComponent<DialogueBox>();
            box.Build();
            box.SetShown(false);
            return box;
        }

        void Build()
        {
            var canvas = gameObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = CanvasLayer;
            gameObject.AddComponent<GraphicRaycaster>();
            var scaler = gameObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.matchWidthOrHeight = 1f;

            var root = (RectTransform)transform;

            // Behind the panel and full-screen: for a MODAL script this both dims the game and
            // swallows clicks, so a story beat cannot be answered by clicking through it.
            _dim = Roi.UI.UiTheme.Flat(root, "Dim", new Color(0.01f, 0.01f, 0.02f, 0.55f));
            var dr = _dim.rectTransform;
            dr.anchorMin = Vector2.zero; dr.anchorMax = Vector2.one;
            dr.offsetMin = Vector2.zero; dr.offsetMax = Vector2.zero;
            var block = _dim.gameObject.AddComponent<Button>();
            block.targetGraphic = _dim;
            block.transition = Selectable.Transition.None;
            block.onClick.AddListener(Skip);

            var panel = Roi.UI.UiTheme.Box(root, "Panel", Roi.UI.UiTheme.Panel, 10);
            Roi.UI.DialogSurface.Apply(panel);
            _panel = panel.rectTransform;
            _panel.anchorMin = new Vector2(0.5f, 0f); _panel.anchorMax = new Vector2(0.5f, 0f);
            _panel.pivot = new Vector2(0.5f, 0f);
            _panel.sizeDelta = new Vector2(PanelMaxW, BodyMinH + 76f);
            _panel.anchoredPosition = new Vector2(0f, Roi.UI.UiLayout.Touch + GapAboveBar * 2f);
            // The panel takes the tap too: a story box you can only advance by hitting the dim
            // would be unadvanceable the moment a script is non-modal.
            var tap = panel.gameObject.AddComponent<Button>();
            tap.targetGraphic = panel;
            tap.transition = Selectable.Transition.None;
            tap.onClick.AddListener(Skip);

            _speaker = Roi.UI.UiTheme.Caps(_panel, "Speaker", "", Roi.UI.UiTheme.HeadSize, Roi.UI.UiTheme.Accent,
                                    TextAlignmentOptions.Left);
            var sr = _speaker.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(Roi.UI.UiTheme.PadLarge, 0f);
            sr.offsetMax = new Vector2(-Roi.UI.UiTheme.PadLarge, 0f);
            sr.sizeDelta = new Vector2(sr.sizeDelta.x, 26f);
            sr.anchoredPosition = new Vector2(0f, -Roi.UI.UiTheme.Pad);

            _body = Roi.UI.UiTheme.Label(_panel, "Body", "", Roi.UI.UiTheme.BodySize, Roi.UI.UiTheme.TextHigh,
                                  TextAlignmentOptions.TopLeft);
            _body.textWrappingMode = TextWrappingModes.Normal;
            var br = _body.rectTransform;
            br.anchorMin = new Vector2(0f, 0f); br.anchorMax = new Vector2(1f, 1f);
            br.offsetMin = new Vector2(Roi.UI.UiTheme.PadLarge, Roi.UI.UiTheme.PadLarge);
            br.offsetMax = new Vector2(-Roi.UI.UiTheme.PadLarge, -(Roi.UI.UiTheme.Pad + 30f));

            // "..." rather than a caret glyph: Noto Sans has no U+25BE, and a missing glyph does
            // not fall back — it draws as a tofu box, in the corner of every line of the story.
            _more = Roi.UI.UiTheme.Label(_panel, "More", "...", Roi.UI.UiTheme.CaptionSize, Roi.UI.UiTheme.TextLow,
                                  TextAlignmentOptions.BottomRight);
            var mr = _more.rectTransform;
            mr.anchorMin = Vector2.zero; mr.anchorMax = Vector2.one;
            mr.offsetMin = new Vector2(0f, Roi.UI.UiTheme.PadSmall);
            mr.offsetMax = new Vector2(-Roi.UI.UiTheme.PadLarge, 0f);

            var choices = new GameObject("Choices", typeof(RectTransform));
            choices.transform.SetParent(_panel, false);
            _choicesRoot = (RectTransform)choices.transform;
            _choicesRoot.anchorMin = new Vector2(0f, 0f); _choicesRoot.anchorMax = new Vector2(1f, 0f);
            _choicesRoot.pivot = new Vector2(0.5f, 0f);
            _choicesRoot.sizeDelta = new Vector2(0f, 0f);
            Roi.UI.DialogSurface.CloseIcon(_panel,Clear);
            _speaker.margin=new Vector4(0,0,48,0);
        }

        // --- the public surface ------------------------------------------------------------

        /// Queue a script. `entryBlock` names where to start; `modal` dims and blocks the game
        /// behind it.
        public void Play(string id, DialogueScript script, string entryBlock = DialogueScript.Entry,
                         bool modal = true)
        {
            _queue.Add(new Entry { Id = id, Script = script, EntryBlock = entryBlock, Modal = modal });
            if (_cur == null) StartNext();
        }

        /// Finish typing the current line, or advance if it has already finished.
        public void Skip()
        {
            if (_cur == null) return;
            if (IsTyping())
            {
                _typed = _body.text.Length;
                _body.maxVisibleCharacters = int.MaxValue;
                OnLineRevealed();
            }
            else if (_choiceBtns.Count == 0) Advance();
        }

        public bool IsBusy => _cur != null;

        /// Drop everything WITHOUT raising ScriptFinished — the script did not finish, it was
        /// called off. A failed story mission uses this: raising the finished event would look
        /// like a completed mission to whatever grants the reward.
        public void Clear()
        {
            _queue.Clear();
            _cur = null;
            _held = false;
            ClearChoices();
            SetShown(false);
        }

        /// Called from a cue handler to say "I am taking this one — do not advance". The script
        /// resumes when the host calls `Release()`. Anything else and the cue is fire-and-forget.
        public void Hold()
        {
            _held = true;
            // Get out of the way. A cue line has no text, so a held box is an EMPTY panel — and a
            // modal one also drags its dim over the very fight the script just handed off to.
            SetShown(false);
        }

        public void Release()
        {
            if (!_held) return;
            _held = false;
            SetShown(true);
            Advance();
        }

        /// Set by the host on a mode switch. Without it the typewriter runs to the end of a line
        /// nobody is looking at and the player comes back to a finished scene. The cursor and the
        /// reveal position are left untouched, so returning resumes mid-word.
        public void SetModeActive(bool v) => _modeActive = v;

        // --- script walking ------------------------------------------------------------------

        void StartNext()
        {
            if (_queue.Count == 0) { _cur = null; SetShown(false); return; }
            _cur = _queue[0];
            _queue.RemoveAt(0);
            _dim.enabled = _cur.Modal;
            _dim.raycastTarget = _cur.Modal;
            _title = _cur.EntryBlock;
            _index = 0;
            SetShown(true);
            ShowLine();
        }

        void Finish()
        {
            var id = _cur != null ? _cur.Id : "";
            _cur = null;
            ScriptFinished?.Invoke(id);
            StartNext();
        }

        /// Move to `title`; "END" (or a title the script does not define) ends the script.
        void Jump(string title)
        {
            if (title == "END" || _cur == null || !_cur.Script.Blocks.ContainsKey(title)) { Finish(); return; }
            _title = title;
            _index = 0;
            ShowLine();
        }

        void Advance() { _index++; ShowLine(); }

        List<DialogueScript.Line> Block()
            => _cur != null && _cur.Script.Blocks.TryGetValue(_title, out var b)
             ? b : new List<DialogueScript.Line>();

        void ShowLine()
        {
            ClearChoices();
            var block = Block();
            // Running off the end of a block ends the script — a block that means to continue says
            // so with an explicit goto.
            if (_index >= block.Count) { Finish(); return; }
            var line = block[_index];

            if (line.Goto != null && line.Choices == null) { Jump(line.Goto); return; }

            // A cue is an instruction to the HOST — start a battle, change the backdrop — not a
            // line. This node never interprets one: it names it, hands over the arguments and
            // moves on, which is what keeps a text file from reaching into the game.
            if (line.Cue != null)
            {
                // The host may call `Hold()` from inside the handler — the event is synchronous,
                // so by the time it returns we know whether it took ownership. `@ scene` changes a
                // backdrop and the script carries on; `@ battle` hands off to a stage and the
                // script must WAIT, or a scene's closing lines play over the fight before them.
                _held = false;
                CueReached?.Invoke(_cur.Id, line.Cue, line.Args ?? new string[0]);
                if (!_held) Advance();
                return;
            }

            // Resolved at DISPLAY time, never when queued: a language switch mid-scene must repaint
            // the line, and a key cached at enqueue would render the old locale forever.
            //
            // The speaker uses the FALLBACK form on purpose, unlike the rest of the project: a
            // dialogue file carries the speaker's written name, so an un-keyed draft shows "Aria"
            // rather than "SPK_ARIA".
            _speaker.text = line.Speaker != "" ? Roi.UI.Loc.GetOr(line.Speaker, line.SpeakerName) : "";
            _speaker.gameObject.SetActive(_speaker.text != "");
            _body.text = LineText(line);
            _body.maxVisibleCharacters = 0;
            _typed = 0f;
            _more.gameObject.SetActive(false);
            // A line that is only a set of choices has nothing to type, so the reveal would never
            // complete and the buttons would never be built.
            if (!IsTyping()) OnLineRevealed();
        }

        /// A keyed line resolves through Loc; an UNkeyed one shows its inline text verbatim. The
        /// second case is what lets a writer's document be dropped in and played the same day.
        /// NEVER NULL. A line that is only a set of choices carries neither a key nor inline text,
        /// and TMP takes a null `text` without complaint — the crash lands one call later, in the
        /// typewriter, reading `.Length` off it.
        static string LineText(DialogueScript.Line line)
            => !string.IsNullOrEmpty(line.Text) ? Roi.UI.Loc.Get(line.Text)
             : (line.Raw ?? "");

        bool IsTyping() => _body != null && !string.IsNullOrEmpty(_body.text)
                           && _typed < _body.text.Length;

        void Update()
        {
            if (_cur == null || !_modeActive || _held) return;
            if (!IsTyping()) return;
            _typed += Time.unscaledDeltaTime * CharsPerSec;
            _body.maxVisibleCharacters = Mathf.Min(_body.text.Length, Mathf.FloorToInt(_typed));
            if (!IsTyping()) OnLineRevealed();
        }

        void OnLineRevealed()
        {
            _body.maxVisibleCharacters = int.MaxValue;
            var block = Block();
            var line = _index < block.Count ? block[_index] : null;
            if (line != null && line.Choices != null) { BuildChoices(line.Choices); return; }
            _more.gameObject.SetActive(true);
        }

        void BuildChoices(List<DialogueScript.Choice> choices)
        {
            float y = 0f;
            foreach (var c in choices)
            {
                var text = !string.IsNullOrEmpty(c.Text) ? Roi.UI.Loc.Get(c.Text) : c.Raw;
                var choice = c;
                var b = Roi.UI.HudBar.MakeGhostButton(_choicesRoot, text, PanelMaxW - Roi.UI.UiTheme.PadLarge * 2f,
                                                      Roi.UI.UiLayout.Touch, () => Pick(choice));
                var rt = (RectTransform)b.transform;
                rt.anchorMin = new Vector2(0.5f, 1f); rt.anchorMax = new Vector2(0.5f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.anchoredPosition = new Vector2(0f, -y);
                _choiceBtns.Add(b.gameObject);
                y += Roi.UI.UiLayout.Touch + Roi.UI.UiTheme.PadSmall;
            }
            // The panel grows to hold them, and the choices hang below the body.
            _choicesRoot.sizeDelta = new Vector2(0f, y);
            _panel.sizeDelta = new Vector2(PanelMaxW, BodyMinH + 76f + y);
            // The root is bottom-anchored: keep answers inside the enlarged panel.
            _choicesRoot.anchoredPosition = new Vector2(0f, Roi.UI.UiTheme.PadLarge);
            _body.rectTransform.offsetMin = new Vector2(Roi.UI.UiTheme.PadLarge,
                y + Roi.UI.UiTheme.PadLarge + Roi.UI.UiTheme.PadSmall);
            _more.gameObject.SetActive(false);
        }

        void Pick(DialogueScript.Choice c)
        {
            var key = !string.IsNullOrEmpty(c.Text) ? c.Text : c.Raw;
            ChoiceMade?.Invoke(_cur != null ? _cur.Id : "", key, c.Effect ?? "");
            ClearChoices();
            Jump(c.Goto ?? "END");
        }

        void ClearChoices()
        {
            foreach (var go in _choiceBtns) Destroy(go);
            _choiceBtns.Clear();
            _choicesRoot.sizeDelta = new Vector2(0f, 0f);
            _body.rectTransform.offsetMin = new Vector2(Roi.UI.UiTheme.PadLarge, Roi.UI.UiTheme.PadLarge);
            _panel.sizeDelta = new Vector2(PanelMaxW, BodyMinH + 76f);
        }

        void SetShown(bool on)
        {
            _dim.gameObject.SetActive(on && _cur != null && _cur.Modal);
            _panel.gameObject.SetActive(on);
        }
    }
}
