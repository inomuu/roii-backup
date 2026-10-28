#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System.Collections;
using Roi.UI;
using TMPro;
using UnityEngine;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        // Visual specimens only: render the production cell at every grade, without granting
        // equipment or changing native rules. This is a screenshot aid, not a gameplay test.
        IEnumerator CaptureGradeSurfaces()
        {
            _host.Items.Cancel();
            var panel=ForgeUi.Panel(_host.transform,"GradeSurfaceSpecimens",14,24,800,370,ExemplarUi.Deep);
            UiTheme.Border(panel.rectTransform,ExemplarUi.Line,1);
            ForgeUi.Text(panel.transform,"PreviewTitle","GRADE COLOURS · EQUIPMENT / GEM",18,12,760,28,18);
            for(int grade=0;grade<10;grade++)
            {
                float x=16+(grade%5)*155,y=58+(grade/5)*145;
                var label=ForgeUi.Text(panel.transform,"Grade"+grade,GameText.GradeLabel(grade),x,y,148,32,12);
                label.alignment=TextAlignmentOptions.Center;
                var equipment=new ForgeCell(panel.transform,62);
                ForgeUi.Place(equipment.Root,x+5,y+38,62,62);
                equipment.Bind(new ItemView{def_id="shield_s1",kind="Armor",slot="Shield",grade_step=grade},null,grade==4);
                var gem=new ForgeCell(panel.transform,62);
                ForgeUi.Place(gem.Root,x+77,y+38,62,62);
                gem.Bind(new ItemView{def_id="gem_strife_g"+grade,kind="Material",grade_step=grade,quantity=6},null,false,true);
            }
            ForgeUi.Text(panel.transform,"PreviewNote","Epic equipment is selected: its grade colour is retained.",18,340,760,20,12,ExemplarUi.Muted);
            Canvas.ForceUpdateCanvases();yield return Shot("17-grade-surfaces");
            Destroy(panel.gameObject);
        }
    }
}
#endif
