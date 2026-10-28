using System;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;

namespace Roi.UI
{
    // A convenience recommendation, not a new equip rule. Native inputs decide legality and
    // actual placement; the verified sequence publishes only the completed loadout.
    public static class ExemplarAutoEquip
    {
        public static VerifiedInputResult Apply(SimDriver driver, long memberId, out int changed)
        {
            changed=0;
            var live=driver?.Session;
            if(live==null||!live.IsAlive)return VerifiedInputResult.Unavailable;
            var initial=ExemplarEquipmentActions.Read(live,memberId);
            if(!initial.Available)return VerifiedInputResult.Unavailable;
            var working=live.Clone();
            if(working==null)return VerifiedInputResult.Unavailable;
            var inputs=new List<string>();
            string expected;
            try
            {
                if(working.StateHash()!=live.StateHash())return VerifiedInputResult.Unavailable;
                // Ten slots plus accessory routing. Strict upgrades prevent oscillation.
                for(int pass=0;pass<20;pass++)
                {
                    var before=ExemplarEquipmentActions.Read(working,memberId);
                    var candidates=before.Inventory.Where(r=>ExemplarEquipmentActions.EquipIssue(before,r)==null)
                        .OrderByDescending(r=>r.Item,Comparer<ItemView>.Create(Compare)).ThenBy(r=>r.Index);
                    bool advanced=false;
                    foreach(var row in candidates)
                    {
                        // Avoid native copies for candidates that cannot improve either legal
                        // accessory destination. The host still decides which one is used.
                        string[] slots=row.Slot=="Ring"||row.Slot=="Necklace"
                            ?new[]{"Ring","Necklace"}:new[]{row.Slot};
                        if(!slots.Any(slot=>Compare(row.Item,before.Equipped.FirstOrDefault(r=>r.Slot==slot)?.Item)>0))continue;
                        string input="{\"EquipItem\":{\"entity_id\":"+memberId+",\"inventory_idx\":"+row.Index+"}}";
                        SimSession trial=working.Clone();
                        if(trial==null)return VerifiedInputResult.Unavailable;
                        try
                        {
                            if(!trial.ApplyInput(input)||!ExemplarEquipmentTransfer.Verify(trial,before,row,null))continue;
                            var after=ExemplarEquipmentActions.Read(trial,memberId);
                            var placed=after.Equipped[after.Equipped.Length-1];
                            var previous=before.Equipped.FirstOrDefault(r=>r.Slot==placed.Slot);
                            if(Compare(placed.Item,previous?.Item)<=0)continue;
                            working.Dispose();working=trial;trial=null;
                            inputs.Add(input);advanced=true;break;
                        }
                        finally { trial?.Dispose(); }
                    }
                    if(!advanced)break;
                }
                expected=ExemplarEquipmentActions.Read(working,memberId).CandidatesFingerprint;
            }
            finally { working.Dispose(); }
            if(inputs.Count==0)return VerifiedInputResult.Applied;
            // Recheck identities, all inventories and loadouts before publishing. A new drop or
            // automatic fusion must never redirect a planned inventory index to a different piece.
            if(!ReferenceEquals(driver.Session,live)
                ||ExemplarEquipmentActions.Read(live,memberId).CandidatesFingerprint!=initial.CandidatesFingerprint)
                return VerifiedInputResult.Stale;
            var result=driver.ApplyVerifiedInputs(live,inputs.ToArray(),copy=>
                ExemplarEquipmentActions.Read(copy,memberId).CandidatesFingerprint==expected);
            if(result==VerifiedInputResult.Applied)changed=inputs.Count;
            return result;
        }
        static int Compare(ItemView a,ItemView b)
        {
            if(a==null)return b==null?0:-1;
            if(b==null)return 1;
            int grade=a.grade_step.CompareTo(b.grade_step);if(grade!=0)return grade;
            long mainA=Math.Max((long)a.graded_atk,a.graded_matk)+a.graded_def;
            long mainB=Math.Max((long)b.graded_atk,b.graded_matk)+b.graded_def;
            int main=mainA.CompareTo(mainB);if(main!=0)return main;
            return a.item_level.CompareTo(b.item_level);
        }
    }
}
