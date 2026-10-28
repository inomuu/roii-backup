using System;
using System.Collections.Generic;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Paper-doll inspection owns an explicit member; the action helper validates the frozen choice.
    public sealed class ExemplarEquipmentPage : MonoBehaviour
    {
        static readonly string[] Slots={"Weapon","Shield","Helmet","Gloves","Armor","Earring","Shoes","Necklace","Ring","Bracer"};
        static readonly string[] SlotKeys={"UI_EXEMPLAR_SLOT_WEAPON","UI_EXEMPLAR_SLOT_OFFHAND","UI_EXEMPLAR_SLOT_HEAD","UI_EXEMPLAR_SLOT_FACE","UI_EXEMPLAR_SLOT_ARMOR","UI_EXEMPLAR_SLOT_GARMENT","UI_EXEMPLAR_SLOT_SHOES","UI_EXEMPLAR_SLOT_ACCESSORY1","UI_EXEMPLAR_SLOT_ACCESSORY2","UI_EXEMPLAR_SLOT_BRACER"};
        readonly Tile[] _tiles=new Tile[Slots.Length];
        readonly TextMeshProUGUI[] _statValues=new TextMeshProUGUI[8];
        SimDriver _driver;
        SimSession _shownSession;
        ExemplarReadModel.Member _member;
        ExemplarEquipmentActions.Snapshot _state;
        ExemplarEquipmentActions.Row _candidate;
        ExemplarEquipmentActions.Choice _choice;
        ExemplarGearDetails _details;
        TextMeshProUGUI _name,_level,_count,_notice,_selectionLabel;
        ExemplarEquipmentPicker _picker;
        Button _primary,_secondary,_inventory;
        int _slot,_confirmationVersion;
        bool _picking;
        string _message="";

        sealed class Tile
        {
            public Button Button;
            public Image Background, Accent, Selection;
            public ExemplarEquipmentIcon Icon;
            public TextMeshProUGUI Level, Refine;
            public ExemplarEquipmentActions.Row Row;
        }
        public static ExemplarEquipmentPage Create(Transform parent)
        {
            var rt=ExemplarUi.Rect(parent,"EquipmentPage",0,0,1,1);
            var page=rt.gameObject.AddComponent<ExemplarEquipmentPage>();page.Build();return page;
        }
        void Build()
        {
            var summaryImage=ExemplarUi.Panel(transform,"EquipmentStats",0,.075f,.215f,.98f,UiTheme.Surface);
            EquipmentSurface.Apply(summaryImage);
            var summary=summaryImage.transform;
            ExemplarUi.Panel(summary,"TopRule",.055f,.987f,.945f,.990f,ExemplarUi.Gold);
            _name=ExemplarUi.Text(summary,"Member","",.06f,.876f,.95f,.975f,34,ExemplarUi.White);
            _level=ExemplarUi.Text(summary,"Level","",.06f,.818f,.95f,.883f,22,ExemplarUi.Gold);
            ExemplarUi.Text(summary,"StatsTitle",Loc.Get("UI_EX_GEAR_TOTAL_STATS"),.06f,.746f,.95f,.804f,18,ExemplarUi.Muted);
            string[] keys={"UI_EXEMPLAR_HP","UI_EXEMPLAR_ATK","UI_EXEMPLAR_DEF","UI_EXEMPLAR_CRIT","UI_EXEMPLAR_CRIT_DMG","UI_EXEMPLAR_DODGE","UI_EXEMPLAR_ASPD","UI_EXEMPLAR_MITIGATION"};
            for(int i=0;i<keys.Length;i++)
            {
                float top=.73f-i*.061f,bottom=top-.054f;
                if(i==1||i==4||i==7)
                    ExemplarUi.Panel(summary,"GroupRule"+i,.06f,top+.003f,.94f,top+.0045f,new Color(.77f,.82f,.88f,.19f));
                ExemplarUi.Text(summary,"StatLabel"+i,Loc.Get(keys[i]),.063f,bottom,.72f,top,18,ExemplarUi.Muted);
                _statValues[i]=ExemplarUi.Text(summary,"StatValue"+i,"—",.71f,bottom,.937f,top,i<4?26:23,ExemplarUi.White,TextAlignmentOptions.MidlineRight);
            }
            _count=ExemplarUi.Text(summary,"EquippedCount","",.06f,.105f,.95f,.166f,18,ExemplarUi.Muted);
            _inventory=ExemplarUi.Button(summary,"OpenInventory",Loc.Get("UI_EXEMPLAR_EQUIP_BAG"),.055f,.019f,.945f,.088f,OpenInventory);
            for(int i=0;i<Slots.Length;i++)
            {
                int index=i;bool extra=i==10;
                float x=extra?.480f:i<5?.255f:.74f;
                float top=extra?.132f:.915f-(i%5)*.153f;
                float right=x+.066f;
                var b=ExemplarUi.Button(transform,"Slot_"+Slots[i],"",x,top,right,top,()=>SelectSlot(index));
                // Width follows the available page; height follows width, so every slot stays square.
                var slotRect=(RectTransform)b.transform;slotRect.pivot=new Vector2(.5f,1);
                var square=b.gameObject.AddComponent<AspectRatioFitter>();
                square.aspectMode=AspectRatioFitter.AspectMode.WidthControlsHeight;square.aspectRatio=1;
                b.GetComponentInChildren<TextMeshProUGUI>().gameObject.SetActive(false);
                var tile=new Tile { Button=b,Background=b.GetComponent<Image>() };
                EquipmentSurface.Apply(tile.Background);
                Frame(b.transform,"Edge",new Color(.72f,.80f,.86f,.23f),.012f);
                // Category names stay in the selected-slot inspector; the paper doll uses icons only.
                var art=ExemplarUi.Rect(b.transform,"CategoryGlyph",.10f,.10f,.90f,.90f);
                tile.Icon=ExemplarEquipmentIcon.Create(art,Slots[i]);
                tile.Accent=ExemplarUi.Panel(b.transform,"Rarity",0,0,1,.025f,ExemplarUi.Line);
                tile.Selection=ExemplarUi.Panel(b.transform,"Selection",0,0,1,1,Color.clear);
                Frame(tile.Selection.transform,"Selected",ExemplarUi.Gold,.019f);
                tile.Level=ExemplarUi.Text(b.transform,"ItemLevel","",.06f,.033f,.77f,.20f,14,ExemplarUi.White);
                tile.Refine=ExemplarUi.Text(b.transform,"Refine","",.65f,.77f,.95f,.98f,18,ExemplarUi.Gold,TextAlignmentOptions.MidlineRight);
                // Level and the full tier/grade stay in the inspector; tiny duplicate badges
                // clipped both strings and covered the equipment artwork on a compact screen.
                tile.Level.gameObject.SetActive(false);tile.Refine.gameObject.SetActive(false);
                _tiles[i]=tile;
            }
            _selectionLabel=ExemplarUi.Text(transform,"SelectedItemCaption","",.34f,.002f,.69f,.064f,18,ExemplarUi.White,TextAlignmentOptions.Center);
            // Use the gutter freed by the compact roster while preserving the inspector width.
            var sideImage=ExemplarUi.Panel(transform,"GearInspector",.839f,.028f,1.054f,.98f,UiTheme.Surface);
            EquipmentSurface.Apply(sideImage);
            var side=sideImage.rectTransform;
            ExemplarUi.Panel(side,"TopLight",.045f,.997f,.955f,1,new Color(.88f,.86f,.75f,.50f));
            _details=ExemplarGearDetails.Create(side);
            _notice=ExemplarUi.Text(side,"ActionNotice","",.055f,.165f,.945f,.235f,16,ExemplarUi.Gold,TextAlignmentOptions.TopLeft);
            _notice.textWrappingMode=TextWrappingModes.Normal;
            _primary=ExemplarUi.Button(side,"ChangeOrEquip",Loc.Get("UI_EX_GEAR_CHANGE"),.05f,.080f,.95f,.151f,Primary,true);
            _secondary=ExemplarUi.Button(side,"UnequipOrCancel",Loc.Get("UI_EX_GEAR_UNEQUIP"),.05f,.012f,.95f,.070f,Secondary);
            _picker=ExemplarEquipmentPicker.Create(transform,PickCandidate,Primary,Secondary);
        }
        public void Show(ExemplarReadModel.Member member)
        {
            if(_driver==null)_driver=UnityEngine.Object.FindFirstObjectByType<SimDriver>();
            var session=_driver!=null?_driver.Session:null;
            bool changed=(_member?.Id??0)!=(member?.Id??0)||_member?.UnitDef!=member?.UnitDef
                ||!ReferenceEquals(_shownSession,session);
            var previous=_state;
            _member=member;_shownSession=session;
            if(changed){_slot=0;_picking=false;_candidate=null;_choice=null;_message="";_confirmationVersion++;}
            _state=ExemplarEquipmentActions.Read(_driver,member?.Id??0);
            if(_candidate!=null)
            {
                var fresh=FindCandidate(_state.Candidates,_candidate);
                bool stale=previous==null||!_state.Available
                    ||previous.CandidatesFingerprint!=_state.CandidatesFingerprint
                    ||previous.EquippedFingerprint!=_state.EquippedFingerprint
                    ||fresh==null||fresh.Raw!=_candidate.Raw;
                if(stale){_candidate=null;_choice=null;_message=Loc.Get("UI_EX_GEAR_STALE");}
                // The explicit candidate click owns the frozen choice. Refresh must not re-arm it.
            }
            PaintStats();PaintSlots();PaintInspector();
        }
        void PaintStats()
        {
            _name.text=_member==null?Loc.Get("UI_EXEMPLAR_NO_CHARACTER"):GameText.UnitName(_member.UnitDef);
            _level.text=_member==null?"":Loc.Get("UI_EXEMPLAR_LEVEL")+" "+_member.Level;
            var s=_member?.Stats;
            string[] values=s==null?null:new[]{s.MaxHp.ToString("N0"),s.Attack.ToString("N0"),s.Defense.ToString("N0"),Percent(s.CritBp),Percent(s.CritDamageBp),Percent(s.DodgeBp),(s.AttackSpeedX100/100m).ToString("0.00")+"/s",Percent(s.MitigationBp)};
            for(int i=0;i<_statValues.Length;i++)_statValues[i].text=values==null?"—":values[i];
            _count.text=Loc.Get("UI_EX_GEAR_EQUIPPED")+"  "+(_state.Equipped?.Length??0);
            _inventory.interactable=_state.Available;
        }
        void PaintSlots()
        {
            for(int i=0;i<_tiles.Length;i++)
            {
                var tile=_tiles[i];var row=FindEquipped(Slots[i]);
                tile.Row=row;tile.Icon.SetItem(row?.Item,Slots[i]);
                tile.Icon.color=row==null?new Color(1,1,1,.46f):Color.white;
                tile.Background.color=i==_slot?UiTheme.SurfaceActive
                    :row==null?UiTheme.Surface:UiTheme.SurfaceHover;
                tile.Accent.color=row==null?ExemplarUi.Line:ItemSlot.ItemColor(row.Item);
                tile.Selection.gameObject.SetActive(i==_slot);tile.Button.interactable=_state.Available;
                tile.Level.text=row==null?"":Loc.Get("UI_EXEMPLAR_LEVEL")+" "+row.Item.item_level;
                tile.Refine.text=row!=null?GameText.GradeLabel(row.Item.grade_step):"";
            }
            _selectionLabel.gameObject.SetActive(true);
            _selectionLabel.text=_tiles[_slot].Row==null?Loc.Get(SlotKeys[_slot]):GameText.ItemName(_tiles[_slot].Row.Item);
        }
        void PaintInspector()
        {
            var worn=_tiles[_slot].Row;
            _selectionLabel.gameObject.SetActive(!_picking);
            if(_picking)
            {
                var rows=new List<ExemplarEquipmentActions.Row>();
                foreach(var row in _state.Candidates)if(Matches(row.Item,Slots[_slot]))rows.Add(row);
                var issue=_candidate==null?null:ExemplarEquipmentActions.EquipIssue(_state,_candidate);
                string notice=!string.IsNullOrEmpty(_message)?_message:issue!=null?Loc.Get(issue)
                    :_candidate?.OwnerId>0?Loc.Get("UI_EX_GEAR_WORN_BY").Replace("{owner}",GameText.UnitName(_candidate.OwnerUnitDef))
                    :Loc.Get("UI_EX_GEAR_SELECT_PREVIEW");
                _picker.Show(_state,worn,rows,_candidate,Slots[_slot],Loc.Get(SlotKeys[_slot]),notice,
                    _state.Available&&_candidate!=null&&_choice!=null&&issue==null);
                return;
            }
            _picker.gameObject.SetActive(false);
            _details.Show(Loc.Get(SlotKeys[_slot]),worn?.Item,null,false);
            string problem=worn==null?null:ExemplarEquipmentActions.UnequipIssue(_state,worn);
            _notice.text=!string.IsNullOrEmpty(_message)?_message:problem!=null?Loc.Get(problem):"";
            SetButtonText(_primary,"UI_EX_GEAR_CHANGE");SetButtonText(_secondary,"UI_EX_GEAR_UNEQUIP");
            _primary.interactable=_state.Available;
            _secondary.interactable=_state.Available&&worn!=null&&problem==null;
        }
        void SelectSlot(int index)
        {
            _confirmationVersion++;_slot=index;_picking=false;_candidate=null;_choice=null;_message="";ItemTooltip.Hide();
            PaintSlots();PaintInspector();
        }
        void PickCandidate(ExemplarEquipmentActions.Row row)
        {
            if(!_picking||UiConfirm.BlocksNavigation)return;
            _confirmationVersion++;_candidate=row;
            _choice=ExemplarEquipmentActions.FreezeEquip(_state,row,Slots[_slot]);_message="";PaintInspector();
        }
        void Primary()
        {
            if(UiConfirm.BlocksNavigation||!isActiveAndEnabled)return;
            if(!_picking){_picking=true;_message="";_candidate=null;_choice=null;PaintInspector();return;}
            var choice=_choice;if(choice==null)return;
            if(!choice.RequiresConfirmation){Apply(choice);return;}
            // A confirmation must have a real modal host; never use Ask's no-HUD auto-accept path.
            if(UnityEngine.Object.FindFirstObjectByType<HudBar>()==null)return;
            int version=_confirmationVersion;
            string body=Loc.Get("UI_EX_GEAR_TRANSFER_BODY")
                .Replace("{item}",GameText.ItemName(_candidate.Item))
                .Replace("{owner}",GameText.UnitName(choice.SourceOwnerUnitDef))
                .Replace("{target}",GameText.UnitName(_member.UnitDef));
            UiConfirm.Ask(Loc.Get("UI_EX_GEAR_TRANSFER_TITLE"),body,Loc.Get("UI_EX_GEAR_TRANSFER_CONFIRM"),()=>
            {
                if(!isActiveAndEnabled||!_picking||version!=_confirmationVersion)return;
                if(!ReferenceEquals(_choice,choice))
                {_message=Loc.Get("UI_EX_GEAR_STALE");PaintInspector();return;}
                Apply(choice);
            },false);
        }
        public bool TryCancelPicker()
        {
            if(!_picking)return false;
            if(!UiConfirm.BlocksNavigation)Secondary();return true;
        }
        void Secondary()
        {
            if(_picking){_confirmationVersion++;_picking=false;_candidate=null;_choice=null;_message="";PaintInspector();return;}
            var worn=_tiles[_slot].Row;
            if(worn!=null)Apply(ExemplarEquipmentActions.FreezeUnequip(_state,worn.Index));
        }
        void Apply(ExemplarEquipmentActions.Choice choice)
        {
            var result=ExemplarEquipmentActions.Apply(_driver,choice,_member?.Id??0);
            bool applied=result==ExemplarEquipmentActions.Result.Applied;
            _message=applied?"":Loc.Get(result==ExemplarEquipmentActions.Result.Stale?"UI_EX_GEAR_STALE"
                :result==ExemplarEquipmentActions.Result.Unavailable?"UI_EX_GEAR_UNAVAILABLE":"UI_EX_GEAR_REFUSED");
            if(applied){_picking=false;_candidate=null;_choice=null;}
            else if(result==ExemplarEquipmentActions.Result.Stale||result==ExemplarEquipmentActions.Result.Unavailable)
            {_candidate=null;_choice=null;}
            var member=ExemplarReadModel.Fetch(_driver).Find(_member?.Id??0);Show(member);
        }
        void OpenInventory()
        {
            if(!isActiveAndEnabled||_state?.Available!=true||_member==null||_driver==null)return;
            long id=_member.Id;
            bool sameSession=_shownSession!=null&&_shownSession.IsAlive
                &&ReferenceEquals(_driver.Session,_shownSession)&&ReferenceEquals(_state.Session,_shownSession);
            var current=sameSession?ExemplarEquipmentActions.Read(_driver,id):null;
            if(current?.Available!=true||current.MemberId!=_state.MemberId||current.UnitDef!=_member.UnitDef)
            {
                _candidate=null;_choice=null;_message=Loc.Get("UI_EX_GEAR_STALE");PaintInspector();return;
            }
            var hud=UnityEngine.Object.FindFirstObjectByType<HudBar>();if(hud==null)return;
            PartySelection.Select(id);hud.CloseAll();hud.Toggle("Inventory");
        }
        ExemplarEquipmentActions.Row FindEquipped(string slot)
        {
            foreach(var row in _state.Equipped)if(row.Item.slot==slot)return row;return null;
        }
        static ExemplarEquipmentActions.Row FindCandidate(ExemplarEquipmentActions.Row[] rows,ExemplarEquipmentActions.Row selected)
        {foreach(var row in rows)if(row.Index==selected.Index&&row.OwnerId==selected.OwnerId)return row;return null;}
        static bool Matches(ItemView item,string slot)
        {
            if(item.kind!="Weapon"&&item.kind!="Armor")return false;
            if(slot=="Shield")return item.slot=="Shield"||item.weapon_type=="Dagger"; // the off-hand: a shield or a second dagger
                        return item.slot==slot;
        }
        static void Frame(Transform parent,string name,Color color,float thickness)
        {
            ExemplarUi.Panel(parent,name+"Top",0,1-thickness,1,1,color);
            ExemplarUi.Panel(parent,name+"Bottom",0,0,1,thickness,color);
            ExemplarUi.Panel(parent,name+"Left",0,thickness,thickness,1-thickness,color);
            ExemplarUi.Panel(parent,name+"Right",1-thickness,thickness,1,1-thickness,color);
        }
        static string Percent(int bp)=>(bp/100f).ToString("0.#")+"%";
        static void SetButtonText(Button button,string key)=>button.GetComponentInChildren<TextMeshProUGUI>().text=Loc.Get(key);
        void OnDestroy(){if(_picker!=null)Destroy(_picker.gameObject);}
        void OnDisable()
        {
            _confirmationVersion++;_picking=false;_candidate=null;_choice=null;
            if(_picker!=null)_picker.gameObject.SetActive(false);ItemTooltip.Hide();
        }
    }
}

