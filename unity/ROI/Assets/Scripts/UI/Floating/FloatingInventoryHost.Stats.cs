using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class FloatingInventoryHost
    {
        void BuildStatSummary()
        {
            string[] keys={"UI_FLOAT_STAT_LEVEL","UI_FLOAT_STAT_MAX_HP","UI_FLOAT_STAT_ATK","UI_FLOAT_STAT_DEF"};
            var panel=ForgeUi.Panel(_main,"CombatStatSummary",14,384,248,80,new Color(.04f,.055f,.075f,.72f));
            panel.raycastTarget=false;
            for(int i=0;i<keys.Length;i++)
            {
                float y=i*20;
                var label=ForgeUi.Copy(panel.transform,keys[i],9,y,84,18,12,new Color(.72f,.76f,.81f));
                label.raycastTarget=false;
                var value=ForgeUi.Text(panel.transform,"Value"+i,"—",94,y,142,18,13,ForgeUi.Ink);
                value.alignment=TextAlignmentOptions.MidlineRight;
                value.enableAutoSizing=true;value.fontSizeMin=8;value.fontSizeMax=13;
                value.textWrappingMode=TextWrappingModes.NoWrap;
                value.raycastTarget=false;
                _summaryValues[i]=value;
                if(i<keys.Length-1)
                    ForgeUi.Panel(panel.transform,"Rule"+i,9,y+19,229,.5f,new Color(1,1,1,.08f)).raycastTarget=false;
            }
        }

        void RefreshStatSummary(ExemplarReadModel.Eff stats)
        {
            // Values already include native equipment, training, passives and active modifiers.
            // Never rescale or reconstruct gameplay stats here; the host owns display units.
            string[] values=stats==null?null:new[]{stats.Level.ToString("N0"),stats.MaxHp.ToString("N0"),
                stats.Attack.ToString("N0"),stats.Defense.ToString("N0")};
            for(int i=0;i<_summaryValues.Length;i++)_summaryValues[i].text=values==null?"—":values[i];
        }
    }
}
