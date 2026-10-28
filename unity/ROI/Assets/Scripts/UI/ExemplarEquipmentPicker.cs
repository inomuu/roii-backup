using System;
using System.Collections.Generic;
using UnityEngine;

namespace Roi.UI
{
    // Item inspection reuses the collection layout; the page still owns frozen native choices.
    public sealed class ExemplarEquipmentPicker : MonoBehaviour
    {
        CollectionBrowser _browser;
        ExemplarEquipmentActions.Snapshot _state;
        public static ExemplarEquipmentPicker Create(Transform parent,Action<ExemplarEquipmentActions.Row> pick,Action equip,Action cancel)
        {
            var screen=parent.GetComponentInParent<ExemplarScreen>();
            var root=ExemplarUi.Rect(screen!=null?screen.transform:parent,"ChangeEquipment",0,0,1,1);
            var picker=root.gameObject.AddComponent<ExemplarEquipmentPicker>();
            picker._browser=CollectionBrowser.Create(root,true);
            picker._browser.Pick=e=>pick?.Invoke(e?.Row);
            picker._browser.Primary=equip;picker._browser.Back=cancel;picker._browser.Tools=cancel;
            picker._browser.Matches=(category,e)=>category switch
            {1=>e.Row.OwnerId==0,2=>e.Row.OwnerId>0,3=>ExemplarEquipmentActions.EquipIssue(picker._state,e.Row)==null,_=>true};
            picker.gameObject.SetActive(false);return picker;
        }
        public void Show(ExemplarEquipmentActions.Snapshot state,ExemplarEquipmentActions.Row worn,
            List<ExemplarEquipmentActions.Row> rows,ExemplarEquipmentActions.Row candidate,
            string slot,string slotLabel,string notice,bool canEquip)
        {
            bool opening=!gameObject.activeSelf;gameObject.SetActive(true);transform.SetAsLastSibling();_state=state;
            _browser.SetHeader(Loc.Get("UI_EX_GEAR_CHANGE")+" / "+slotLabel,
                Loc.Get("UI_EXEMPLAR_EQUIPMENT")+"  ·  "+rows.Count,GameText.UnitName(state.UnitDef),state.UnitDef,false);
            var entries=new List<CollectionBrowser.Entry>();CollectionBrowser.Entry selected=null;
            foreach(var row in rows)
            {
                var entry=new CollectionBrowser.Entry{Item=row.Item,Row=row,Key=row.OwnerId+":"+row.Index};entries.Add(entry);
                if(candidate!=null&&row.OwnerId==candidate.OwnerId&&row.Index==candidate.Index&&row.Raw==candidate.Raw)selected=entry;
            }
            _browser.Bind(entries,selected?.Key);
            string owner=selected?.Row.OwnerId>0?Loc.Fmt("UI_FLOAT_OWNER",GameText.UnitName(selected.Row.OwnerUnitDef)):Loc.Get("UI_EX_GEAR_IN_BAG");
            string details=selected==null?null:FloatingItemDescription.Format(selected.Item,selected.Row);
            if(state.HasBenchedEquipment)details+="\n\n"+Loc.Get("UI_EX_GEAR_BENCH_UNAVAILABLE");
            _browser.ShowDetails(selected,details,worn?.Item,owner,Loc.Get("UI_EX_GEAR_EQUIP"),canEquip,canEquip?"":notice);
            if(opening){_browser.SelectCategory(0);_browser.GridScroll.verticalNormalizedPosition=1;}
        }
        void OnEnable()=>GetComponentInParent<ModeTabs>()?.SetModeSwitchInputEnabled(false);
        void OnDisable()=>GetComponentInParent<ModeTabs>()?.SetModeSwitchInputEnabled(true);
    }
}
