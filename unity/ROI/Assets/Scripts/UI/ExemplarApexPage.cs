using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using Roi.Sim;

namespace Roi.UI
{
    // Explicit character binding. Read the running host schema: older native builds expose one
    // post-cap pool, while tiered metadata exposes separate Core and post-cap pools.
    public sealed class ExemplarApexPage : MonoBehaviour
    {
        SimDriver _driver;
        SimSession _session;
        ExemplarReadModel.Member _member;
        StatsDto _state;
        BucketDto[] _buckets = Array.Empty<BucketDto>();
        readonly List<Card> _cards = new();
        RectTransform _content, _allocations;
        TextMeshProUGUI _name, _level, _coreLabel, _core, _apexLabel, _apex, _condition, _xp, _notice, _substatCondition;
        Image _xpFill;
        Button _refund;
        string _metadata = "";
        bool _tiered;

        public static ExemplarApexPage Create(Transform parent)
        {
            var rt = ExemplarUi.Rect(parent, "ApexPage", 0, 0, 1, 1);
            var page = rt.gameObject.AddComponent<ExemplarApexPage>();
            page.Build(); return page;
        }
        void Build()
        {
            // One management column leaves the entire portrait lane free of panels.
            var summary = ExemplarUi.Panel(transform, "Summary", 0, .035f, 1, .985f,
                Color.clear).rectTransform;
            GameMenuSurface.Add(summary);
            _name = ExemplarUi.Text(summary, "Character", "", 0, 0, 1, 1, 28, ExemplarUi.White);
            At(_name.rectTransform, .045f, .51f, 16, 36);
            _level = ExemplarUi.Text(summary, "ApexLevel", "", 0, 0, 1, 1, 22, ExemplarUi.Gold, TextAlignmentOptions.MidlineRight);
            At(_level.rectTransform, .54f, .955f, 16, 36);
            _coreLabel = ExemplarUi.Text(summary, "CoreLabel", Loc.Get("UI_EX_APEX_CORE_POINTS"), 0, 0, 1, 1, 17, ExemplarUi.Muted);
            At(_coreLabel.rectTransform, .045f, .49f, 62, 24);
            _core = ExemplarUi.Text(summary, "CorePoints", "—", 0, 0, 1, 1, 28, ExemplarUi.White, numeric:true);
            At(_core.rectTransform, .045f, .49f, 86, 32);
            _apexLabel = ExemplarUi.Text(summary, "ApexLabel", Loc.Get("UI_APEX_POINTS"), 0, 0, 1, 1, 17, ExemplarUi.Muted);
            _apex = ExemplarUi.Text(summary, "ApexPoints", "—", 0, 0, 1, 1, 28, ExemplarUi.White, numeric:true);
            var divider = ExemplarUi.Panel(summary, "IdentityDivider", 0, 0, 1, 1, ExemplarUi.Line);
            At(divider.rectTransform, .045f, .955f, 126, 1);
            _condition = ExemplarUi.Text(summary, "Condition", "", 0, 0, 1, 1, 17, ExemplarUi.Muted);
            At(_condition.rectTransform, .045f, .955f, 137, 44);
            _condition.textWrappingMode = TextWrappingModes.Normal;
            var track = ExemplarUi.Panel(summary, "XpTrack", 0, 0, 1, 1, ExemplarUi.Deep);
            At(track.rectTransform, .045f, .955f, 212, 3);
            _xpFill = ExemplarUi.Panel(track.transform, "Fill", 0, 0, 0, 1, ExemplarUi.Gold);
            _xp = ExemplarUi.Text(summary, "Xp", "", 0, 0, 1, 1, 16, ExemplarUi.Muted);
            At(_xp.rectTransform, .045f, .955f, 185, 24);
            _refund = ExemplarUi.Button(summary, "Refund", Loc.Get("UI_APEX_RESET"), .045f, 0, .955f, 0, AskRefund);
            _refund.GetComponent<RectTransform>().offsetMin = new Vector2(0, 14);
            _refund.GetComponent<RectTransform>().offsetMax = new Vector2(0, 58);
            _allocations = ExemplarUi.Rect(summary, "Allocations", .03f, 0, .975f, 1);
            _allocations.offsetMin = new Vector2(0, 74);
            _allocations.offsetMax = new Vector2(0, -194);
            _content = UiScroll.Create(_allocations, out var scroll); scroll.horizontal = false;
            _notice = ExemplarUi.Text(_allocations, "Unavailable", "", .025f, .80f, .98f, 1, 20, ExemplarUi.Muted);
            _notice.textWrappingMode = TextWrappingModes.Normal;
        }
        public void Show(ExemplarReadModel.Member member)
        {
            _member = member;
            if (_driver == null) _driver = UnityEngine.Object.FindFirstObjectByType<SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (!ReferenceEquals(session, _session)) { _session = session; _metadata = ""; }
            bool live = session != null && session.IsAlive;
            string metadata = live ? session.QueryJson("apex_buckets") : "[]";
            if (metadata != _metadata)
            {
                _metadata = metadata;
                var list = !string.IsNullOrWhiteSpace(metadata) && metadata.TrimStart()[0] == '['
                    ? Parse<BucketList>("{\"rows\":" + metadata + "}") : null;
                _buckets = list?.rows ?? Array.Empty<BucketDto>();
                _tiered = false;
                foreach (var bucket in _buckets)
                    if (bucket != null && !string.IsNullOrEmpty(bucket.tier)) { _tiered = true; break; }
                RebuildCards();
            }
            _state = live && member != null && !member.Benched ? ReadStats(session, member.Id) : null;
            Paint();
        }
        void Paint()
        {
            bool valid = _state != null;
            _name.text = _member == null ? Loc.Get("UI_EXEMPLAR_NO_CHARACTER") : GameText.UnitName(_member.UnitDef);
            _level.text = Loc.Get("UI_APEX_LEVEL") + "  " + (valid ? _state.apex_level.ToString("N0") : "—");
            _coreLabel.gameObject.SetActive(_tiered); _core.gameObject.SetActive(_tiered);
            _core.text = valid && _tiered ? _state.core_unspent.ToString("N0") : "—";
            At(_apexLabel.rectTransform, _tiered ? .535f : .045f, .955f, 62, 24);
            At(_apex.rectTransform, _tiered ? .535f : .045f, .955f, 86, 32);
            _apex.text = valid ? _state.apex_unspent.ToString("N0") : "—";
            _condition.text = !valid ? (_member?.Benched == true ? Loc.Get("UI_EXEMPLAR_BENCH_STATS") : Loc.Get("UI_EX_APEX_UNAVAILABLE"))
                : _state.level < _state.level_cap
                    ? Loc.Fmt("UI_EX_APEX_UNLOCK", _state.level_cap) + "\n" + Loc.Fmt("UI_EX_APEX_CURRENT_LEVEL", _state.level, _state.level_cap)
                    : Loc.Get("UI_APEX_HINT");
            bool atCap = valid && _state.level >= _state.level_cap;
            if (_substatCondition != null)
                _substatCondition.text = valid && !atCap ? Loc.Fmt("UI_EX_APEX_UNLOCK", _state.level_cap) : "";
            bool maxed = atCap && _state.apex_level_cap > 0 && _state.apex_level >= _state.apex_level_cap;
            bool xpAvailable = atCap && _state.apex_cost > 0;
            _xp.text = maxed ? Loc.Get("UI_EX_APEX_MAX") : xpAvailable
                ? Loc.Fmt("UI_EX_APEX_NEXT_XP", _state.xp.ToString("N0"), _state.apex_cost.ToString("N0")) : "";
            _xpFill.transform.parent.gameObject.SetActive(atCap && (xpAvailable || maxed));
            _allocations.offsetMax = new Vector2(0, atCap && (xpAvailable || maxed) ? -230 : -194);
            _xpFill.rectTransform.anchorMax = new Vector2(maxed ? 1 : xpAvailable ? Mathf.Clamp01((float)((double)_state.xp / _state.apex_cost)) : 0, 1);
            _notice.text = _cards.Count == 0 ? Loc.Get("UI_EX_APEX_UNAVAILABLE") : "";
            bool allocated = false;
            foreach (var card in _cards)
            {
                int i = card.Index; var b = _buckets[i];
                bool known = valid && i < _state.apex_alloc.Length;
                int points = known ? _state.apex_alloc[i] : 0;
                if (known && points > 0) allocated = true;
                card.Total.text = known ? Amount((long)points * b.per_pt, b.bp) : "—";
                card.Rank.text = known ? Loc.Fmt("UI_EX_APEX_ALLOCATION", points, _state.apex_bucket_cap) : "—";
                card.One.interactable = card.Five.interactable = known && CanSpend(_state, b, i);
                var actionInk=card.One.interactable?ExemplarUi.White:UiTheme.TextLow;
                card.One.GetComponentInChildren<TextMeshProUGUI>().color=actionInk;
                card.Five.GetComponentInChildren<TextMeshProUGUI>().color=actionInk;
                card.Surface.color = Color.clear;
                bool locked = valid && Tier(b) == "substat" && !atCap;
                card.One.gameObject.SetActive(!locked); card.Five.gameObject.SetActive(!locked);
                card.Lock.SetActive(locked);
            }
            _refund.interactable = valid && allocated;
            _refund.GetComponentInChildren<TextMeshProUGUI>().color=_refund.interactable?ExemplarUi.White:UiTheme.TextLow;
        }
        void RebuildCards()
        {
            foreach (Transform child in _content) Destroy(child.gameObject); _cards.Clear();
            _substatCondition = null;
            float y = 0;
            foreach (string tier in new[] { "core", "substat" })
            {
                int count = 0; foreach (var b in _buckets) if (b != null && Tier(b) == tier) count++;
                if (count == 0) continue;
                var heading = ExemplarUi.Text(_content, "Section_" + tier, Loc.Get(tier == "core" ? "UI_EX_APEX_CORE" : "UI_EX_APEX_SUBSTATS"), 0, 1, 1, 1, 24, ExemplarUi.Gold);
                At(heading.rectTransform, .025f, .98f, y, 38); y += 42;
                if (tier == "substat")
                {
                    _substatCondition = ExemplarUi.Text(_content, "SubstatCondition", "", 0, 1, 1, 1, 17, ExemplarUi.Muted);
                    At(_substatCondition.rectTransform, .025f, .98f, y, 26); y += 32;
                }
                int position = 0;
                for (int i = 0; i < _buckets.Length; i++)
                {
                    var b = _buckets[i]; if (b == null || Tier(b) != tier) continue;
                    int bucket = i; int row = position++;
                    var panel = ExemplarUi.Panel(_content, "Bucket_" + i, 0, 1, 1, 1, Color.clear);
                    At(panel.rectTransform, 0, 1, y + row * 96, 88);
                    panel.rectTransform.offsetMax += new Vector2(-4, 0);
                    var rt = panel.rectTransform;
                    ExemplarUi.Panel(rt, "BottomRule", .035f, .01f, .965f, .017f, new Color(.42f,.48f,.50f,.30f));
                    
                    ExemplarUi.Text(rt, "Name", BucketName(b), .035f, .65f, .57f, .98f, 21, ExemplarUi.White);
                    var total = ExemplarUi.Text(rt, "Total", "", .035f, .29f, .26f, .66f, 25, ExemplarUi.Gold);
                    ExemplarUi.Text(rt, "PerPoint", Loc.Fmt("UI_EX_APEX_PER_POINT", Amount(b.per_pt, b.bp)), .27f, .29f, .59f, .65f, 16, ExemplarUi.Muted, TextAlignmentOptions.MidlineRight);
                    var rank = ExemplarUi.Text(rt, "Allocated", "", .035f, .02f, .59f, .29f, 16, ExemplarUi.Muted);
                    var one = ExemplarUi.Button(rt, "One", Loc.Fmt("UI_EX_APEX_ALLOCATE", 1), .61f, .22f, .765f, .72f, () => Spend(bucket, 1));
                    var five = ExemplarUi.Button(rt, "Five", Loc.Fmt("UI_EX_APEX_ALLOCATE", 5), .80f, .22f, .955f, .72f, () => Spend(bucket, 5));
                    var locked = LockMark(rt);
                    _cards.Add(new Card { Index = i, Surface = panel, Total = total, Rank = rank, One = one, Five = five, Lock = locked });
                }
                y += count * 96 + 18;
            }
            UiScroll.Fit(_content, y);
        }
        // The section caption gives the exact host level gate once; each locked card keeps a clear lock mark.
        static GameObject LockMark(Transform parent)
        {
            var rt = ExemplarUi.Rect(parent, "Locked", .61f, .22f, .955f, .72f);
            var icon = ExemplarUi.Rect(rt, "Lock", .5f, .5f, .5f, .5f);
            icon.sizeDelta = new Vector2(18, 22);
            ExemplarUi.Panel(icon, "Body", 0, 0, 1, .58f, ExemplarUi.Muted);
            ExemplarUi.Panel(icon, "ShackleLeft", .18f, .52f, .3f, .92f, ExemplarUi.Muted);
            ExemplarUi.Panel(icon, "ShackleRight", .7f, .52f, .82f, .92f, ExemplarUi.Muted);
            ExemplarUi.Panel(icon, "ShackleTop", .18f, .84f, .82f, .96f, ExemplarUi.Muted);
            ExemplarUi.Panel(icon, "Keyhole", .44f, .16f, .56f, .38f, ExemplarUi.Deep);
            return rt.gameObject;
        }
        static void At(RectTransform rt, float x0, float x1, float y, float height)
        {
            rt.anchorMin = new Vector2(x0, 1); rt.anchorMax = new Vector2(x1, 1); rt.pivot = new Vector2(.5f, 1);
            rt.offsetMin = new Vector2(0, -y - height); rt.offsetMax = new Vector2(0, -y);
        }
        void Spend(int index, int count)
        {
            if (!Bound() || index < 0 || index >= _buckets.Length) return;
            var current = ReadStats(_session, _member.Id);
            if (current == null || !CanSpend(current, _buckets[index], index)) { _state = current; Paint(); return; }
            _driver.ApplyInput("{\"AllocateApex\":{\"entity_id\":" + _member.Id + ",\"bucket\":" + index + ",\"count\":" + count + "}}");
            Show(_member);
        }
        void AskRefund()
        {
            if (!Bound() || !_refund.interactable) return;
            long id = _member.Id; var session = _session;
            UiConfirm.Ask(Loc.Get("UI_APEX_RESET"), Loc.Fmt("UI_EX_APEX_REFUND_BODY", GameText.UnitName(_member.UnitDef)),
                Loc.Get("UI_EX_APEX_REFUND_ACTION"), () =>
                {
                    if (!Bound() || _member.Id != id || !ReferenceEquals(_session, session)) return;
                    if (ReadStats(session, id) == null) return;
                    _driver.ApplyInput("{\"ResetApex\":{\"entity_id\":" + id + "}}");
                    Show(_member);
                }, false);
        }
        bool Bound() => isActiveAndEnabled && _member != null && !_member.Benched && _driver != null
            && _session != null && _session.IsAlive && ReferenceEquals(_driver.Session, _session);
        // A missing tier is the legacy all-post-cap contract, not evidence of a Core pool.
        string Tier(BucketDto bucket) => _tiered ? bucket.tier : "substat";
        bool CanSpend(StatsDto s, BucketDto bucket, int index)
        {
            if (bucket == null || index >= s.apex_alloc.Length || s.apex_alloc[index] >= s.apex_bucket_cap) return false;
            return Tier(bucket) == "core" ? s.core_unspent > 0
                : Tier(bucket) == "substat" && s.level >= s.level_cap && s.apex_unspent > 0;
        }
        static StatsDto ReadStats(SimSession session, long id)
        {
            var s = Parse<StatsDto>(session.QueryJson("stats:" + id));
            return s != null && s.level > 0 && s.level_cap > 0 && s.apex_bucket_cap > 0 && s.apex_alloc != null ? s : null;
        }
        static T Parse<T>(string json) where T : class
        {
            if (string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '{') return null;
            try { return JsonUtility.FromJson<T>(json); } catch (ArgumentException) { return null; }
        }
        static string Amount(long amount, bool bp) => "+" + (bp ? (amount / 100m).ToString("0.##") + "%" : amount.ToString("N0"));
        static string BucketName(BucketDto b)
        {
            string key = b.name switch
            {
                "ATK" => "UI_APEX_ATK", "HP" => "UI_APEX_HP", "DEF" => "UI_APEX_DEF", "Crit" => "UI_APEX_CRIT",
                "Crit DMG" => "UI_APEX_CRITDMG", "Dodge" => "UI_APEX_DODGE", "DEF Pen" => "UI_APEX_DEFPEN",
                "Break DMG" => "UI_APEX_BREAKDMG", "Break Buildup" => "UI_APEX_BREAKBUILD", "Cooldown" => "UI_APEX_COOLDOWN",
                "Cast" => "UI_APEX_CAST", "Charge" => "UI_APEX_CHARGE", "Heal" => "UI_APEX_HEAL", _ => b.loc
            };
            return string.IsNullOrEmpty(key) ? b.name ?? "" : Loc.GetOr(key, b.name ?? key);
        }
        sealed class Card { public int Index; public Image Surface; public TextMeshProUGUI Total, Rank; public GameObject Lock; public Button One, Five; }
        [Serializable] sealed class BucketList { public BucketDto[] rows; }
        [Serializable] sealed class BucketDto { public string name, loc, tier; public int per_pt; public bool bp; }
        [Serializable] sealed class StatsDto
        {
            public int level, level_cap, apex_level, apex_level_cap, apex_bucket_cap;
            public long xp, apex_cost, core_unspent, apex_unspent;
            public int[] apex_alloc;
        }
    }
}
