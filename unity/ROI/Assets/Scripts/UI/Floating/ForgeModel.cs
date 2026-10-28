using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    // Shared across Battle/Companion: presentation choices only. Rules and results come from sim-host.
    public sealed class ForgeModel : MonoBehaviour
    {
        [Serializable] public sealed class Rules { public int inputs, ceiling_step, server_roll_from_step, blacksmith_level; }
        [Serializable] public sealed class GemSet { public string id, group; public int weight; public string[] defs; }
        [Serializable] public sealed class GemRules { public int merge, ceiling_step, crafter_level; public int[] fail_bp_now_by_grade; public GemSet[] sets; }
        [Serializable] public sealed class Preview
        { public int tier, tier_up_bp, grade_step, fail_bp; public ulong payout_gil, attempt; public bool needs_server_roll; public string key; }
        [Serializable] public sealed class Receipt { public string kind, status; public ulong gil, attempt; public ItemView item; }
        public sealed class Pick { public int Index; public string Raw; public ItemView Item; }
        public SimDriver Driver { get; private set; }
        public Rules GearRules { get; private set; }
        public GemRules GemsRules { get; private set; }
        public readonly List<Pick> Gear = new();
        public readonly List<string> Gems = new();
        public ItemView[] Bag = Array.Empty<ItemView>(), HeldGems = Array.Empty<ItemView>();
        public bool GemMode { get; private set; }
        public bool Busy { get; private set; }
        public bool AutoFusionEnabled { get; private set; }
        public bool AutoFusionRunning { get; private set; }
        public const float AutoFusionInterval=10f;
        public bool PublishingAutoFusionResult { get; private set; }
        public Func<int,string,bool> AutoFusionReserved;
        public Action AutoFusionBeforeBatch;
        public Action<int[]> AutoFusionConsumed;
        public Func<bool> AutoFusionBlocked;
        public string StatusKey { get; private set; } = "UI_FORGE_PICK";
        public Preview Quote { get; private set; }
        public event Action Changed;
        public event Action<Receipt> Completed;
        SimSession _session;
        Coroutine _autoFusionRoutine;
        List<string> _raw = new();
        public int Required => GemMode ? GemsRules?.merge ?? 0 : GearRules?.inputs ?? 0;
        public int Count => GemMode ? Gems.Count : Gear.Count;
        public bool CanSubmit => !Busy && Required > 0 && Count == Required && (GemMode || Quote != null);
        public bool DestroysGems => !GemMode && Gear.Any(p => p.Item.gems?.Length > 0);
        public string DraftKey => GemMode ? string.Join(",", Gems) : string.Join("|", Gear.Select(p => p.Index + ":" + p.Raw));
        public static T Parse<T>(string s) where T : class { try { return JsonUtility.FromJson<T>(s); } catch { return null; } }
        public static ItemView[] Items(string s) => Parse<ItemList>("{\"items\":" + s + "}")?.items ?? Array.Empty<ItemView>();
        public void Bind(SimDriver driver) { Driver = driver; Refresh(); }
        public bool Refresh()
        {
            var session = Driver != null ? Driver.Session : null;
            if (!ReferenceEquals(session, _session))
            {
                _session = session; SetAutoFusion(false); Gear.Clear(); Gems.Clear(); Quote = null;
                GearRules = session == null ? null : Parse<Rules>(session.QueryJson("fuse_rules"));
                GemsRules = session == null ? null : Parse<GemRules>(session.QueryJson("gem_rules"));
            }
            if (session == null || !session.IsAlive) { Bag = HeldGems = Array.Empty<ItemView>(); return false; }
            // Crafter levels and live odds change after each operation, not just session changes.
            GearRules = Parse<Rules>(session.QueryJson("fuse_rules"));
            GemsRules = Parse<GemRules>(session.QueryJson("gem_rules"));
            var json = session.QueryJson("inventory"); Bag = Items(json); _raw = MailWindow.RowsOfArray(json);
            // Gems live in the inventory. Bound and unbound stacks may share a definition;
            // present their combined available quantity without changing the bag projection.
            HeldGems = Bag.Where(IsGem).GroupBy(m => m.def_id).Select(group =>
            {
                var item = Parse<ItemView>(JsonUtility.ToJson(group.First()));
                item.quantity = group.Sum(m => m.quantity);
                return item;
            }).ToArray();
            if (Gear.Any(p => p.Index >= _raw.Count || _raw[p.Index] != p.Raw))
            { Gear.Clear(); StatusKey = "UI_FORGE_STALE"; }
            if (Gems.GroupBy(id => id).Any(g => HeldGems.FirstOrDefault(m => m.def_id == g.Key)?.quantity < g.Count()
                || !HeldGems.Any(m => m.def_id == g.Key)))
            { Gems.Clear(); StatusKey = "UI_FORGE_STALE"; }
            Quote = Gear.Count == GearRules?.inputs ? Parse<Preview>(session.QueryJson("fuse_preview:" + string.Join(",", Gear.Select(p => p.Index)))) : null;
            if (Quote != null && string.IsNullOrEmpty(Quote.key)) Quote = null;
            return true;
        }
        public void Mode(bool gems)
        {
            if (Busy) return;
            GemMode = gems; Refresh(); Changed?.Invoke();
        }
        public bool IsGem(ItemView item) => item != null && GemsRules?.sets != null && GemsRules.sets.Any(s => s.defs != null && Array.IndexOf(s.defs, item.def_id) >= 0);
        public int GemGrade(string id)
        { foreach (var set in GemsRules?.sets ?? Array.Empty<GemSet>()) { int i = Array.IndexOf(set.defs ?? Array.Empty<string>(), id); if (i >= 0) return i; } return -1; }
        public bool Eligible(ItemView item, bool auto = false) => item != null && (item.kind == "Weapon" || item.kind == "Armor")
            && !item.bound && !item.unique && item.equipped_by == 0 && item.grade_step < (GearRules?.ceiling_step ?? 0)
            && (!auto || item.gems == null || item.gems.Length == 0);
        public bool CanPickGear(int index) => !Busy&&!GemMode&&index>=0&&index<Bag.Length
            &&Eligible(Bag[index])&&Gear.Count<Required&&!Gear.Any(p=>p.Index==index)
            &&(Gear.Count==0||Gear[0].Item.grade_step==Bag[index].grade_step);
        public bool CanPickGem(string id)
        {
            var item=HeldGems.FirstOrDefault(m=>m.def_id==id);int grade=GemGrade(id);
            return !Busy&&GemMode&&item!=null&&grade>=0&&grade<(GemsRules?.ceiling_step??0)
                &&Gems.Count<Required&&Gems.Count(g=>g==id)<item.quantity
                &&(Gems.Count==0||GemGrade(Gems[0])==grade);
        }
        public void PickGear(int index)
        {
            if (Busy || GemMode || !Refresh()) return;
            var existing = Gear.Find(p => p.Index == index);
            if (existing != null) Gear.Remove(existing);
            else if (CanPickGear(index))
                Gear.Add(new Pick { Index = index, Item = Bag[index], Raw = _raw[index] });
            else { StatusKey = "UI_FORGE_SAME_GRADE"; Changed?.Invoke(); return; }
            StatusKey = "UI_FORGE_PICK"; Refresh(); Changed?.Invoke();
        }
        public void PickGem(string id)
        {
            if (Busy || !GemMode || !Refresh()) return;
            if (!CanPickGem(id))
            { StatusKey = "UI_FORGE_SAME_GRADE"; Changed?.Invoke(); return; }
            Gems.Add(id); StatusKey = "UI_FORGE_PICK"; Changed?.Invoke();
        }
        public void Remove(int index)
        { if (Busy) return; if (GemMode) { if (index < Gems.Count) Gems.RemoveAt(index); } else if (index < Gear.Count) Gear.RemoveAt(index); Refresh(); Changed?.Invoke(); }
        public void Clear() { if (Busy) return; if (GemMode) Gems.Clear(); else Gear.Clear(); Refresh(); Changed?.Invoke(); }
        // Draft inputs belong to the open Forge panel. Leaving that panel must not leave
        // stale green selection markers on the main inventory when it is opened again.
        public void ResetDraft()
        {
            if (Busy) return;
            Gear.Clear(); Gems.Clear(); Quote=null;
            StatusKey=AutoFusionEnabled?"UI_FORGE_AUTO_FUSION_WAIT":"UI_FORGE_PICK";
        }
        public void ToggleAutoFusion()
        {
            if(Busy||GemMode)return;
            SetAutoFusion(!AutoFusionEnabled);
            Gear.Clear(); Quote=null;
            StatusKey=AutoFusionEnabled?"UI_FORGE_AUTO_FUSION_WAIT":"UI_FORGE_PICK";
            Refresh(); Changed?.Invoke();
        }
        void SetAutoFusion(bool enabled)
        {
            AutoFusionEnabled=enabled;
            if(_autoFusionRoutine!=null){StopCoroutine(_autoFusionRoutine);_autoFusionRoutine=null;}
            AutoFusionRunning=false;
            if(enabled&&!GemMode)_autoFusionRoutine=StartCoroutine(AutoFusionLoop());
        }
        IEnumerator AutoFusionLoop()
        {
            AutoFusionRunning=true;
            while(AutoFusionEnabled)
            {
                yield return new WaitForSecondsRealtime(AutoFusionInterval);
                if(!AutoFusionEnabled)break;
                if(Busy||AutoFusionBlocked?.Invoke()==true||!Refresh())continue;
                if(!PrepareAutoFusionBatch())
                { StatusKey="UI_FORGE_AUTO_FUSION_WAIT"; Changed?.Invoke();continue; }
                var session=_session;
                bool applied;
                PublishingAutoFusionResult=true;
                try {applied=ApplyGear(session,"0");}
                finally {PublishingAutoFusionResult=false;}
                if(!applied)
                { AutoFusionEnabled=false; break; }
            }
            AutoFusionRunning=false;_autoFusionRoutine=null;
            if(AutoFusionEnabled){Gear.Clear();Quote=null;StatusKey="UI_FORGE_AUTO_FUSION_WAIT";Refresh();}
            Changed?.Invoke();
        }
        bool PrepareAutoFusionBatch()
        {
            if(GearRules==null||GearRules.inputs!=6)return false;
            AutoFusionBeforeBatch?.Invoke();
            var group=Bag.Select((item,index)=>new{item,index})
                .Where(x=>x.item!=null&&x.item.grade_step<=1&&Eligible(x.item,true)&&AutoFusionReserved?.Invoke(x.index,_raw[x.index])!=true)
                .GroupBy(x=>x.item.grade_step).OrderBy(g=>g.Key).FirstOrDefault(g=>g.Count()>=Required);
            if(group==null)return false;
            Gear.Clear();
            foreach(var pick in group.Take(Required))Gear.Add(new Pick{Index=pick.index,Item=pick.item,Raw=_raw[pick.index]});
            Refresh();
            if(CanSubmit&&Quote!=null&&!Quote.needs_server_roll)return true;
            Gear.Clear(); Quote=null; Refresh(); return false;
        }
        public void AutoFill()
        {
            if (Busy || !Refresh()) return;
            if (GemMode)
            {
                int grade = Gems.Count > 0 ? GemGrade(Gems[0]) : HeldGems.GroupBy(m => GemGrade(m.def_id))
                    .Where(g => g.Key < GemsRules.ceiling_step && g.Sum(m => (long)m.quantity) >= Required).OrderBy(g => g.Key).Select(g => g.Key).DefaultIfEmpty(-1).First();
                foreach (var item in HeldGems.Where(m => GemGrade(m.def_id) == grade))
                    while (Gems.Count < Required && Gems.Count(g => g == item.def_id) < item.quantity) Gems.Add(item.def_id);
            }
            else
            {
                int grade = Gear.Count > 0 ? Gear[0].Item.grade_step : Bag.Where(m => Eligible(m, true)).GroupBy(m => m.grade_step)
                    .Where(g => g.Count() >= Required).OrderBy(g => g.Key).Select(g => g.Key).DefaultIfEmpty(-1).First();
                for (int i = 0; i < Bag.Length && Gear.Count < Required; i++)
                    if (Eligible(Bag[i], true) && Bag[i].grade_step == grade && !Gear.Any(p => p.Index == i))
                        Gear.Add(new Pick { Index = i, Item = Bag[i], Raw = _raw[i] });
            }
            StatusKey = Count == Required ? "UI_FORGE_READY" : "UI_FORGE_SHORT"; Refresh(); Changed?.Invoke();
        }
        public void Submit(string confirmedDraft)
        {
            Refresh(); if (!CanSubmit || DraftKey != confirmedDraft) { StatusKey = "UI_FORGE_STALE"; Changed?.Invoke(); return; }
            var session = _session; var draft = DraftKey; var quote = Quote;
            if (GemMode) { Apply(session, "{\"FuseGems\":{\"gems\":[" + string.Join(",", Gems.Select(QuoteString)) + "]}}"); return; }
            if (!quote.needs_server_roll) { ApplyGear(session, "0"); return; }
            var online = Roi.Net.OnlineService.Instance;
            if (online == null || !online.LoggedIn) { StatusKey = "UI_FORGE_LOGIN"; Changed?.Invoke(); return; }
            Busy = true; StatusKey = "UI_FORGE_WAITING"; Changed?.Invoke();
            StartCoroutine(online.Client.FuseRoll(quote.key, quote.attempt, (ok, nonce, reason) =>
            {
                Busy = false; Refresh();
                if (!ReferenceEquals(session, _session) || draft != DraftKey || Quote?.key != quote.key || Quote?.attempt != quote.attempt)
                { StatusKey = "UI_FORGE_STALE"; Changed?.Invoke(); return; }
                if (!ok || !ulong.TryParse(nonce, out var n) || n == 0) { StatusKey = "UI_FORGE_NETWORK"; Changed?.Invoke(); return; }
                ApplyGear(session, n.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }));
        }
        bool ApplyGear(SimSession session, string nonce, bool suppressResult=false) => Apply(session,
            "{\"FuseGear\":{\"picks\":[" + string.Join(",", Gear.Select(p => p.Index)) + "],\"server_roll\":" + nonce + "}}",suppressResult);
        bool Apply(SimSession session, string input, bool suppressResult=false)
        {
            // Preflight accepts both consumed outcomes. It MUST NOT reroll a failure.
            var applied = Driver.ApplyVerifiedInputs(session, new[] { input }, s =>
            { var r = Parse<Receipt>(s.QueryJson("forge_receipt")); return r?.status == "success" || r?.status == "failed"; });
            var receipt = applied == VerifiedInputResult.Applied ? Parse<Receipt>(session.QueryJson("forge_receipt")) : null;
            if (receipt == null) { StatusKey = "UI_FORGE_STALE"; Refresh(); Changed?.Invoke(); return false; }
            if(PublishingAutoFusionResult&&!GemMode)AutoFusionConsumed?.Invoke(Gear.Select(p=>p.Index).ToArray());
            if (GemMode) Gems.Clear(); else Gear.Clear();
            StatusKey = suppressResult?"UI_FORGE_AUTO_FUSION_WAIT":"UI_FORGE_PICK"; Refresh(); Changed?.Invoke(); if(!suppressResult)Completed?.Invoke(receipt);
            return true;
        }
        public static string QuoteString(string s) => "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
    }
}
