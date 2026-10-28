#if UNITY_EDITOR
using System;
using Roi.Sim;
using Roi.UI;
using UnityEngine;

namespace Roi.Perf
{
    // Run outside Play mode: Roi.Perf.CrestUiSmoke.Run(). Disposable native state only;
    // never touches SessionBoot, player saves, online sessions or the active driver.
    public static class CrestUiSmoke
    {
        static void Require(bool ok,string message)
        { if(!ok)throw new InvalidOperationException(message); }

        public static string Run()
        {
            var go=new GameObject("CrestUiSmoke");
            var driver=go.AddComponent<SimDriver>();driver.Paused=true;
            try
            {
                driver.Attach(SimSession.NewStarter(42,SimSession.LoadCatalog(),"zone_b1_1","Crest UI test"));
                var session=driver.Session;
                Require(!PartySlotPurchase.CanBuy(PartySlotPurchase.Read(session)),"Insufficient Gil");
                session.Debug("{\"give_currency\":{\"gil\":1000000}}");
                for(int i=1;i<5;i++)
                {
                    var before=PartySlotPurchase.Read(session);
                    Require(before.slot_reward_tickets==10,"Host reward metadata");
                    Require(PartySlotPurchase.Buy(driver,session,before.party_slots,before.slot_cost)==VerifiedInputResult.Applied,"Party purchase");
                    Require(PartySlotPurchase.Buy(driver,session,before.party_slots,before.slot_cost)==VerifiedInputResult.Stale,"Duplicate click must not buy next slot");
                }
                Require(!PartySlotPurchase.CanBuy(PartySlotPurchase.Read(session)),"Maximum party slots");
                Require(CrestModel.Read(session).crests.Length==20,"Twenty host Crests");
                session.Debug("{\"grant_item\":{\"def_id\":\"sword_s1\"}}");
                Require(CrestModel.Freeze(session,LastIndex(session))==null,"T1 cannot disenchant");
                for(int i=0;i<10;i++)
                {
                    GrantSword(session);
                    var choice=CrestModel.Freeze(session,LastIndex(session));
                    Require(choice!=null,"Native T3 preview");
                    Require(CrestModel.Disenchant(driver,choice)==VerifiedInputResult.Applied,"Disenchant removes item and credits exact shards");
                }
                Require(CrestModel.Upgrade(driver,"crest_w3")==VerifiedInputResult.Applied,"Upgrade spends shards and increases level");
                GrantSword(session);
                var stale=CrestModel.Freeze(session,LastIndex(session));
                GrantSword(session);
                string hash=session.StateHash();
                Require(CrestModel.Disenchant(driver,stale)==VerifiedInputResult.Stale&&session.StateHash()==hash,"Inventory mutation must cancel destructive confirmation");
                Require(CrestModel.Amount(ulong.MaxValue,10).Contains("1,844,674,407,370,955,161"),"Large counters retain integer precision");
                return "PASS: party purchases/funds/max/duplicate; 20 Crests; T1 rejection; ten disenchant transactions; upgrade; stale selection; u64 formatting.";
            }
            finally { UnityEngine.Object.DestroyImmediate(go); }
        }

        static int LastIndex(SimSession s)=>MailWindow.RowsOfArray(s.QueryJson("inventory")).Count-1;
        static void GrantSword(SimSession s)=>s.Debug("{\"grant_item\":{\"def_id\":\"sword_s3\"}}");
    }
}
#endif
