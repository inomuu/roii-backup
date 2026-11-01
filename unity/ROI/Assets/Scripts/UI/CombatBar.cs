// Combat party readout: transparent busts, a thin HP rule and a circular Ultimate reservoir.
// Values come from the sim; interpolation is presentation only. Membership controls rebuilding,
// and the small skill row retains its cooldowns and hover descriptions.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class CombatBar : MonoBehaviour
    {
        const float CardW = 156f, CardGap = 24f, Margin = 32f, CardH = 148f;
        const float Interval = .25f;
        static readonly Color Cyan = new Color(.40f, .94f, .96f);
        Roi.Sim.SimDriver _driver;
        RectTransform _root;
        readonly List<Member> _cards = new();
        string _membership = "";
        float _next, _layoutWidth = -1f;
        static Texture2D _seraphineArt;

#if UNITY_EDITOR
        int _previewCount;
        // Layout-only clones. No input events, party changes or save mutations are made.
        public void PreviewMemberCountForEditor(int count)
        {
            _previewCount = Mathf.Clamp(count, 0, 5);
            _membership = ""; _next = 0f;
            Refresh();
        }
#endif

        sealed class Member
        {
            public RectTransform Root, Fill;
            public TextMeshProUGUI Hp;
            public CombatHudPortrait Illustration;
            public Image SpritePortrait, HealthImage;
            public CombatUltimateGauge Ultimate;
            public bool Ready, Down;
            public CombatHudCrest Crest;
            public string PortraitId;
            public float HealthRatio, TargetHealth, HealthVelocity, UltimateRatio, TargetUltimate, UltimateVelocity;
            public bool Seeded;
            public CombatSkillRow Skills;
        }

        public static CombatBar Create(Transform parent)
        {
            var go = new GameObject("CombatBar", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var self = (RectTransform)go.transform;
            self.anchorMin = Vector2.zero; self.anchorMax = Vector2.one;
            self.offsetMin = self.offsetMax = Vector2.zero;
            var bar = go.AddComponent<CombatBar>();
            var root = new GameObject("Bar", typeof(RectTransform));
            root.transform.SetParent(go.transform, false);
            bar._root = (RectTransform)root.transform;
            bar._root.anchorMin = Vector2.zero; bar._root.anchorMax = new Vector2(1f, 0f);
            bar._root.pivot = new Vector2(.5f, 0f);
            bar._root.sizeDelta = new Vector2(0f, CardH);
            bar._root.anchoredPosition = new Vector2(0f, 16f);
            return bar;
        }

        public void SetVisible(bool on)
        {
            if (gameObject.activeSelf != on) gameObject.SetActive(on);
        }

        void Update()
        {
            AnimateBars();
            if (!Mathf.Approximately(_layoutWidth, _root.rect.width)) LayoutCards();
            if (Time.unscaledTime < _next) return;
            _next = Time.unscaledTime + Interval;
            Refresh();
        }

        void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            var view = JsonUtility.FromJson<RosterView>(session.ViewJson());
            var live = new List<RosterEntity>();
            if (view?.grid?.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet) live.Add(e);
#if UNITY_EDITOR
            if (_previewCount > 0 && live.Count > 0)
            {
                if (live.Count > _previewCount) live.RemoveRange(_previewCount, live.Count - _previewCount);
                while (live.Count < _previewCount) live.Add(live[0]);
            }
#endif
            var fingerprint = new System.Text.StringBuilder();
            foreach (var e in live) fingerprint.Append(e.id).Append(':').Append(e.unit_def).Append(',');
            if (fingerprint.ToString() != _membership)
            {
                _membership = fingerprint.ToString();
                Rebuild(live.Count);
            }
            for (int i = 0; i < _cards.Count && i < live.Count; i++)
            {
                Paint(_cards[i], live[i]);
                _cards[i].Skills.Refresh(session, live[i].id);
            }
        }

        void Rebuild(int count)
        {
            foreach (var card in _cards) Destroy(card.Root.gameObject);
            _cards.Clear();
            int total=PartySlotPurchase.Read(_driver?.Session)?.party_max??count;
            for (int i = 0; i < Mathf.Max(count,total); i++)
            {
                var go = new GameObject("Member" + i, typeof(RectTransform));
                go.transform.SetParent(_root, false);
                var rt = (RectTransform)go.transform;
                Place(rt, 0, 0, CardW, CardH);
                var m = new Member { Root = rt };
                if(i>=count)
                {
                    var empty=new GameObject("EmptySlot",typeof(RectTransform));empty.transform.SetParent(rt,false);
                    Place((RectTransform)empty.transform,0,30,CardW,95);
                    PartySlotUnlock.Create((RectTransform)empty.transform,i);
                    _cards.Add(m);continue;
                }

                // The source PNG is already transparent. Crop to the head/shoulders, preserving
                // aspect, and feather the side cuts so flowing hair does not become a square tile.
                var art = new GameObject("Illustration", typeof(RectTransform), typeof(CombatHudPortrait));
                art.transform.SetParent(rt, false);
                m.Illustration = art.GetComponent<CombatHudPortrait>();
                m.Illustration.raycastTarget = false;
                Place((RectTransform)art.transform, -26f, 36f, 152f, 108f);
                m.SpritePortrait = Flat(rt, "SpritePortrait", Color.white, -10, 37, 96, 99);
                m.SpritePortrait.preserveAspect = true;

                // Large reservoir overlapping the shoulder. The reference fills it UP from the
                // bottom; the outline remains circular at every charge level.
                var gauge = new GameObject("Ultimate", typeof(RectTransform), typeof(CombatUltimateGauge));
                gauge.transform.SetParent(rt, false);
                m.Ultimate = gauge.GetComponent<CombatUltimateGauge>();
                m.Ultimate.raycastTarget = false;
                Place((RectTransform)gauge.transform, 88, 51, 72, 72);
                var emblem = new GameObject("UltimateCrest", typeof(RectTransform), typeof(CombatHudCrest));
                emblem.transform.SetParent(rt, false);
                m.Crest = emblem.GetComponent<CombatHudCrest>();
                m.Crest.color = new Color(.88f, .95f, 1f, .30f);
                m.Crest.raycastTarget = false;
                Place((RectTransform)emblem.transform, 99, 62, 50, 50);

                // WuWa idiom: the slot number sits in a chamfered plate, not a gold coin medallion —
                // the cut corner is this UI's one shape language, and a circular medallion is the
                // generic fantasy-mobile frame already rejected elsewhere (design-system MASTER.md).
                Badge(rt, "SlotRim", UiTheme.Accent, 140, 105, 25);
                Badge(rt, "SlotBackground", UiTheme.Panel, 141, 106, 23);
                var slot = UiTheme.Label(rt, "Slot", (i + 1).ToString(), 16,
                                        new Color(.96f, .90f, .68f), TextAlignmentOptions.Center);
                Place(slot.rectTransform, 141, 106, 23, 23);
                slot.raycastTarget = false;

                // Two fine rules provide contrast over both light grass and dark environments.
                Flat(rt, "HpShadow", new Color(0f, 0f, 0f, .72f), -1, 28, CardW + 2, 9);
                Flat(rt, "HpBorder", new Color(.81f, .94f, .96f, .8f), 0, 30, CardW, 6);
                var track = Flat(rt, "Track", new Color(.02f, .09f, .11f, 1f), 1, 31, CardW - 2, 4);
                m.HealthImage = Flat(track.transform, "Fill", Cyan, 0, 0, CardW - 2, 4);
                m.Fill = m.HealthImage.rectTransform;
                var shine = Flat(m.Fill, "Highlight", new Color(1, 1, 1, .45f), 0, 3, 0, 1);
                shine.rectTransform.anchorMin = new Vector2(0, 0);
                shine.rectTransform.anchorMax = new Vector2(1, 0);
                m.Hp = UiTheme.Label(rt, "Hp", "", 16, Color.white, TextAlignmentOptions.Right);
                var hpFont = Resources.Load<TMP_FontAsset>("HomeUI/HomeHeading");
                if (hpFont != null)
                {
                    m.Hp.font = hpFont;
                    m.Hp.fontSharedMaterial = hpFont.material;
                }
                m.Hp.fontStyle = FontStyles.Normal; // Native semibold, without synthetic bold.
                m.Hp.characterSpacing = .5f;
                m.Hp.enableAutoSizing = true; m.Hp.fontSizeMin = 13; m.Hp.fontSizeMax = 19;
                m.Hp.textWrappingMode = TextWrappingModes.NoWrap;
                Place(m.Hp.rectTransform, 78, 35, CardW - 78, 24);
                var shadow = m.Hp.gameObject.AddComponent<Shadow>();
                shadow.effectColor = new Color(0, 0, 0, .9f);
                shadow.effectDistance = new Vector2(1, -1);
                m.Hp.raycastTarget = false;

                m.Skills = go.AddComponent<CombatSkillRow>();
                m.Skills.PlaceBelowHealth();
                _cards.Add(m);
            }
            LayoutCards();
        }

        void LayoutCards()
        {
            _layoutWidth = _root.rect.width;
            float total = _cards.Count * CardW + Mathf.Max(0, _cards.Count - 1) * CardGap;
            float scale = Mathf.Min(1f, Mathf.Max(1f, _layoutWidth - Margin * 2) / Mathf.Max(1f, total));
            for (int i = 0; i < _cards.Count; i++)
            {
                var rt = _cards[i].Root;
                rt.localScale = Vector3.one * scale;
                rt.anchoredPosition = new Vector2(Margin + i * (CardW + CardGap) * scale, 0);
            }
        }

        static void Place(RectTransform rt, float x, float y, float w, float h)
        {
            rt.anchorMin = rt.anchorMax = Vector2.zero; rt.pivot = Vector2.zero;
            rt.anchoredPosition = new Vector2(x, y); rt.sizeDelta = new Vector2(w, h);
        }

        static Image Flat(Transform parent, string name, Color color, float x, float y, float w, float h)
        {
            var image = UiTheme.Flat(parent, name, color); image.raycastTarget = false;
            Place(image.rectTransform, x, y, w, h);
            return image;
        }

        // WuWa idiom: the slot badge is the same chamfered plate as a card fill, 9-sliced so the
        // corner cut stays crisp at any size instead of a per-pixel circular alpha falloff.
        static Image Badge(Transform parent, string name, Color color, float x, float y, float size)
        {
            var image = Flat(parent, name, color, x, y, size, size);
            image.sprite = UiTheme.CutFill(8);
            image.type = Image.Type.Sliced;
            return image;
        }

        void AnimateBars()
        {
            foreach (var m in _cards)
            {
                if (!m.Seeded) continue;
                m.HealthRatio = Mathf.SmoothDamp(m.HealthRatio, m.TargetHealth, ref m.HealthVelocity,
                                                  .16f, Mathf.Infinity, Time.unscaledDeltaTime);
                m.UltimateRatio = Mathf.SmoothDamp(m.UltimateRatio, m.TargetUltimate, ref m.UltimateVelocity,
                                                    .18f, Mathf.Infinity, Time.unscaledDeltaTime);
                m.Fill.sizeDelta = new Vector2((CardW - 2) * m.HealthRatio, 4f);
                m.Ultimate.SetState(m.UltimateRatio, m.Ready, m.Down);
            }
        }

        void Paint(Member m, RosterEntity e)
        {
            if (m.PortraitId != e.unit_def)
            {
                m.PortraitId = e.unit_def;
                bool authored = e.unit_def == "seraphine";
                m.Illustration.gameObject.SetActive(authored);
                m.SpritePortrait.gameObject.SetActive(!authored);
                if (authored)
                {
                    _seraphineArt ??= Resources.Load<Texture2D>("CombatPortraits/Seraphine");
                    m.Illustration.SetTexture(_seraphineArt);
                }
                else m.SpritePortrait.sprite = ExemplarPortrait.Avatar(e.unit_def);
            }
            long max = e.eff_max_hp > 0 ? e.eff_max_hp : e.max_hp;
            m.TargetHealth = max > 0 ? Mathf.Clamp01(e.hp / (float)max) : 0f;
            m.TargetUltimate = e.eff_lb_max > 0 ? Mathf.Clamp01(e.limit_break / (float)e.eff_lb_max) : 0f;
            if (!m.Seeded)
            {
                m.HealthRatio = m.TargetHealth; m.UltimateRatio = m.TargetUltimate; m.Seeded = true;
            }
            ulong hp = (ulong)System.Math.Max(0L, e.hp);
            m.Hp.text = hp < 1000000UL ? UiNumber.Exact(hp) : UiNumber.Compact(hp);
            bool down = !e.alive || e.hp <= 0;
            bool ready = e.eff_lb_max > 0 && e.limit_break >= e.eff_lb_max && !down;
            m.Ready = ready; m.Down = down;
            m.HealthImage.color = down ? new Color(.35f, .40f, .44f)
                : m.TargetHealth < .3f ? new Color(.99f, .44f, .42f) : Cyan;
            var portraitColor = down ? new Color(.5f, .55f, .6f, .5f) : Color.white;
            m.Illustration.color = portraitColor; m.SpritePortrait.color = portraitColor;
            m.Crest.color = new Color(1f, 1f, 1f, ready ? .92f : .34f);
        }

        [System.Serializable] class RosterView { public RosterGrid grid; }
        [System.Serializable] class RosterGrid { public RosterEntity[] entities; }
        [System.Serializable] class RosterEntity
        {
            public long id; public string team, unit_def;
            public bool is_pet, alive;
            public long hp, max_hp, eff_max_hp;
            public int limit_break, eff_lb_max;
        }
    }
}
