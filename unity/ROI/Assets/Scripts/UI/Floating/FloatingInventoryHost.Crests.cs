using System.Linq;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class FloatingInventoryHost
    {
        CrestPanel _crests;
        DisenchantPanel _disenchantPanel;
        Button _crestsButton,_disenchantButton;
        void BuildCrests()
        {
            _crests=new CrestPanel(_frame,this);
            _disenchantPanel=new DisenchantPanel(_frame,this);
            Model.AutoFusionBeforeBatch=SyncDisenchant;
            Model.AutoFusionConsumed=AutoFusionConsumed;
            Model.AutoFusionReserved=(index,raw)=>IsOpen&&(_disenchantBatch.Contains(index)
                ||InspectedOwner==0&&InspectedIndex==index&&InspectedRaw==raw
                ||Items?.Dragged?.Row?.Index==index&&Items.Dragged.Row.OwnerId==0&&Items.Dragged.Row.Raw==raw);
            Model.AutoFusionBlocked=()=>_confirm!=null||Result?.Visible==true||Items?.Dragged!=null
                ||IsOpen&&ActiveSide==SideForge&&Model.Gear.Count>0;
            _crestsButton=ForgeUi.Button(_main,"UI_CRESTS",375,516,112,34,()=>Side(ActiveSide==SideCrests?SideNone:SideCrests));
            _disenchantButton=ForgeUi.Button(_main,"UI_DISENCHANT",496,516,112,34,()=>Side(ActiveSide==SideDisenchant?SideNone:SideDisenchant));
        }
        void RefreshCrests()
        {
            _crests.Bind();
            _disenchantPanel.Bind(ActiveSide==SideDisenchant?_disenchantBatch.Freeze(Model.Driver.Session):null,ItemsAvailable,
                Time.unscaledTime<_autoEquipMessageUntil?_autoEquipMessage:null);
        }
        void DestroySelected(bool disenchant)
        {
            if(!ItemsAvailable||InspectedOwner!=0||InspectedIndex<0)return;
            var session=Model.Driver.Session;
            int index=InspectedIndex;string raw=InspectedRaw;
            var rows=MailWindow.RowsOfArray(session.QueryJson("inventory"));
            if(index>=rows.Count||rows[index]!=raw){ActionNotice(VerifiedInputResult.Stale);return;}
            string inventory=session.QueryJson("inventory");
            var choice=disenchant?CrestModel.Freeze(session,index):null;
            if(disenchant&&choice==null)return;
            string description=Loc.Fmt("UI_DISENCHANT_CONFIRM",ForgeUi.Name(Inspected))+
                "\n"+(disenchant?CrestModel.Yield(choice.Reward):Loc.Get("UI_DISCARD_NO_REWARD"));
            Confirm(description,()=>
            {
                VerifiedInputResult result;
                if(disenchant)result=CrestModel.Disenchant(Model.Driver,choice);
                else if(Model.Driver.Session!=session||session.QueryJson("inventory")!=inventory)result=VerifiedInputResult.Stale;
                else result=Model.Driver.ApplyVerifiedInputs(session,new[]{"{\"DiscardItem\":{\"inv_idx\":"+index+"}}"},copy=>copy.QueryJson("inventory")!=inventory);
                if(result==VerifiedInputResult.Applied)
                {
                    Model.ResetDraft();Inspected=null;InspectedIndex=-1;InspectedRaw=null;
                }
                ActionNotice(result,disenchant?CrestModel.Yield(choice?.Reward):Loc.Get("UI_DISCARD_DONE"));
            });
        }
        void ActionNotice(VerifiedInputResult result,string success=null)
        {
            _autoEquipMessage=result==VerifiedInputResult.Applied?success:Loc.Get("UI_EX_GEAR_STALE");
            _autoEquipMessageUntil=Time.unscaledTime+5;RefreshNow();
        }
    }
}
