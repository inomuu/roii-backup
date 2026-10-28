using System.Linq;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class FloatingInventoryHost
    {
        readonly DisenchantBatch _disenchantBatch=new();
        void AutoFusionConsumed(int[] indices)
        {
            _disenchantBatch.AfterAutoFusion(Model.Driver.Session,indices);
            if(InspectedOwner==0&&InspectedIndex>=0)
            {
                if(indices.Contains(InspectedIndex)){Inspected=null;InspectedIndex=-1;InspectedRaw=null;}
                else InspectedIndex-=indices.Count(i=>i<InspectedIndex);
            }
        }
        internal void ToggleDisenchant(int index,string raw)
        {
            if(!ItemsAvailable||ActiveSide!=SideDisenchant)return;
            bool changed=_disenchantBatch.Toggle(Model.Driver.Session,index,raw);
            if(!changed)
            {
                _autoEquipMessage=Loc.Get("UI_DISENCHANT_UNAVAILABLE");_autoEquipMessageUntil=Time.unscaledTime+3;
            }
            else
            {
                _autoEquipMessageUntil=0;
                // The batch owns its reservation. A removed entry must not stay protected as
                // the single inspected item from the same click.
                if(!_disenchantBatch.Contains(index)){Inspected=null;InspectedIndex=-1;InspectedRaw=null;}
            }
            RefreshNow();
        }
        internal void AutoSelectDisenchant(int mask)
        {
            if(!ItemsAvailable||ActiveSide!=SideDisenchant)return;
            _disenchantBatch.AutoSelect(Model.Driver.Session,mask);
            Inspected=null;InspectedIndex=-1;InspectedRaw=null;_autoEquipMessageUntil=0;RefreshNow();
        }
        internal void ClearDisenchant()
        {
            if(!ItemsAvailable)return;
            ResetDisenchant();_autoEquipMessageUntil=0;RefreshNow();
        }
        void ResetDisenchant()
        {
            _disenchantBatch.Clear();
            if(ActiveSide==SideDisenchant){Inspected=null;InspectedIndex=-1;InspectedRaw=null;}
        }
        void SyncDisenchant()
        {
            if(_disenchantBatch.Sync(Model.Driver?.Session))
            {_autoEquipMessage=Loc.Get("UI_DISENCHANT_SELECTION_CHANGED");_autoEquipMessageUntil=Time.unscaledTime+5;}
        }
        internal void ConfirmDisenchantBatch()
        {
            if(!ItemsAvailable||ActiveSide!=SideDisenchant)return;
            var plan=_disenchantBatch.Freeze(Model.Driver.Session);if(plan==null)return;
            string count=UiNumber.Exact((ulong)plan.Choices.Length);
            Confirm(Loc.Fmt("UI_DISENCHANT_BATCH_CONFIRM",count)+"\n"+DisenchantBatch.Summary(plan),()=>
            {
                var result=DisenchantBatch.Apply(Model.Driver,plan);
                if(result==VerifiedInputResult.Applied)ResetDisenchant();
                ActionNotice(result,Loc.Fmt("UI_DISENCHANT_BATCH_DONE",count)+"\n"+DisenchantBatch.Summary(plan));
            });
        }
    }
}
