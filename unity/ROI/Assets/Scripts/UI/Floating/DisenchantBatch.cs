using System;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;

namespace Roi.UI
{
    // Only presentation selection and aggregation; native previews decide eligibility/yields.
    public sealed class DisenchantBatch
    {
        public sealed class Plan
        {
            public SimSession Session;
            public string Inventory;
            public CrestModel.Choice[] Choices;
            public CrestModel.Preview[] Rewards;
        }
        readonly SortedSet<int> _indices=new();
        SimSession _session;
        string _inventory;
        public int Count=>_indices.Count;
        public bool Contains(int index)=>_indices.Contains(index);
        public void Clear()=>_indices.Clear();
        public void AfterAutoFusion(SimSession session,int[] consumed)
        {
            if(session!=_session){Sync(session);return;}
            var old=MailWindow.RowsOfArray(_inventory??"[]");
            string inventory=session.QueryJson("inventory");var rows=MailWindow.RowsOfArray(inventory);
            var remapped=new SortedSet<int>();
            foreach(int i in _indices)
            {
                int next=i-consumed.Count(c=>c<i);
                if(!consumed.Contains(i)&&next>=0&&next<rows.Count&&rows[next]==old[i])remapped.Add(next);
            }
            _indices.Clear();foreach(int i in remapped)_indices.Add(i);_inventory=inventory;
        }
        public bool Sync(SimSession session)
        {
            string inventory=session?.QueryJson("inventory");
            if(_session==session&&_inventory==inventory)return false;
            bool stale=false;
            if(_session==session&&Count>0)
            {
                var old=MailWindow.RowsOfArray(_inventory??"[]");var next=MailWindow.RowsOfArray(inventory??"[]");
                var remapped=new SortedSet<int>();
                foreach(int index in _indices)
                {
                    string raw=old[index];var matches=next.Select((r,i)=>new{r,i}).Where(x=>x.r==raw).Select(x=>x.i).ToArray();
                    // Known auto-fusion removals are remapped explicitly. Unknown removals of
                    // identical rows without IDs are ambiguous: drop that selection safely.
                    if(matches.Length<old.Count(r=>r==raw)){stale=true;continue;}
                    int ordinal=old.Take(index).Count(r=>r==raw);remapped.Add(matches[ordinal]);
                }
                _indices.Clear();foreach(int i in remapped)_indices.Add(i);
            }
            else {stale=Count>0;Clear();}
            _session=session;_inventory=inventory;return stale;
        }
        public bool Toggle(SimSession session,int index,string renderedRow)
        {
            Sync(session);
            var rows=MailWindow.RowsOfArray(_inventory??"[]");
            if(index<0||index>=rows.Count||rows[index]!=renderedRow)return false;
            if(_indices.Remove(index))return true;
            if(CrestModel.Freeze(session,index,_inventory)==null)return false;
            _indices.Add(index);return true;
        }
        public void AutoSelect(SimSession session,int gradeMask)
        {
            Sync(session);Clear();
            var rows=MailWindow.RowsOfArray(_inventory??"[]");
            for(int i=0;i<rows.Count;i++)
            {
                var item=ForgeModel.Parse<ItemView>(rows[i]);
                if(item==null||item.grade_step<0||item.grade_step>=10||(gradeMask&(1<<item.grade_step))==0)continue;
                if(CrestModel.Freeze(session,i,_inventory)!=null)_indices.Add(i);
            }
        }
        public Plan Freeze(SimSession session)
        {
            Sync(session);
            if(Count==0||session==null)return null;
            var choices=_indices.Select(i=>CrestModel.Freeze(session,i,_inventory)).ToArray();
            if(choices.Any(c=>c==null||c.Inventory!=_inventory)){Clear();return null;}
            var rules=CrestModel.Read(session);if(rules?.crests==null)return null;
            var rewards=new List<CrestModel.Preview>();
            foreach(var group in choices.GroupBy(c=>c.Reward.material).OrderBy(g=>g.Key,StringComparer.Ordinal))
            {
                var p=group.First().Reward;
                var slot=rules.crests.FirstOrDefault(c=>c.material==p.material);if(slot==null)return null;
                ulong sum=0;foreach(var c in group)sum+=c.Reward.yield;
                // Display what the native saturating u32 purse can actually receive.
                ulong gain=Math.Min((ulong)uint.MaxValue-slot.held,sum);
                rewards.Add(new CrestModel.Preview{material=p.material,category=p.category,tier=p.tier,
                    shards_per_material=p.shards_per_material,yield=gain});
            }
            return new Plan{Session=session,Inventory=_inventory,Choices=choices,Rewards=rewards.ToArray()};
        }
        public static string Summary(Plan plan)=>plan==null?"":string.Join("\n",plan.Rewards.Select(CrestModel.Yield));
        public static VerifiedInputResult Apply(SimDriver driver,Plan plan)
        {
            if(plan?.Choices==null||plan.Choices.Length==0)return VerifiedInputResult.Refused;
            if(driver?.Session!=plan.Session||plan.Session.QueryJson("inventory")!=plan.Inventory)return VerifiedInputResult.Stale;
            foreach(var c in plan.Choices)
                if(plan.Session.QueryJson("disenchant_preview:"+c.Index)!=c.PreviewJson)return VerifiedInputResult.Stale;
            var before=CrestModel.Read(plan.Session);if(before?.crests==null)return VerifiedInputResult.Unavailable;
            var removed=new HashSet<int>(plan.Choices.Select(c=>c.Index));
            if(removed.Count!=plan.Choices.Length)return VerifiedInputResult.Refused;
            var expectedRows=MailWindow.RowsOfArray(plan.Inventory).Where((r,i)=>!removed.Contains(i)).ToArray();
            // Remove largest indices first so every subsequent index still identifies its item.
            var inputs=removed.OrderByDescending(i=>i).Select(i=>"{\"Disenchant\":{\"inv_idx\":"+i+"}}").ToArray();
            return driver.ApplyVerifiedInputs(plan.Session,inputs,copy=>
            {
                if(!MailWindow.RowsOfArray(copy.QueryJson("inventory")).SequenceEqual(expectedRows))return false;
                var after=CrestModel.Read(copy);if(after?.crests==null)return false;
                return plan.Rewards.All(p=>
                {
                    var start=before.crests.FirstOrDefault(s=>s.material==p.material);
                    var finish=after.crests.FirstOrDefault(s=>s.material==p.material);
                    return start!=null&&finish!=null&&finish.held==start.held+p.yield;
                });
            });
        }
    }
}
