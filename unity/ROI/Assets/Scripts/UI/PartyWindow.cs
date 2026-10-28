// THE PARTY — who is deployed, who is on the bench, and who can still be recruited. Ported from
// godot/views/party_view.gd, which is the same three sections in the same order.
//
// The screen the port had been missing: everything else so far shows what the party OWNS, and
// this is the first one that shows the party.
//
// NO DRAG (user, 2026-09-07 — mobile is a planned platform). Moving a character between the field
// and the bench is a TAP on that character's own button, never a drag between two lists. That is
// also what the Godot original does, so this is parity and not a compromise.
//
// THREE READS, ONE WRITE PATH:
//   deployed  = the per-tick view's `grid.entities`, filtered to the player's team
//   bench     = query "roster"     — owned characters with no entity, so no per-entity list can
//               carry them
//   slots     = query "party_meta" (name, gil, party_slots, slot_cost, cap) — characters come from gacha
// Every action is an `InputEvent` through `ApplyInput`, which is what makes it replay-safe: the
// server verifies a save by replaying its inputs, so a mutation that skipped the log would make
// the save unverifiable. There is no "party API" and there must not be one.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class PartyWindow : MonoBehaviour
    {
        const float HeadH = 44f, SectionH = 30f, RowH = 96f, RecruitH = 40f;
        /// The width a recruit button WANTS. The real width is derived from it and the window's.
        const float RecruitW = 210f;

        Roi.Sim.SimDriver _driver;
        RectTransform _body, _content;
        TextMeshProUGUI _name, _gil;
        CombatHudPortrait _portrait;
        readonly List<GameObject> _built = new();
        string _fingerprint = "";
        string _slotNotice="";

        public static PartyWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<PartyWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            _body = body;
            float pad = UiTheme.PadLarge;

            // The party's own NAME, not the window's title. The title bar already says PARTY; this
            // is the thing the player chose and the only place it is shown.
            _name = UiTheme.Label(body, "PartyName", "", UiTheme.HeadSize,
                                  UiTheme.TextHigh, TextAlignmentOptions.Left);
            var nr = _name.rectTransform;
            nr.anchorMin = new Vector2(0f, 1f); nr.anchorMax = new Vector2(1f, 1f);
            nr.pivot = new Vector2(0.5f, 1f);
            nr.offsetMin = new Vector2(pad, -HeadH); nr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            // Gil in the accent, because on this screen it is the number a decision depends on.
            _gil = UiTheme.Label(body, "Gil", "", UiTheme.HeadSize,
                                 UiTheme.Accent, TextAlignmentOptions.Right);
            _gil.rectTransform.anchorMin = nr.anchorMin; _gil.rectTransform.anchorMax = nr.anchorMax;
            _gil.rectTransform.pivot = nr.pivot;
            _gil.rectTransform.offsetMin = nr.offsetMin; _gil.rectTransform.offsetMax = nr.offsetMax;

            var scrollHost = new GameObject("Scroll", typeof(RectTransform));
            scrollHost.transform.SetParent(body, false);
            var host = (RectTransform)scrollHost.transform;
            host.anchorMin = Vector2.zero; host.anchorMax = new Vector2(1f,1);
            host.offsetMin = new Vector2(pad, UiTheme.Pad);
            host.offsetMax = new Vector2(-pad, -(HeadH + UiTheme.Gap));
            _content = UiScroll.Create(host, out _);
            var portraitGo=new GameObject("PartyIllustration",typeof(RectTransform));portraitGo.transform.SetParent(body,false);
            var pr=(RectTransform)portraitGo.transform;pr.anchorMin=new Vector2(.70f,.12f);pr.anchorMax=new Vector2(.99f,.93f);pr.offsetMin=pr.offsetMax=Vector2.zero;
            _portrait=portraitGo.AddComponent<CombatHudPortrait>();_portrait.Crop=new Rect(0,0,1,1);_portrait.PreserveAspect=true;_portrait.raycastTarget=false;
        }

        /// Fetched when the window opens, and rebuilt only when the party actually CHANGES — the
        /// same fingerprint rule the item grid uses, and for the same reason: a live HP value ticks
        /// ten times a second, and rebuilding a button under the pointer eats the click.
        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _name.text = Loc.Get("UI_PARTY_NO_SESSION"); _gil.text = ""; return; }

            var meta = JsonUtility.FromJson<PartyMeta>(session.QueryJson("party_meta")) ?? new PartyMeta();
            var bench = JsonUtility.FromJson<RosterList>("{\"rows\":" + session.QueryJson("roster") + "}");
            var classes = JsonUtility.FromJson<ClassList>("{\"rows\":" + session.QueryJson("units") + "}");
            var view = JsonUtility.FromJson<PartyViewDTO>(session.ViewJson());

            var deployed = new List<PartyEntity>();
            if (view != null && view.grid != null && view.grid.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet && e.alive) deployed.Add(e);

            _name.text = string.IsNullOrEmpty(meta.name) ? "Party" : meta.name;
            _gil.text = Money(meta.gil) + " Gil";

            var benchRows = bench != null && bench.rows != null ? bench.rows : new RosterRow[0];
            var classRows = classes != null && classes.rows != null ? classes.rows : new ClassRow[0];
            var engrams = JsonUtility.FromJson<EngramsDTO>(session.QueryJson("engrams")) ?? new EngramsDTO();

            var fp = new System.Text.StringBuilder();
            foreach (var e in deployed) fp.Append(e.id).Append(':').Append(e.unit_def).Append(',');
            fp.Append('|');
            foreach (var r in benchRows) fp.Append(r.id).Append(':').Append(r.unit_def).Append(':').Append(r.refund_xp).Append(',');
            fp.Append('|').Append(meta.party_slots).Append('|').Append(meta.gil).Append('|').Append(classRows.Length).Append('|');
            if (engrams.owned != null) foreach (var n in engrams.owned) fp.Append(n).Append(',');
            fp.Append(engrams.ceiling_level);
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();

            Rebuild(meta, deployed, benchRows, classRows, engrams);
        }

        void Rebuild(PartyMeta meta, List<PartyEntity> deployed, RosterRow[] bench, ClassRow[] classes,
                     EngramsDTO engrams)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            float y = 0f;
            _portrait.gameObject.SetActive(false);
            int cap = meta.party_max > 0 ? meta.party_max : 5;

            Section(ref y, "Deployed", $"{deployed.Count} / {cap}");
            float formationTop=y;
            for(int i=0;i<deployed.Count;i++)
            {
                var e=deployed[i];long id=e.id;
                FormationMember(i,formationTop,e,deployed.Count>1?
                    (System.Action)(()=>Send($"{{\"BenchCharacter\":{{\"char_id\":{id}}}}}")):null);
            }
            y=formationTop+Mathf.Ceil(deployed.Count/3f)*420f;

            y += UiTheme.Pad;
            Section(ref y, "Bench", bench.Length.ToString());
            Note(ref y, EngramLine(engrams));
            if (bench.Length == 0) Note(ref y, "Nobody is benched.");
            bool full = deployed.Count >= meta.party_slots;
            foreach (var r in bench)
            {
                long id = r.id;
                var card = Member(ref y, r.unit_def, GameText.UnitName(r.unit_def), r.level, null, 0, 0,
                       full ? null : "Deploy",
                       () => Send($"{{\"DeployCharacter\":{{\"char_id\":{id}}}}}"));
                RefundButtons(card, r, full);
            }
            if (full && bench.Length > 0) Note(ref y, $"The field is full at {cap}.");

            y += UiTheme.Pad;
            // PARTY SLOTS (sim 0.0.413): Gil buys the field's SIZE, four times and cheaply; the
            // characters themselves come from gacha, so there is no class list to buy from any
            // more. `slot_cost` is the sim's price for the next slot (-1 = all bought).
            bool slotsLeft = meta.slot_cost >= 0;
            Section(ref y, "Party slots", $"{Mathf.Max(meta.party_slots, deployed.Count)} / {cap}");
            if (slotsLeft)
            {
                bool afford = meta.gil >= (ulong)meta.slot_cost;
                var btn = HudBar.MakeButton(_content, Loc.Fmt("UI_PARTY_SLOT_UNLOCK",Money((ulong)meta.slot_cost)), "",
                                            _content.rect.width, RecruitH, BuySlot);
                btn.interactable = afford;
                if (!afford) btn.GetComponentInChildren<TextMeshProUGUI>().color = UiTheme.TextLow;
                var rt = btn.GetComponent<RectTransform>();
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.anchoredPosition = new Vector2(0f, -y);
                _built.Add(btn.gameObject);
                y += RecruitH + UiTheme.Gap;
                Note(ref y,Loc.Fmt("UI_PARTY_SLOT_REWARD",meta.slot_reward_tickets));
                if (!afford) Note(ref y, "Not enough Gil for the next slot.");
            }
            else Note(ref y, "Every slot is open. New members come from Summon.");

            if(!string.IsNullOrEmpty(_slotNotice))Note(ref y,_slotNotice);
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        void FormationMember(int index,float top,PartyEntity member,System.Action bench)
        {
            var card=ExemplarUi.Rect(_content,"Formation_"+member.id,index%3/3f,1,(index%3+1)/3f,1);
            card.pivot=new Vector2(.5f,1);
            card.offsetMin=new Vector2(14,-top-(index/3+1)*420+18);
            card.offsetMax=new Vector2(-14,-top-index/3*420);
            _built.Add(card.gameObject);
            ExemplarUi.Panel(card,"PortraitGround",.03f,.22f,.97f,.98f,UiTheme.WithAlpha(UiTheme.Surface,.45f));
            var number=ExemplarUi.Text(card,"Position",(index+1).ToString("00"),0,.66f,.30f,1,64,UiTheme.WithAlpha(UiTheme.TextMid,.23f));
            number.raycastTarget=false;
            var art=ExemplarUi.Rect(card,"Portrait",.06f,.22f,.94f,.97f);
            var texture=ExemplarPortrait.Illustration(member.unit_def);
            if(texture!=null)
            {
                var portrait=art.gameObject.AddComponent<CombatHudPortrait>();portrait.SetTexture(texture);
                portrait.PreserveAspect=true;portrait.raycastTarget=false;
            }
            else
            {
                var avatar=ExemplarPortrait.Avatar(member.unit_def);
                if(avatar!=null){var image=art.gameObject.AddComponent<Image>();image.sprite=avatar;image.preserveAspect=true;image.raycastTarget=false;}
                else ExemplarUi.Icon(art,"Exemplars",.25f,.28f,.75f,.72f,UiTheme.WithAlpha(UiTheme.Accent,.45f));
            }
            ExemplarUi.Panel(card,"PortraitRule",.03f,.218f,.97f,.221f,UiTheme.LineStrong);
            var name=ExemplarUi.Text(card,"Name",GameText.UnitName(member.unit_def),.04f,.11f,.68f,.21f,20,UiTheme.TextHigh);
            var level=ExemplarUi.Text(card,"Level",Loc.Fmt("UI_COMMON_LEVEL_SHORT",member.stats!=null?member.stats.level:0),.69f,.11f,.97f,.21f,20,UiTheme.Accent,TextAlignmentOptions.Right);
            if(bench!=null)
            {
                var button=HudBar.MakeGhostButton(card,Loc.Get("UI_PARTY_BENCH_ACTION"),120,UiLayout.Touch,bench,UiTheme.CaptionSize);
                var r=(RectTransform)button.transform;r.anchorMin=r.anchorMax=new Vector2(.97f,0);r.pivot=new Vector2(1,0);r.anchoredPosition=Vector2.zero;
            }
        }

        // --- pieces --------------------------------------------------------------------------

        void Section(ref float y, string title, string right)
        {
            var head = UiTheme.Caps(_content, "Head_" + title, title, UiTheme.CaptionSize,
                                    UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(head.rectTransform, y, SectionH);
            _built.Add(head.gameObject);
            if (!string.IsNullOrEmpty(right))
            {
                var r = UiTheme.Label(_content, "HeadR_" + title, right, UiTheme.CaptionSize,
                                      UiTheme.TextLow, TextAlignmentOptions.Right);
                Place(r.rectTransform, y, SectionH);
                _built.Add(r.gameObject);
            }
            y += SectionH;
        }

        /// One character. `element` null and `maxHp` 0 mean a benched one: no entity, so no health
        /// and no element to show — and an HP bar drawn at zero would read as a dead character
        /// rather than as an absent one.
        RectTransform Member(ref float y, string unitDef, string className, int level, string element,
                    long hp, long maxHp, string action, System.Action onAction)
        {
            var card = UiTheme.Box(_content, "Member_" + className, UiTheme.Surface);
            UiTheme.Border(card.rectTransform, UiTheme.Line);
            Place(card.rectTransform, y, RowH);
            _built.Add(card.gameObject);

            var avatarGo=new GameObject("Portrait",typeof(RectTransform),typeof(Image));avatarGo.transform.SetParent(card.transform,false);
            var avatar=avatarGo.GetComponent<Image>();avatar.sprite=ExemplarPortrait.Avatar(unitDef);avatar.preserveAspect=true;avatar.raycastTarget=false;
            var ar=avatar.rectTransform;ar.anchorMin=ar.anchorMax=new Vector2(0,.5f);ar.pivot=new Vector2(0,.5f);ar.sizeDelta=new Vector2(64,76);ar.anchoredPosition=new Vector2(12,0);
            if(unitDef=="seraphine") {
                avatar.enabled=false;
                var faceGo=new GameObject("Face",typeof(RectTransform));faceGo.transform.SetParent(avatarGo.transform,false);
                var face=faceGo.AddComponent<CombatHudPortrait>();face.SetTexture(Resources.Load<Texture2D>("CombatPortraits/Seraphine"));face.raycastTarget=false;
                face.rectTransform.anchorMin=Vector2.zero;face.rectTransform.anchorMax=Vector2.one;face.rectTransform.offsetMin=face.rectTransform.offsetMax=Vector2.zero;
            }
            // Name FIRST and level after it. The class is what the player is looking for in a
            // list of five; the level is context, and leading with it made every row start with
            // the same two characters.
            var nameT = UiTheme.Label(card.rectTransform, "Name", className, UiTheme.BodySize,
                                      UiTheme.TextHigh, TextAlignmentOptions.Left);
            var nr = nameT.rectTransform;
            nr.anchorMin = nr.anchorMax = new Vector2(0f, 0.5f);
            nr.pivot = new Vector2(0f, 0.5f);
            nr.sizeDelta = new Vector2(150f, 20f);
            nr.anchoredPosition = new Vector2(88f, 17f);

            var lv = UiTheme.Label(card.rectTransform, "Level", Loc.Fmt("UI_COMMON_LEVEL_SHORT", level), UiTheme.CaptionSize,
                                   UiTheme.TextMid, TextAlignmentOptions.Left);
            var lr = lv.rectTransform;
            lr.anchorMin = lr.anchorMax = new Vector2(0f, 0.5f);
            lr.pivot = new Vector2(0f, 0.5f);
            lr.sizeDelta = new Vector2(70f, 18f);
            lr.anchoredPosition = new Vector2(88f, -13f);

            if (!string.IsNullOrEmpty(element))
            {
                var el = UiTheme.Caps(card.rectTransform, "Element", GameText.ElementName(element), 12f,
                                      UiTheme.TextLow, TextAlignmentOptions.Right);
                var er = el.rectTransform;
                er.anchorMin = er.anchorMax = new Vector2(1f, 0.5f);
                er.pivot = new Vector2(1f, 0.5f);
                er.sizeDelta = new Vector2(120f, 22f);
                er.anchoredPosition = new Vector2(-(UiTheme.Pad + 108f), 26f);
            }
            if (maxHp > 0)
            {
                var track = UiTheme.Box(card.rectTransform, "HpTrack", new Color(0f, 0f, 0f, 0.45f), 4);
                track.raycastTarget = false;
                var tr = track.rectTransform;
                tr.anchorMin = tr.anchorMax = new Vector2(1f, 0.5f);
                tr.pivot = new Vector2(1f, 0.5f);
                tr.sizeDelta = new Vector2(120f, 5f);
                tr.anchoredPosition = new Vector2(-(UiTheme.Pad + 108f), -8f);

                var fill = UiTheme.Box(track.rectTransform, "HpFill", new Color(0.42f, 0.72f, 0.45f), 4);
                fill.raycastTarget = false;
                var fr = fill.rectTransform;
                fr.anchorMin = new Vector2(0f, 0f); fr.anchorMax = new Vector2(0f, 1f);
                fr.pivot = new Vector2(0f, 0.5f);
                fr.sizeDelta = new Vector2(120f * Mathf.Clamp01(hp / (float)maxHp), 0f);
                fr.anchoredPosition = Vector2.zero;
                // The number above the bar: current / max.
                var hpText = UiTheme.Label(card.rectTransform, "HpText",
                                           System.Math.Max(hp, 0).ToString("N0") + " / " + maxHp.ToString("N0"),
                                           UiTheme.CaptionSize, UiTheme.TextMid, TextAlignmentOptions.Right);
                var hr = hpText.rectTransform;
                hr.anchorMin = hr.anchorMax = new Vector2(1f, 0.5f);
                hr.pivot = new Vector2(1f, 0.5f);
                hr.sizeDelta = new Vector2(160f, 24f);
                hr.anchoredPosition = new Vector2(-(UiTheme.Pad + 108f), 6f);
            }

            if (!string.IsNullOrEmpty(action))
            {
                var btn = HudBar.MakeGhostButton(card.rectTransform, action, 92f, 30f, onAction,
                                                 UiTheme.CaptionSize);
                var br = btn.GetComponent<RectTransform>();
                br.anchorMin = br.anchorMax = new Vector2(1f, 0.5f);
                br.pivot = new Vector2(1f, 0.5f);
                br.anchoredPosition = new Vector2(-UiTheme.PadSmall, 0f);
                UiTheme.Border(br, UiTheme.Line);
            }
            y += RowH + UiTheme.Gap;
            return card.rectTransform;
        }

        // --- XP refund (0.0.349) ----------------------------------------------------------------

        /// "Engrams  1k ×3  10k ×0  100k ×2  1M ×1  ·  ceiling Lv 35" — the reserve per tier from
        /// the sim's own tier table, and how far a benched unit may be poured.
        static string EngramLine(EngramsDTO e)
        {
            var sb = new System.Text.StringBuilder("Engrams  ");
            int n = e.owned != null && e.tiers != null ? Mathf.Min(e.owned.Length, e.tiers.Length) : 0;
            for (int i = 0; i < n; i++) sb.Append(Xp(e.tiers[i])).Append(" ×").Append(e.owned[i]).Append("  ");
            sb.Append("·  ceiling Lv ").Append(e.ceiling_level);
            return sb.ToString();
        }

        static string Xp(long v)
        {
            if (v >= 1_000_000) return (v % 1_000_000 == 0) ? (v / 1_000_000) + "M" : (v / 1_000_000f).ToString("0.0") + "M";
            if (v >= 1_000) return (v % 1_000 == 0) ? (v / 1_000) + "k" : (v / 1_000f).ToString("0.0") + "k";
            return v.ToString();
        }

        /// Per benched unit, left of Deploy: REFUND when it has XP to give back, FILL when the sim's
        /// preview would actually spend something. Each opens a confirm carrying the sim's own
        /// figures — the XP refunded, or the resulting level and the XP wasted — so what the player
        /// agrees to is what the sim will do; the commit re-derives the same plan.
        void RefundButtons(RectTransform card, RosterRow r, bool full)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            long id = r.id;
            float x = -(UiTheme.PadSmall + (full ? 0f : 92f + UiTheme.Gap));
            if (r.refund_xp > 0)
            {
                var b = HudBar.MakeGhostButton(card, "Refund", 80f, 30f, () =>
                    UiConfirm.Ask("Refund XP",
                        $"Return this unit to Lv1 and bank {Xp(r.refund_xp)} XP as Engrams.\n"
                        + "Gear, skills and the Potential Board stay.",
                        "Refund", () => Send($"{{\"RefundUnitXp\":{{\"unit_id\":{id}}}}}"), danger: false),
                    UiTheme.CaptionSize);
                Right(b, x);
                x -= 80f + UiTheme.Gap;
            }
            var pv = JsonUtility.FromJson<PreviewDTO>(session.QueryJson("engram_preview:" + id));
            bool spends = false;
            if (pv != null && pv.plan != null) foreach (var n in pv.plan) if (n > 0) spends = true;
            if (spends)
            {
                string body = $"Spend Engrams to bring this unit to Lv {pv.resulting_level}.";
                if (pv.xp_wasted > 0) body += $"\n{Xp(pv.xp_wasted)} XP would be wasted at the ceiling.";
                var b = HudBar.MakeGhostButton(card, "Fill", 80f, 30f, () =>
                    UiConfirm.Ask("Fill to max", body, "Fill",
                        () => Send($"{{\"ApplyEngramsToMax\":{{\"unit_id\":{id}}}}}"),
                        danger: pv.xp_wasted > 0),
                    UiTheme.CaptionSize);
                Right(b, x);
            }
        }

        static void Right(Button b, float x)
        {
            var br = b.GetComponent<RectTransform>();
            br.anchorMin = br.anchorMax = new Vector2(1f, 0.5f);
            br.pivot = new Vector2(1f, 0.5f);
            br.anchoredPosition = new Vector2(x, 0f);
            UiTheme.Border(br, UiTheme.Line);
        }

        void Note(ref float y, string text)
        {
            var t = UiTheme.Label(_content, "Note", text, UiTheme.CaptionSize,
                                  UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 26f);
            _built.Add(t.gameObject);
            y += 26f;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        /// Apply and re-read. The sim answers the next frame either way; forcing the refresh here
        /// is what makes the button feel like it did something rather than like it was ignored.
        void Send(string eventJson)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            session.ApplyInput(eventJson);
            _fingerprint = "";
            Refresh();
        }

        void BuySlot()
        {
            var session=_driver?.Session;var m=PartySlotPurchase.Read(session);if(m==null)return;
            var result=PartySlotPurchase.Buy(_driver,session,m.party_slots,m.slot_cost);
            _slotNotice=result==Roi.Sim.VerifiedInputResult.Applied?Loc.Fmt("UI_PARTY_SLOT_BOUGHT",m.slot_reward_tickets):Loc.Get("UI_TRAIN_STALE");
            _fingerprint="";Refresh();
        }

        static string Money(ulong v) => v.ToString("N0");

        // --- the shapes this screen reads ----------------------------------------------------

        [System.Serializable] class PartyViewDTO { public GridDTO grid; }
        [System.Serializable] class GridDTO { public PartyEntity[] entities; }
        [System.Serializable]
        class PartyEntity
        {
            public long id; public string team, unit_def, element;
            public bool is_pet, alive;
            public long hp, max_hp, eff_max_hp;
            public StatsDTO stats;
        }
        [System.Serializable] class StatsDTO { public int level; }
        [System.Serializable] class RosterRow { public long id; public string unit_def; public int level, apex_level; public long refund_xp; }
        // XP refund (0.0.349): the Engram reserve and what fill-to-max would do — both the sim's.
        [System.Serializable] class EngramsDTO { public long[] owned; public long[] tiers; public int ceiling_level, ceiling_apex; }
        [System.Serializable] class PreviewDTO { public long[] plan; public int resulting_level, resulting_apex; public long xp_wasted; }
        [System.Serializable] class RosterList { public RosterRow[] rows; }
        [System.Serializable] class ClassRow { public string id; }
        [System.Serializable] class ClassList { public ClassRow[] rows; }
        [System.Serializable]
        class PartyMeta
        {
            public string name; public ulong gil; public long slot_cost = -1; public int owned, party_size, party_max, party_slots, slot_reward_tickets;
        }
    }
}
