using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class DisenchantPanel
    {
        const string GradePreference="ROI.Disenchant.AutoSelectGrades";
        public readonly RectTransform Root;
        readonly FloatingInventoryHost _host;
        readonly TMP_Text _count,_yield,_status;
        readonly RectTransform _itemsContent,_yieldContent;
        readonly Button _confirm,_auto,_clear;
        readonly List<ForgeCell> _cells=new();
        readonly Toggle[] _grades=new Toggle[10];
        int _gradeMask;
        public DisenchantPanel(Transform parent,FloatingInventoryHost host)
        {
            _host=host;_gradeMask=PlayerPrefs.GetInt(GradePreference,3)&1023;
            Root=ForgeUi.Panel(parent,"Disenchant",630,0,320,560,ForgeUi.Ground).rectTransform;
            UiTheme.Border(Root,ForgeUi.Gold,2);
            ForgeUi.Heading(ForgeUi.Copy(Root,"UI_DISENCHANT",14,4,240,30,17,ForgeUi.Ink),22);
            ForgeUi.Button(Root,"UI_FLOAT_CLOSE",286,5,26,26,()=>host.Side(FloatingInventoryHost.SideNone));
            var intro=ForgeUi.Copy(Root,"UI_DISENCHANT_MULTI_HELP",14,42,292,34,12);ForgeUi.Wrap(intro);
            ForgeUi.Copy(Root,"UI_DISENCHANT_GRADES",14,79,292,21,12,ForgeUi.Gold);
            for(int i=0;i<_grades.Length;i++)MakeGrade(i);
            _auto=ForgeUi.Button(Root,"UI_DISENCHANT_AUTO_SELECT",14,239,179,29,()=>host.AutoSelectDisenchant(_gradeMask),true);
            _clear=ForgeUi.Button(Root,"UI_DISENCHANT_CLEAR",201,239,105,29,host.ClearDisenchant);
            _count=ForgeUi.Text(Root,"SelectedCount","",14,275,292,24,12,ForgeUi.Gold);
            _itemsContent=UiScroll.Create(ForgeUi.Rect(Root,"SelectedItems",14,303,292,105),out _);
            ForgeUi.Panel(Root,"YieldBackground",14,416,292,64,ForgeUi.Raised);
            _yieldContent=UiScroll.Create(ForgeUi.Rect(Root,"EssenceScroll",23,422,274,53),out _);
            _yield=ForgeUi.Text(_yieldContent,"EssencePreview","",0,0,258,53,12,ForgeUi.Gold);ForgeUi.Wrap(_yield);
            _confirm=ForgeUi.Button(Root,"UI_DISENCHANT",14,489,292,32,host.ConfirmDisenchantBatch,true);
            _status=ForgeUi.Text(Root,"Status","",14,526,292,29,11,UiTheme.TextMid);ForgeUi.Wrap(_status);
            Root.gameObject.SetActive(false);
        }
        void MakeGrade(int grade)
        {
            var bg=ForgeUi.Panel(Root,"Grade"+grade,14+(grade%2)*150,103+(grade/2)*26,142,23,ForgeUi.Raised);
            var toggle=bg.gameObject.AddComponent<Toggle>();toggle.targetGraphic=bg;
            var box=ForgeUi.Panel(bg.transform,"Box",4,4,15,15,new Color(.05f,.06f,.08f));box.raycastTarget=false;
            UiTheme.Border(box.rectTransform,ForgeUi.Gold);
            var check=ForgeUi.Panel(box.transform,"Check",3,3,9,9,ForgeUi.Gold);check.raycastTarget=false;toggle.graphic=check;
            var label=ForgeUi.Text(bg.transform,"GradeName",GameText.GradeName(grade),25,0,112,23,13,GameText.GradeColor(grade));
            label.raycastTarget=false;label.enableAutoSizing=true;label.fontSizeMin=12;label.fontSizeMax=13;
            toggle.SetIsOnWithoutNotify((_gradeMask&(1<<grade))!=0);
            toggle.onValueChanged.AddListener(on=>
            {
                if(on)_gradeMask|=1<<grade;else _gradeMask&=~(1<<grade);
                PlayerPrefs.SetInt(GradePreference,_gradeMask);PlayerPrefs.Save();
                _auto.interactable=_host.ItemsAvailable&&_gradeMask!=0;
            });
            _grades[grade]=toggle;
        }
        public void Bind(DisenchantBatch.Plan plan,bool available,string notice)
        {
            if(!Root.gameObject.activeSelf)return;
            int count=plan?.Choices.Length??0;
            _count.text=Loc.Fmt("UI_DISENCHANT_SELECTED_COUNT",UiNumber.Exact((ulong)count));
            _auto.interactable=available&&_gradeMask!=0;_clear.interactable=available&&count>0;
            foreach(var g in _grades)g.interactable=available;
            var rows=plan==null?null:MailWindow.RowsOfArray(plan.Inventory);
            while(_cells.Count<count)_cells.Add(new ForgeCell(_itemsContent,48));
            for(int i=0;i<_cells.Count;i++)
            {
                var cell=_cells[i];cell.Root.gameObject.SetActive(i<count);if(i>=count)continue;
                var choice=plan.Choices[i];string raw=rows[choice.Index];var item=ForgeModel.Parse<ItemView>(raw);
                ForgeUi.Place(cell.Root,(i%5)*54,(i/5)*54,48,48);
                cell.Bind(item,()=>_host.ToggleDisenchant(choice.Index,raw),true,gem:_host.Model.IsGem(item));
                _host.Items.Source(cell.Root,()=>null,()=>ForgeUi.Description(item),()=>item);
            }
            UiScroll.Fit(_itemsContent,Mathf.Max(105,Mathf.Ceil(count/5f)*54));
            _yield.text=notice??(count==0?Loc.Get("UI_DISENCHANT_SELECT"):DisenchantBatch.Summary(plan));
            float height=Mathf.Max(53,_yield.GetPreferredValues(_yield.text,258,4000).y+5);
            _yield.rectTransform.sizeDelta=new Vector2(258,height);UiScroll.Fit(_yieldContent,height);
            _confirm.interactable=available&&count>0;
            _confirm.GetComponentInChildren<TMP_Text>().text=Loc.Fmt("UI_DISENCHANT_BATCH_ACTION",UiNumber.Exact((ulong)count));
            _status.text=Loc.Get("UI_DISENCHANT_WARNING");
        }
    }
}
