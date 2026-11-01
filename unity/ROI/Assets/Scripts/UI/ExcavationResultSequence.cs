using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Results already exist on the server. Skip/close only control their presentation.
    public sealed class ExcavationResultSequence : MonoBehaviour
    {
        readonly List<UiEntrance> _cards = new();
        TMP_Text _label;
        UiConfirm _modal;
        public bool Complete { get; private set; }
        public void Begin(UiConfirm modal, List<RectTransform> cards)
        {
            _modal = modal;
            var close = modal.transform.Find("Sheet/Back").GetComponent<Button>();
            _label = close.GetComponentInChildren<TMP_Text>();
            close.onClick.RemoveAllListeners();
            close.onClick.AddListener(Advance);
            for (int i=0;i<cards.Count;i++) _cards.Add(UiEntrance.Play(cards[i], .12f+i*.075f, .35f, 22));
            _label.text = Loc.Get("UI_EXCAVATION_SKIP");
            Update();
        }
        void Update()
        {
            foreach (var card in _cards) if (card != null && !card.Complete) return;
            Complete = true;
            if (_label != null) _label.text = Loc.Get("UI_EXCAVATION_CONTINUE");
            enabled = false;
        }
        public void Advance()
        {
            if (Complete) { _modal.Close(); return; }
            foreach (var card in _cards) if (card != null) card.Finish();
            Update();
        }
    }
}
