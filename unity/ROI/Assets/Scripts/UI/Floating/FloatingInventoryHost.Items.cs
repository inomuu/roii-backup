using System;
using System.Linq;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class FloatingInventoryHost
    {
        public FloatingItemInteraction Items { get; private set; }
        internal void MarkBagRead(int index)
        {
            _inventoryNotices.Read(index);
            for(int i=0;i<_bag.Count;i++)_bag[i].SetNew(!(ActiveSide==SideForge&&Model.GemMode)&&_inventoryNotices.IsNew(i));
        }
        internal bool ItemsAvailable=>IsOpen&&_confirm==null&&!Result.Visible&&!Model.Busy;
        internal FloatingItemInteraction.Payload Payload(ItemView item,ExemplarEquipmentActions.Row row=null)
            =>new(){Item=item,Row=row,Session=Model.Driver?.Session,Member=_memberId,Side=ActiveSide,GemMode=Model.GemMode,Loadout=_equipment?.EquippedFingerprint,Inventory=_equipment?.InventoryFingerprint};
        internal bool DragStillValid(FloatingItemInteraction.Payload p)
        {
            if(!ItemsAvailable||!ReferenceEquals(p.Session,Model.Driver?.Session)||p.Member!=_memberId
                ||p.Side!=ActiveSide||p.GemMode!=Model.GemMode)return false;
            if(p.Row!=null)
            {
                var now=_equipment?.Candidates?.FirstOrDefault(r=>r.OwnerId==p.Row.OwnerId&&r.Index==p.Row.Index);
                return now!=null&&now.Raw==p.Row.Raw&&p.Loadout==_equipment.EquippedFingerprint
                    &&(p.Row.OwnerId>0||!string.IsNullOrEmpty(p.Row.InstanceId)||p.Inventory==_equipment.InventoryFingerprint);
            }
            return Model.HeldGems.Any(m=>m.def_id==p.Item.def_id&&m.quantity>0);
        }
        internal void PrepareDrag(FloatingItemInteraction.Payload p)
        {
            if(p.Row==null)return;
            // Ask the native equip path once at drag start, not for ten slots every frame — against
            // a snapshot read NOW, not the 0.4 s timer's: the preview clones the live sim, and a bag
            // that moved since the last refresh (a drop, a stack merge, auto-fusion firing every
            // frame) failed the identity check for EVERY slot and painted them all red until the
            // next quiet refresh (the intermittent equip rejection, 2026-09-18).
            var fresh=FreshSnapshot(p.Row);
            if(fresh==null)return;
            // The payload's fingerprints follow the snapshot it is now judged against.
            p.Loadout=fresh.EquippedFingerprint; p.Inventory=fresh.InventoryFingerprint;
            p.EquipSlot=ExemplarEquipmentActions.PreviewDropSlot(Model.Driver,fresh,p.Row);
        }
        /// The equipment snapshot re-read from the live sim, adopted as `_equipment` — null when
        /// `row` no longer exists there in the same shape (the payload is then stale itself).
        ExemplarEquipmentActions.Snapshot FreshSnapshot(ExemplarEquipmentActions.Row row)
        {
            var fresh=ExemplarEquipmentActions.Read(Model.Driver,_memberId);
            if(fresh==null||!fresh.Available)return null;
            bool present=(fresh.Candidates?.Any(r=>r.OwnerId==row.OwnerId&&r.Index==row.Index&&r.Raw==row.Raw)??false)
                ||(fresh.Equipped?.Any(r=>r.OwnerId==row.OwnerId&&r.Index==row.Index&&r.Raw==row.Raw)??false);
            if(!present)return null;
            _equipment=fresh;
            return fresh;
        }
        internal void ContextClick(FloatingItemInteraction.Payload p)
        {
            if(p?.Item==null)return;
            if(p.Row?.OwnerId==0)MarkBagRead(p.Row.Index);
            if(ActiveSide==SideDisenchant)
            {
                if(p.Row?.OwnerId==0){Inspect(p.Item,p.Row.Index,0,p.Row.Raw,false);ToggleDisenchant(p.Row.Index,p.Row.Raw);}
                return;
            }
            if(ActiveSide==SideForge)
            {
                if(Model.GemMode)
                {
                    if(Model.IsGem(p.Item))Model.PickGem(p.Item.def_id);
                }
                else if(p.Row?.OwnerId==0)Model.PickGear(p.Row.Index);
                return;
            }
            if(ActiveSide==SideSocket)
            {
                if(Model.IsGem(p.Item)) { _socket.SelectGemFirst(p.Item.def_id); return; }
                if(p.Row!=null&&(p.Item.kind=="Weapon"||p.Item.kind=="Armor"))
                {
                    if(_socket.GemFirst&&_socket.HasSelectedGem)_socket.SelectEquipmentAfterGem();
                    Inspect(p.Item,p.Row.Index,p.Row.OwnerId,p.Row.Raw); RefreshNow();
                }
                return;
            }
            if((ActiveSide!=SideNone&&ActiveSide!=SideTraining&&ActiveSide!=SideCrests)||p.Row?.OwnerId!=0||(p.Item.kind!="Weapon"&&p.Item.kind!="Armor"))return;
            // Same race as the drag preview: freeze against the sim as it is now.
            var snap=FreshSnapshot(p.Row)??_equipment;
            var choice=ExemplarEquipmentActions.FreezeEquip(snap,p.Row);
            var result=ExemplarEquipmentActions.Apply(Model.Driver,choice,_memberId);
            if(result!=ExemplarEquipmentActions.Result.Applied)ActionNotice(VerifiedInputResult.Stale);
            RefreshNow();
        }
        void BindItemInteractions(ItemView[] entries,bool gems)
        {
            for(int i=0;i<_worn.Count;i++)
            {
                string slot=Slots[i];var row=_equipment?.Equipped?.FirstOrDefault(r=>r.Item.slot==slot);
                Items.Source(_worn[i].Root,()=>row==null?null:Payload(row.Item,row),()=>DescribeItem(row?.Item,row));
                Items.Target(_worn[i].Root,"equip:"+slot,p=>p.Row?.OwnerId==0&&p.EquipSlot==slot&&DragStillValid(p),p=>
                {
                    var choice=ExemplarEquipmentActions.FreezeEquip(_equipment,p.Row,slot);
                    var result=ExemplarEquipmentActions.Apply(Model.Driver,choice,_memberId);
                    if(result!=ExemplarEquipmentActions.Result.Applied)ActionNotice(VerifiedInputResult.Stale);
                    RefreshNow();
                });
            }
            for(int i=0;i<entries.Length;i++)
            {
                int index=i;var item=entries[i];var row=gems?null:_equipment?.Inventory?.FirstOrDefault(r=>r.Index==index);
                Items.Source(_bag[i].Root,()=>Payload(item,row),()=>
                {if(!gems)MarkBagRead(index);return DescribeItem(item,row);});
            }
            // Unequipping is also a native action; a full bag is rejected by its preview.
            Items.Target((RectTransform)_bagContent.parent,"inventory",p=>p.Row?.OwnerId==_memberId&&p.EquipSlot=="Inventory"&&DragStillValid(p),p=>
            {
                var choice=ExemplarEquipmentActions.FreezeUnequip(_equipment,p.Row.Index);
                ExemplarEquipmentActions.Apply(Model.Driver,choice,_memberId);RefreshNow();
            });
        }
        internal string DescribeItem(ItemView item,ExemplarEquipmentActions.Row row=null)
        {
            if(item==null)return null;
            string text=FloatingItemDescription.Format(item,row,Model.IsGem(item));
            if(row?.OwnerId==0&&(item.kind=="Weapon"||item.kind=="Armor"))
            {
                var worn=_equipment?.Equipped?.FirstOrDefault(r=>r.Item.slot==item.slot)?.Item;
                text+="\n\n"+Loc.Get("UI_EX_GEAR_COMPARE_COLUMNS")+"\n"+Loc.Get("UI_EX_GEAR_CURRENT")+": "+(worn==null?Loc.Get("UI_REVIEW_EMPTY_SLOT"):GameText.ItemName(worn));
                text+="\n"+Loc.Get("UI_EXEMPLAR_ATK")+"  "+(worn?.graded_atk??0)+" → "+item.graded_atk;
                text+="\n"+Loc.Get("UI_EXEMPLAR_DEF")+"  "+(worn?.graded_def??0)+" → "+item.graded_def;
                text+="\n"+Loc.Get("UI_EX_GEAR_ITEM_ONLY");
            }
            if(Model.IsGem(item))
            {
                // gem_plan supplies formatted min/max lines even for a non-fitting target.
                var target=_equipment?.Candidates?.FirstOrDefault(r=>r.Item.kind=="Weapon"||r.Item.kind=="Armor");
                if(target!=null)
                {
                    string key=target.OwnerId==0?"gem_plan:inv:"+target.Index:"gem_plan:eq:"+target.OwnerId+","+target.Index;
                    var plan=ForgeModel.Parse<GemSocketPanel.Plan>(Model.Driver.Session.QueryJson(key));
                    var held=plan?.held?.FirstOrDefault(g=>g.def_id==item.def_id);
                    if(held!=null)text+="\n\n"+Loc.Get("UI_FLOAT_GEM_ROLLS")+"\n"+string.Join("\n",(held.lines??Array.Empty<GemSocketPanel.Line>()).Select(l=>l.lo+" – "+l.hi));
                }
                var set=Model.GemsRules.sets.FirstOrDefault(s=>Array.IndexOf(s.defs,item.def_id)>=0);
                if(set!=null)text+="\n"+Loc.Fmt("UI_FLOAT_GEM_GROUP",Loc.GetOr("UI_FLOAT_GROUP_"+set.group,set.group));
            }
            return text;
        }
    }
}
