using System;
using System.Linq;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ForgePanel
    {
        public readonly RectTransform Root;
        readonly ForgeModel _model;
        readonly FloatingInventoryHost _host;
        readonly ForgeCell[] _inputs=new ForgeCell[6];
        readonly TextMeshProUGUI _count,_preview,_rules,_status,_mode;
        readonly Button _submit,_auto,_autoFusion,_clear,_equipment,_gems;
        public ForgePanel(Transform parent,ForgeModel model,FloatingInventoryHost host)
        {
            _model=model; _host=host;
            Root=ForgeUi.Panel(parent,"FusionForge",510,0,320,560,ForgeUi.Ground).rectTransform;
            UiTheme.Border(Root,new Color(.48f,.43f,.32f),2);
            ForgeUi.Heading(ForgeUi.Copy(Root,"UI_FORGE_TITLE",14,4,260,30,17,ForgeUi.Ink),22);
            ForgeUi.Button(Root,"UI_FLOAT_CLOSE",286,5,26,26,()=>host.Side(FloatingInventoryHost.SideNone));
            _equipment=ForgeUi.Button(Root,"UI_FORGE_EQUIPMENT",10,42,148,32,()=>model.Mode(false));
            _gems=ForgeUi.Button(Root,"UI_FORGE_GEMS",162,42,148,32,()=>model.Mode(true));
            _count=ForgeUi.Text(Root,"Count","",14,85,290,20,12); _count.alignment=TextAlignmentOptions.Center;
            var well=ForgeUi.Panel(Root,"FusionWell",12,114,296,192,new Color(.075f,.086f,.105f));
            var sigil=ForgeUi.Rect(well.transform,"Emblem",60,8,176,176).gameObject.AddComponent<ForgeSigil>(); sigil.Rays=true; sigil.color=ForgeUi.Gold; sigil.raycastTarget=false;
            for(int i=0;i<6;i++)
            { int pick=i; _inputs[i]=new ForgeCell(well.transform,52); ForgeUi.Place(_inputs[i].Root,22+(i%3)*99,15+(i/3)*110,52,52); }
            _mode=ForgeUi.Copy(well.transform,"UI_FORGE_NEXT_GRADE",12,78,272,30,15,ForgeUi.Gold); _mode.alignment=TextAlignmentOptions.Center;
            _preview=ForgeUi.Text(Root,"Preview","",18,318,284,64,12); ForgeUi.Wrap(_preview);
            _rules=ForgeUi.Text(Root,"Rules","",18,390,284,60,12,UiTheme.TextMid); ForgeUi.Wrap(_rules);
            _auto=ForgeUi.Button(Root,"UI_FORGE_AUTO",12,458,102,30,()=>model.AutoFill());
            _autoFusion=ForgeUi.Button(Root,"UI_FORGE_AUTO_FUSION_OFF",120,458,128,30,()=>model.ToggleAutoFusion());
            _autoFusion.GetComponentInChildren<TMP_Text>().fontSize=10;
            _clear=ForgeUi.Button(Root,"UI_FORGE_CLEAR",256,458,52,30,()=>model.Clear());
            _submit=ForgeUi.Button(Root,"UI_FORGE_COMBINE",12,496,296,34,Submit,true);
            _status=ForgeUi.Text(Root,"Status","",12,530,296,24,12,UiTheme.TextMid); _status.alignment=TextAlignmentOptions.Center;
        }
        public void Bind()
        {
            if(!Root.gameObject.activeSelf)return;
            for(int i=0;i<6;i++)
            {
                int index=i; ItemView item=null;
                if(_model.GemMode && i<_model.Gems.Count)item=_model.HeldGems.FirstOrDefault(m=>m.def_id==_model.Gems[i]);
                if(!_model.GemMode && i<_model.Gear.Count)item=_model.Gear[i].Item;
                _inputs[i].Bind(item,item==null||_model.Busy?null:()=>_model.Remove(index),false,_model.GemMode);
                _host.Items.Source(_inputs[i].Root,null,()=>_host.DescribeItem(item),()=>item);
                _host.Items.Target(_inputs[i].Root,"forge:"+i,p=>_host.DragStillValid(p)&&index==_model.Count
                    &&(_model.GemMode?p.Row==null&&_model.CanPickGem(p.Item.def_id):p.Row?.OwnerId==0&&_model.CanPickGear(p.Row.Index)),p=>
                {if(_model.GemMode)_model.PickGem(p.Item.def_id);else _model.PickGear(p.Row.Index);});
            }
            _count.text=Loc.Fmt("UI_FORGE_COUNT",_model.Count,_model.Required);
            _status.text=Loc.Get(_model.StatusKey);
            _equipment.interactable=_gems.interactable=!_model.Busy;
            _auto.interactable=!_model.Busy&&!_model.AutoFusionEnabled;
            _autoFusion.interactable=!_model.Busy&&!_model.GemMode;
            _clear.interactable=!_model.Busy&&!_model.AutoFusionRunning;
            _submit.interactable=_model.CanSubmit&&!_model.AutoFusionEnabled;
            _autoFusion.GetComponentInChildren<TMP_Text>().text=Loc.Get(_model.AutoFusionEnabled?"UI_FORGE_AUTO_FUSION_ON":"UI_FORGE_AUTO_FUSION_OFF");
            _autoFusion.GetComponent<Image>().color=_model.AutoFusionEnabled?new Color(.29f,.26f,.19f):ForgeUi.Raised;
            _equipment.GetComponent<Image>().color=!_model.GemMode?new Color(.29f,.26f,.19f):ForgeUi.Raised;
            _gems.GetComponent<Image>().color=_model.GemMode?new Color(.29f,.26f,.19f):ForgeUi.Raised;
            int step=_model.GemMode?(_model.Gems.Count>0?_model.GemGrade(_model.Gems[0])+1:-1):_model.Quote?.grade_step??-1;
            _mode.text=step<0?Loc.Get("UI_FORGE_NEXT_GRADE"):GameText.GradeName(step);
            if(_model.GemMode)
            {
                int grade=step-1;
                var odds=_model.GemsRules?.fail_bp_now_by_grade;
                string chance=grade>=0&&odds!=null&&grade<odds.Length?Percent(10000-odds[grade]):"—";
                _preview.text=Loc.Fmt("UI_FORGE_CHANCE",chance)+"\n"+Loc.Get("UI_FORGE_GEM_PREVIEW").Replace("\\n","\n");
                var groups=(_model.GemsRules?.sets??Array.Empty<ForgeModel.GemSet>()).GroupBy(s=>s.group);
                float total=(_model.GemsRules?.sets??Array.Empty<ForgeModel.GemSet>()).Sum(s=>s.weight);
                _rules.text=Loc.Fmt("UI_FORGE_GEM_CRAFTER",_model.GemsRules?.crafter_level??0)+"\n"+Loc.Get("UI_FORGE_GEM_ODDS")+"\n"+string.Join(" · ",groups.Select(g=>g.Key+" "+g.Count()+" × "+(total>0?100f*g.First().weight/total:0).ToString("0.00")+"%"));
            }
            else if(_model.Quote is { } q)
            {
                _preview.text=Loc.Fmt("UI_FORGE_CHANCE",Percent(10000-q.fail_bp))
                    +"\n"+Loc.Fmt("UI_FORGE_GIL_RECEIVED",q.payout_gil);
                _rules.text=Loc.Fmt("UI_FORGE_BLACKSMITH",_model.GearRules?.blacksmith_level??0)+"\n"+Loc.Get(_model.DestroysGems?"UI_FORGE_GEM_WARNING":"UI_FORGE_GEAR_RULES");
            }
            else { _preview.text=Loc.Get("UI_FORGE_PREVIEW_EMPTY"); _rules.text=Loc.Get("UI_FORGE_GEAR_RULES"); }
        }
        static string Percent(int bp) => (bp/100f).ToString("0.##") + "%";
        void Submit()
        {
            _model.Refresh(); var draft=_model.DraftKey;
            if(!_model.CanSubmit)return;
            if(_model.DestroysGems) _host.Confirm(Loc.Get("UI_FORGE_GEM_WARNING"),()=>_model.Submit(draft));
            else _model.Submit(draft);
        }
    }
}
