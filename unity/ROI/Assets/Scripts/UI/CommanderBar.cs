// THE COMMANDER'S ACTION ROW, sitting just above the bottom bar. Ported from views/skill_bar.gd.
//
// ONE RESPONSIBILITY: show every castable Commander skill, and say which one the player asked for.
// It owns no targeting at all. A press raises `ActivateRequested` and stops there; the ARENA
// decides what that means, because the arena is what knows where the cursor is and what a cell is.
//
// THE LIST COMES FROM THE SIM — `commander_skills` in the view state, built from
// `events::COMMANDER_SKILLS`: ids, cooldowns and target kinds all. Nothing here enumerates the
// skills, so shipping a second one is a row in that Rust catalog plus a name in strings.csv, and
// this file does not change.
//
// REBUILT ONLY WHEN THE SET CHANGES, never per tick. Freeing and recreating Buttons under a live
// pointer is how a bar ends up unclickable — the press lands on a control that has been destroyed.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class CommanderBar : MonoBehaviour
    {
        const float BtnW = 168f, BtnH = 48f, Gap = 8f, BarGap = 8f;

        /// The skill the player wants to use. Carries the ID, not the slot: the arena looks it up
        /// in the same `commander_skills` list to learn how it asks for a target.
        public event System.Action<string> ActivateRequested;

        /// Hotkeys BY SLOT. Position in the sim's list is the position on the bar is the number you
        /// press — one ordering for all three, so a skill cannot move on screen without its key
        /// moving with it. The number row rather than letters: A–Z is full on the action bar.
        static readonly Key[] SlotKeys = { Key.Digit1, Key.Digit2, Key.Digit3, Key.Digit4, Key.Digit5 };

        RectTransform _row;
        readonly List<Button> _btns = new();
        readonly List<string> _ids = new();
        string _textKey = "";
        string _aiming = "";

        public static CommanderBar Create(Transform parent)
        {
            var go = new GameObject("CommanderBar", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var bar = go.AddComponent<CommanderBar>();
            bar._row = (RectTransform)go.transform;
            bar._row.anchorMin = bar._row.anchorMax = new Vector2(0.5f, 0f);
            bar._row.pivot = new Vector2(0.5f, 0f);
            bar._row.anchoredPosition = new Vector2(0f, UiLayout.BottomBar + BarGap);
            bar._row.sizeDelta = new Vector2(0f, BtnH);
            return bar;
        }

        /// Which skill the arena is aiming — the bar lights that button as a reminder of what the
        /// next click will cast. Pushed from the arena; the bar never decides it.
        public void SetAiming(string skillId)
        {
            _aiming = skillId ?? "";
            for (int i = 0; i < _btns.Count; i++) Light(_btns[i], _ids[i] == _aiming && _aiming != "");
        }

        public void OnView(string json)
        {
            var v = JsonUtility.FromJson<BarView>(json);
            var skills = v != null && v.commander_skills != null ? v.commander_skills : new Skill[0];

            bool same = skills.Length == _ids.Count;
            if (same) for (int i = 0; i < skills.Length; i++) if (skills[i].id != _ids[i]) { same = false; break; }
            if (!same) Rebuild(skills);
            Refresh(v != null ? v.commander_charges : 0, skills);
        }

        void Rebuild(Skill[] skills)
        {
            foreach (var b in _btns) Destroy(b.gameObject);
            _btns.Clear();
            _ids.Clear();
            foreach (var s in skills)
            {
                var id = s.id;
                _ids.Add(id);
                var b = HudBar.MakeButton(_row, "", "", BtnW, BtnH, () => ActivateRequested?.Invoke(id));
                var rt = (RectTransform)b.transform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
                rt.pivot = new Vector2(0f, 0.5f);
                _btns.Add(b);
            }
            _textKey = "";                       // fresh buttons carry no text yet
            _row.sizeDelta = new Vector2(Width(), BtnH);
            for (int i = 0; i < _btns.Count; i++)
                ((RectTransform)_btns[i].transform).anchoredPosition = new Vector2(i * (BtnW + Gap), 0f);
            SetAiming(_aiming);
            gameObject.SetActive(_btns.Count > 0);
        }

        float Width() => _btns.Count == 0 ? 0f : _btns.Count * BtnW + (_btns.Count - 1) * Gap;

        /// Charges as pips, or the cooldown in whole seconds. Guarded on a CHANGE rather than run
        /// per tick: this is fed at tick rate, and re-assigning `text` re-runs TMP's shaping for a
        /// string that is almost always identical.
        void Refresh(int charges, Skill[] skills)
        {
            var key = new System.Text.StringBuilder(Loc.Language).Append('/').Append(charges);
            foreach (var s in skills) key.Append('/').Append(s.cd);
            if (key.ToString() == _textKey) return;
            _textKey = key.ToString();

            for (int i = 0; i < _btns.Count && i < skills.Length; i++)
            {
                int cd = skills[i].cd;
                var label = GameText.CommanderSkillName(_ids[i]);
                _btns[i].interactable = charges > 0 && cd == 0;
                var t = _btns[i].GetComponentInChildren<TextMeshProUGUI>();
                // Dim the LABEL as well as the panel. Unity's disabled tint multiplies the image
                // only, so a bright label on a dimmed panel reads as available.
                t.color = _btns[i].interactable ? UiTheme.TextHigh : UiTheme.TextLow;
                if (cd > 0)
                    // Ceil, so a cooldown with any ticks left never reads "0s" — which looks ready.
                    t.text = $"[{i + 1}] {label}  {Mathf.CeilToInt(cd / TicksPerSec)}s";
                else
                    t.text = $"[{i + 1}] {label}  " + (charges > 0 ? new string('●', Mathf.Min(charges, 5)) : "—");
            }
        }

        const float TicksPerSec = 10f;

        /// The AIMED skill's button sits lit — a reminder of what the next click will cast.
        ///
        /// It restores `UiTheme.Surface`, not white. A Button's state colours MULTIPLY its image,
        /// so writing white here does not mean "no highlight", it means "the surface is white" —
        /// and the disabled state then tinted that to a bright grey slab instead of a dimmed
        /// panel. The same trap the ghost buttons hit from the other direction.
        static void Light(Button b, bool on)
        {
            var img = b.GetComponent<Image>();
            if (img != null) img.color = on ? UiTheme.SurfaceActive : UiTheme.Surface;
            var t = b.GetComponentInChildren<TextMeshProUGUI>();
            if (t != null) t.color = on ? UiTheme.Accent : UiTheme.TextHigh;
        }

        void Update()
        {
            var kb = Keyboard.current;
            if (kb == null || _btns.Count == 0 || UiInputFocus.Typing || UiInputFocus.Modal) return;
            for (int i = 0; i < _btns.Count && i < SlotKeys.Length; i++)
            {
                if (!kb[SlotKeys[i]].wasPressedThisFrame) continue;
                // A disabled slot ignores its key, so pressing a skill on cooldown does exactly
                // what clicking it does: nothing. Entering aim mode for a cast the sim would refuse
                // is worse than no response — the player lines up a shot and the click evaporates.
                if (!_btns[i].interactable) continue;
                ActivateRequested?.Invoke(_ids[i]);
                return;
            }
        }

        [System.Serializable] class BarView { public int commander_charges; public Skill[] commander_skills; }
        [System.Serializable] public class Skill { public string id, target; public int cd, cd_max, side; }
    }
}
