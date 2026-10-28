using Roi.Sim;

namespace Roi.UI
{
    public sealed partial class FloatingInventoryHost
    {
        string _autoEquipMessage;
        float _autoEquipMessageUntil;
        void AutoEquip()
        {
            if(!ItemsAvailable)return;
            Items.Cancel();
            var result=ExemplarAutoEquip.Apply(Model.Driver,_memberId,out int changed);
            _autoEquipMessage=result==VerifiedInputResult.Applied
                ?(changed>0?Loc.Fmt("UI_FLOAT_AUTO_EQUIP_DONE",changed):Loc.Get("UI_FLOAT_AUTO_EQUIP_NONE"))
                :Loc.Get("UI_EX_GEAR_STALE");
            _autoEquipMessageUntil=UnityEngine.Time.unscaledTime+5f;
            RefreshNow();
        }
    }
}
