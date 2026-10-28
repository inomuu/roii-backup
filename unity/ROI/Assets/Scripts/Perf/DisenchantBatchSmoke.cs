#if UNITY_EDITOR
using System;
using System.Linq;
using System.Reflection;
using Roi.Sim;
using Roi.UI;
using UnityEngine;

namespace Roi.Perf
{
    public static class DisenchantBatchSmoke
    {
        static void Require(bool ok,string why){if(!ok)throw new InvalidOperationException(why);}
        static void Grant(SimSession s,string id)=>s.Debug("{\"grant_item\":{\"def_id\":\""+id+"\"}}");
        public static string Run()
        {
            var go=new GameObject("BatchDisenchantSmoke");var driver=go.AddComponent<SimDriver>();driver.Paused=true;
            try
            {
                driver.Attach(SimSession.NewStarter(43,SimSession.LoadCatalog(),"zone_b1_1","batch test"));
                var s=driver.Session;
                for(int i=0;i<6;i++)Grant(s,"sword_s3");
                s.ApplyInput("{\"FuseGear\":{\"picks\":[0,1,2,3,4,5],\"server_roll\":0}}");
                Require(ForgeModel.Items(s.QueryJson("inventory")).Single().grade_step==1,"Fine fixture");
                Grant(s,"sword_s3");Grant(s,"helm_s3");Grant(s,"sword_s1");
                var draft=new DisenchantBatch();draft.AutoSelect(s,2);
                Require(draft.Count==1&&draft.Contains(0),"Exact Fine mask");
                draft.AutoSelect(s,1);Require(draft.Count==2&&draft.Contains(1)&&draft.Contains(2),"Common mask skips ineligible T1");
                draft.AutoSelect(s,0);Require(draft.Count==0,"Empty mask");
                var rows=MailWindow.RowsOfArray(s.QueryJson("inventory"));
                draft.Toggle(s,0,rows[0]);draft.Toggle(s,2,rows[2]);
                var plan=draft.Freeze(s);Require(plan.Choices.Length==2&&plan.Rewards.Length==2,"Aggregate two material types");
                Require(DisenchantBatch.Apply(driver,plan)==VerifiedInputResult.Applied,"Descending batch succeeds atomically");
                Require(MailWindow.RowsOfArray(s.QueryJson("inventory")).SequenceEqual(new[]{rows[1],rows[3]}),"Unselected items unchanged");
                draft.AutoSelect(s,1);plan=draft.Freeze(s);Grant(s,"sword_s3");
                string hash=s.StateHash();Require(DisenchantBatch.Apply(driver,plan)==VerifiedInputResult.Stale&&s.StateHash()==hash,"Stale batch consumes nothing");

                driver.Attach(SimSession.NewStarter(44,SimSession.LoadCatalog(),"zone_b1_1","reservation test"));s=driver.Session;
                for(int i=0;i<7;i++)Grant(s,"sword_s3");
                draft=new DisenchantBatch();rows=MailWindow.RowsOfArray(s.QueryJson("inventory"));draft.Toggle(s,2,rows[2]);
                var model=go.AddComponent<ForgeModel>();model.Bind(driver);
                model.AutoFusionBeforeBatch=()=>draft.Sync(s);model.AutoFusionReserved=(i,raw)=>draft.Contains(i);
                bool prepared=(bool)typeof(ForgeModel).GetMethod("PrepareAutoFusionBatch",BindingFlags.NonPublic|BindingFlags.Instance).Invoke(model,null);
                Require(prepared&&model.Gear.Count==6&&!model.Gear.Any(p=>p.Index==2),"Auto Fusion skips selected duplicate, uses other six");
                int[] consumed=model.Gear.Select(p=>p.Index).ToArray();
                s.ApplyInput("{\"FuseGear\":{\"picks\":["+string.Join(",",consumed)+"],\"server_roll\":0}}");
                draft.AfterAutoFusion(s,consumed);draft.Sync(s);
                Require(draft.Count==1&&draft.Contains(0),"Selection reindexes after Auto Fusion");
                Require(draft.Freeze(s).Choices.Single().Index==0,"Selected item still disenchantable");
                draft.Clear();Require(!draft.Contains(0),"Clear releases reservation");
                Require(ForgeModel.AutoFusionInterval==10f,"Ten second cadence");
                return "PASS: grade filters, ineligible exclusion, multi-material batch, exact survivors, stale atomic rejection, selected duplicate reservation/reindex, release, 10s interval.";
            }
            finally {UnityEngine.Object.DestroyImmediate(go);}
        }
    }
}
#endif
