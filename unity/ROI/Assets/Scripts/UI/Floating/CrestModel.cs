using System;
using System.Linq;
using Roi.Sim;

namespace Roi.UI
{
    // Amounts are host-owned shard counters, never floating-point currency.
    public static class CrestModel
    {
        [Serializable] public sealed class Rules { public ulong shards_per_material=1; public Slot[] crests; }
        [Serializable] public sealed class Slot
        {
            public string id,category,material,text,text_per_level;
            public int tier,level,max_level;
            public ulong held,next_cost;
        }
        [Serializable] public sealed class Preview
        { public string material,category;public int tier;public ulong yield,shards_per_material=1; }
        public sealed class Choice
        {
            public SimSession Session; public int Index; public string Inventory,PreviewJson;
            public Preview Reward;
        }
        public static Rules Read(SimSession s)=>s==null?null:ForgeModel.Parse<Rules>(s.QueryJson("crest_rules"));
        public static bool CanUpgrade(Slot s)=>s!=null&&s.level<s.max_level&&s.next_cost>0&&s.held>=s.next_cost;
        public static string Material(string category,int tier)=>Loc.Fmt("MAT_"+category,tier);
        public static string Amount(ulong shards,ulong per)
        {
            per=Math.Max(1UL,per);
            if(shards>0&&shards<per)return UiNumber.Exact(shards)+"/"+UiNumber.Exact(per);
            return shards%per==0?UiNumber.Exact(shards/per)
                :Loc.Fmt("UI_CREST_FRACTION",UiNumber.Exact(shards/per),UiNumber.Exact(shards%per),UiNumber.Exact(per));
        }
        public static VerifiedInputResult Upgrade(SimDriver driver,string id)
        {
            var session=driver?.Session;var s=Read(session)?.crests?.FirstOrDefault(x=>x.id==id);
            if(!CanUpgrade(s))return VerifiedInputResult.Refused;
            return driver.ApplyVerifiedInputs(session,new[]{"{\"UpgradeCrest\":{\"crest_id\":"+ForgeModel.QuoteString(id)+"}}"},copy=>
            {
                var after=Read(copy)?.crests?.FirstOrDefault(x=>x.id==id);
                return after!=null&&after.level==s.level+1&&after.held==s.held-s.next_cost;
            });
        }
        public static Choice Freeze(SimSession session,int index,string inventory=null)
        {
            if(session==null||index<0)return null;
            string raw=session.QueryJson("disenchant_preview:"+index);
            var p=ForgeModel.Parse<Preview>(raw);
            return p==null||string.IsNullOrEmpty(p.material)||p.yield==0?null:
                new Choice{Session=session,Index=index,Inventory=inventory??session.QueryJson("inventory"),PreviewJson=raw,Reward=p};
        }
        public static string Yield(Preview p)=>p==null?Loc.Get("UI_DISENCHANT_UNAVAILABLE"):
            Loc.Fmt("UI_DISENCHANT_YIELD",Material(p.category,p.tier),Amount(p.yield,p.shards_per_material));
        public static VerifiedInputResult Disenchant(SimDriver driver,Choice choice)
        {
            if(choice==null||driver?.Session!=choice.Session)return VerifiedInputResult.Stale;
            var session=choice.Session;
            // Auto-fusion can move indices while a confirmation is open. Never destroy its replacement.
            if(session.QueryJson("inventory")!=choice.Inventory||session.QueryJson("disenchant_preview:"+choice.Index)!=choice.PreviewJson)
                return VerifiedInputResult.Stale;
            var before=Read(session)?.crests?.FirstOrDefault(x=>x.material==choice.Reward.material);
            if(before==null)return VerifiedInputResult.Unavailable;
            // Native crest_mats is a saturating u32 purse; formatting still accepts u64.
            ulong expected=Math.Min(uint.MaxValue,before.held+choice.Reward.yield);
            return driver.ApplyVerifiedInputs(session,new[]{"{\"Disenchant\":{\"inv_idx\":"+choice.Index+"}}"},copy=>
            {
                var after=Read(copy)?.crests?.FirstOrDefault(x=>x.material==choice.Reward.material);
                return after!=null&&after.held==expected&&copy.QueryJson("inventory")!=choice.Inventory;
            });
        }
    }
}
