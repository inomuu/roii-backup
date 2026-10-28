// MEMBER TABS — the strip along the top of an open screen, and a PICKER rather than a readout.
//
// Split from the combat bar on the user's call ("แต่ว่ามันควรจะเป็นคนละ UI กันนะ"). The two share a
// subject and nothing else. The combat bar is a control surface under the thumb; this is a header
// that answers one question a per-character screen cannot answer for itself: WHICH character.
//
// That question was already being answered wrongly. The Cards screen showed the lead's panel with
// no way to reach anyone else's — the same silent assumption a stat sheet and a skill page would
// each have made in turn. `PartySelection` holds the answer once, this strip is how it is set, and
// a screen subscribes rather than guessing.
//
// Deliberately SHORTER than the combat bar and deliberately quieter: a tab is not a card. Name and
// level, an accent tick on the chosen one, and no health — a bar here would be a number nobody is
// acting on, competing with the content the screen exists to show.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class MemberTabs : MonoBehaviour
    {
        const float TabW = 156f, TabGap = 8f, Pad = UiTheme.PadSmall;
        const float Interval = 0.5f;

        Roi.Sim.SimDriver _driver;
        RectTransform _root;
        readonly List<GameObject> _tabs = new();
        readonly List<long> _ids = new();
        string _membership = "";
        float _next;

        public static MemberTabs Create(Transform parent)
        {
            var go = new GameObject("MemberTabs", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var self = (RectTransform)go.transform;
            self.anchorMin = Vector2.zero; self.anchorMax = Vector2.one;
            self.offsetMin = Vector2.zero; self.offsetMax = Vector2.zero;
            var t = go.AddComponent<MemberTabs>();
            t.Build();
            return t;
        }

        void Build()
        {
            var bg = UiTheme.Flat(transform, "Tabs", UiTheme.Panel);
            _root = bg.rectTransform;
            _root.anchorMin = new Vector2(0f, 1f); _root.anchorMax = new Vector2(1f, 1f);
            _root.pivot = new Vector2(0.5f, 1f);
            _root.sizeDelta = new Vector2(0f, UiLayout.TabsH);

            var rule = UiTheme.Flat(_root, "Rule", UiTheme.Line);
            rule.raycastTarget = false;
            var rr = rule.rectTransform;
            rr.anchorMin = new Vector2(0f, 0f); rr.anchorMax = new Vector2(1f, 0f);
            rr.pivot = new Vector2(0.5f, 0f); rr.sizeDelta = new Vector2(0f, 1f);

            PartySelection.Changed += Repaint;
        }

        void OnDestroy() => PartySelection.Changed -= Repaint;

        /// Shown only while a screen is open. Hidden in combat, where the combat bar is the party's
        /// representation and this would be the same subject drawn twice.
        public void SetVisible(bool on)
        {
            if (gameObject.activeSelf != on) gameObject.SetActive(on);
            if (on) { _next = 0f; Refresh(); }
        }

        void Update()
        {
            // Twice a second is plenty: the membership only changes when the player changes it,
            // and nothing else on this strip moves.
            if (Time.unscaledTime < _next) return;
            _next = Time.unscaledTime + Interval;
            Refresh();
        }

        void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;

            var view = JsonUtility.FromJson<TabView>(session.ViewJson());
            var live = new List<TabEntity>();
            if (view != null && view.grid != null && view.grid.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet) live.Add(e);

            var fp = new System.Text.StringBuilder();
            foreach (var e in live) fp.Append(e.id).Append(':').Append(e.unit_def).Append(',');
            if (fp.ToString() == _membership) return;
            _membership = fp.ToString();

            _ids.Clear();
            foreach (var e in live) _ids.Add(e.id);
            // Resolve BEFORE building, so the tick lands on the tab that will actually exist: a
            // selection left pointing at a benched member would light nothing at all.
            PartySelection.Resolve(_ids);
            Rebuild(live);
        }

        void Rebuild(List<TabEntity> live)
        {
            foreach (var go in _tabs) Destroy(go);
            _tabs.Clear();
            for (int i = 0; i < live.Count; i++)
            {
                long id = live[i].id;
                var tab = UiTheme.Box(_root, "Tab" + i, UiTheme.Surface);
                UiTheme.Border(tab.rectTransform, UiTheme.Line);
                var btn = tab.gameObject.AddComponent<Button>();
                btn.targetGraphic = tab;
                btn.navigation = new Navigation { mode = Navigation.Mode.None };
                var colors = btn.colors;
                colors.highlightedColor = new Color(1.3f, 1.3f, 1.3f, 1f);
                colors.fadeDuration = 0.08f;
                btn.colors = colors;
                btn.onClick.AddListener(() => PartySelection.Select(id));

                var rt = tab.rectTransform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
                rt.pivot = new Vector2(0f, 0.5f);
                rt.sizeDelta = new Vector2(TabW, UiLayout.Touch);
                rt.anchoredPosition = new Vector2(Pad + i * (TabW + TabGap), 0f);

                var tick = UiTheme.Flat(rt, "Tick", UiTheme.Accent);
                tick.raycastTarget = false;
                var kr = tick.rectTransform;
                kr.anchorMin = kr.anchorMax = new Vector2(0f, 0.5f);
                kr.pivot = new Vector2(0f, 0.5f);
                kr.sizeDelta = new Vector2(2f, 16f);
                kr.anchoredPosition = new Vector2(6f, 0f);

                var name = UiTheme.Label(rt, "Name", GameText.UnitName(live[i].unit_def),
                                         UiTheme.BodySize, UiTheme.TextHigh, TextAlignmentOptions.Left);
                var nr = name.rectTransform;
                nr.anchorMin = Vector2.zero; nr.anchorMax = Vector2.one;
                nr.offsetMin = new Vector2(16f, 0f); nr.offsetMax = new Vector2(-46f, 0f);

                var lv = UiTheme.Label(rt, "Level",
                                       live[i].stats != null ? "Lv " + live[i].stats.level : "",
                                       UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Right);
                var lr = lv.rectTransform;
                lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
                lr.offsetMin = new Vector2(16f, 0f); lr.offsetMax = new Vector2(-12f, 0f);

                _tabs.Add(tab.gameObject);
            }
            int cap=PartySlotPurchase.Read(_driver?.Session)?.party_max??live.Count;
            for(int i=live.Count;i<cap;i++)
            {
                var rt=new GameObject("EmptySlot"+i,typeof(RectTransform)).GetComponent<RectTransform>();rt.SetParent(_root,false);
                rt.anchorMin=rt.anchorMax=rt.pivot=new Vector2(0,.5f);
                rt.sizeDelta=new Vector2(TabW,UiLayout.Touch);rt.anchoredPosition=new Vector2(Pad+i*(TabW+TabGap),0);
                PartySlotUnlock.Create(rt,i);_tabs.Add(rt.gameObject);
            }
            Repaint();
        }

        void Repaint()
        {
            long sel = PartySelection.SelectedId;
            for (int i = 0; i < _tabs.Count && i < _ids.Count; i++)
            {
                bool on = _ids[i] == sel;
                var img = _tabs[i].GetComponent<Image>();
                img.color = on ? UiTheme.SurfaceActive : UiTheme.Surface;
                var tick = _tabs[i].transform.Find("Tick");
                if (tick != null) tick.GetComponent<Image>().enabled = on;
                var border = _tabs[i].transform.Find("Border");
                if (border != null) border.GetComponent<Image>().color = on ? UiTheme.LineStrong : UiTheme.Line;
            }
        }

        [System.Serializable] class TabView { public TabGrid grid; }
        [System.Serializable] class TabGrid { public TabEntity[] entities; }
        [System.Serializable]
        class TabEntity
        {
            public long id; public string team, unit_def;
            public bool is_pet;
            public TabStats stats;
        }
        [System.Serializable] class TabStats { public int level; }
    }
}
