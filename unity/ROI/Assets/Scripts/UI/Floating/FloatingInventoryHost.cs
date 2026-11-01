using System;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    // One persistent controller, separate from HudBar (which Companion hides). Closing is visual only.
    public sealed partial class FloatingInventoryHost : MonoBehaviour
    {
        public const int MainWidth=620;
        public const float Width=MainWidth+330, Height=560;
        const int BagColumns=5;
        const float BagCellSize=56, BagPitch=62;
        const int BagMinimumCells=30;
        public const int SideNone=0, SideForge=1, SideSocket=2, SideTraining=3, SideCrests=4, SideDisenchant=5;
        public ForgeModel Model { get; private set; }
        public bool IsOpen=>_frame!=null&&_frame.gameObject.activeSelf;
        public int ActiveSide { get; private set; }
        public bool CompanionContext { get; private set; }
        public bool FullStatsOpen => _fullStats != null && _fullStats.Visible;
        public int WindowWidth => MainWidth + (FullStatsOpen ? 330 : 0) + (ActiveSide != SideNone ? 330 : 0);
        internal string CurrentUnitDef { get; private set; }
        public event Action LayoutChanged;
        public long MemberId=>_memberId;
        public ItemView Inspected { get; private set; }
        public int InspectedIndex { get; private set; }=-1;
        public long InspectedOwner { get; private set; }
        public string InspectedRaw { get; private set; }
        public ForgeResult Result { get; private set; }
        RectTransform _frame,_main,_bagContent,_confirm;
        Canvas _canvas;
        int _companionHeight;
        float _companionUiScale=1;
        internal void SetCompanionLayout(int height,float scale) { _companionHeight=height; _companionUiScale=scale; Arrange(); }
        TextMeshProUGUI _memberName,_notice,_bagTitle,_bagEmpty;
        int _bagFilter; bool _bagSort;
        readonly List<Button> _bagFilters=new();
        Button _bagEquip,_bagSortButton; ScrollRect _bagScroll;
        readonly TextMeshProUGUI[] _summaryValues=new TextMeshProUGUI[4];
        Image _avatar;
        CombatHudPortrait _equipmentPortrait;
        string _portraitUnit;
        readonly List<ForgeCell> _bag=new(),_worn=new();
        ForgePanel _forge; GemSocketPanel _socket; TrainingPanel _training; FloatingFullStatsPanel _fullStats;
        Button _fullStatsButton, _autoEquip;
        Button _blacksmithButton, _trainingButton, _socketButton;
        long _memberId; float _nextRefresh;
        ExemplarReadModel.Snapshot _roster;
        ExemplarEquipmentActions.Snapshot _equipment;
        SimSession _lastSession;
        readonly InventoryNoticeState _inventoryNotices=new();
        Vector2 _battlePosition=new(-160,45),_companionPosition=Vector2.zero;
        // Three rows of core gear; the bottom two rows are accessories (two on each side).
        static readonly string[] Slots={"Weapon","Helmet","Armor","Ring","Necklace","Shield","Gloves","Shoes","Earring","Bracer"};
        public static FloatingInventoryHost Create(Transform parent)
        {
            var go=new GameObject("FloatingInventory",typeof(RectTransform)); go.transform.SetParent(parent,false);
            var host=go.AddComponent<FloatingInventoryHost>(); host.Build(); return host;
        }
        void Build()
        {
            _canvas=gameObject.AddComponent<Canvas>(); _canvas.renderMode=RenderMode.ScreenSpaceOverlay; _canvas.sortingOrder=65;
            gameObject.AddComponent<CanvasScaler>().uiScaleMode=CanvasScaler.ScaleMode.ConstantPixelSize;
            gameObject.AddComponent<GraphicRaycaster>();
            Model=gameObject.AddComponent<ForgeModel>(); Model.Bind(FindFirstObjectByType<SimDriver>());
            Model.Changed+=RefreshNow; Model.Completed+=OnResult;
            _frame=ForgeUi.Rect(transform,"WindowGroup",0,0,Width,Height); _frame.anchorMin=_frame.anchorMax=_frame.pivot=new Vector2(.5f,.5f);
            _main=ForgeUi.Panel(_frame,"CharacterInventory",0,0,MainWidth,Height,ForgeUi.Ground).rectTransform;
            var backdrop=ForgeUi.Rect(_main,"InventoryBackdrop",10,48,MainWidth-20,Height-60).gameObject.AddComponent<InventoryBackdrop>();
            backdrop.raycastTarget=false;
            var outer=AlbumCodexSurface.Create(_main,"CodexFrame",true);
            outer.Selected=true;outer.rectTransform.anchorMin=Vector2.zero;outer.rectTransform.anchorMax=Vector2.one;outer.rectTransform.offsetMin=outer.rectTransform.offsetMax=Vector2.zero;outer.transform.SetAsFirstSibling();
            _main.GetComponent<Image>().color=Color.clear;
            var light=ForgeUi.Rect(_main,"AmbientSurface",10,48,MainWidth-20,Height-60);MenuAtmosphere.Create(light);
            ForgeUi.Panel(_main,"BagSurface",278,100,MainWidth-294,386,new Color(.018f,.032f,.051f,.72f)).raycastTarget=false;
            var titlebar=ForgeUi.Panel(_main,"DragBar",8,8,MainWidth-16,36,Color.clear);
            ForgeUi.Heading(ForgeUi.Copy(titlebar.transform,"UI_FLOAT_TITLE",10,0,400,36,16,ForgeUi.Ink),22);
            var drag=titlebar.gameObject.AddComponent<ForgeDrag>(); drag.Target=_frame; drag.Moved=()=>{ClampPosition();RememberPosition();};
            ForgeUi.Button(_main,"UI_FLOAT_CLOSE",MainWidth-48,10,36,32,Close);
            ForgeUi.Panel(_main,"Divider",270,56,1,448,UiTheme.LineStrong);
            _memberName=ForgeUi.Text(_main,"MemberName","",52,56,166,36,14); _memberName.alignment=TextAlignmentOptions.Center;
            ForgeUi.Heading(_memberName,20);
            _memberName.enableAutoSizing=true;_memberName.fontSizeMax=20;_memberName.fontSizeMin=14;
            var prev=ForgeUi.Button(_main,"UI_FLOAT_PREV",14,56,34,36,()=>Member(-1));
            var next=ForgeUi.Button(_main,"UI_FLOAT_NEXT",225,56,34,36,()=>Member(1));
            var illustration=ForgeUi.Rect(_main,"CharacterIllustration",30,96,210,242);
            _equipmentPortrait=illustration.gameObject.AddComponent<CombatHudPortrait>();
            _equipmentPortrait.Crop=new Rect(.22f,.25f,.50f,.664f);
            _equipmentPortrait.FeatherAllEdges=true;
            _equipmentPortrait.color=Color.white; _equipmentPortrait.raycastTarget=false;
            var portrait=ForgeUi.Rect(_main,"Portrait",87,257,96,96);
            _avatar=portrait.gameObject.AddComponent<Image>(); _avatar.preserveAspect=true; _avatar.raycastTarget=false;
            for(int i=0;i<10;i++)
            { int index=i; var cell=new ForgeCell(_main,44); ForgeUi.Place(cell.Root,i<5?14:218,104+(i%5)*48,44,44); _worn.Add(cell); }
            _autoEquip=ForgeUi.Button(_main,"UI_FLOAT_AUTO_EQUIP",66,342,144,36,AutoEquip,true);
            BuildStatSummary();
            _fullStatsButton=ForgeUi.Button(_main,"UI_FLOAT_FULL_STATS",14,472,248,36,()=>SetFullStatsVisible(!FullStatsOpen));
            _bagTitle=ForgeUi.Copy(_main,"UI_FLOAT_BAG",284,56,304,36,14,ForgeUi.Gold);
            ForgeUi.Heading(_bagTitle,20);
            _bagTitle.enableAutoSizing=true;_bagTitle.fontSizeMin=13;_bagTitle.fontSizeMax=20;
            _bagTitle.textWrappingMode=TextWrappingModes.NoWrap;
            string[] filterKeys={"UI_COLLECTION_ALL","UI_REVIEW_GEAR","UI_FORGE_GEMS","UI_REVIEW_OTHER"};
            for(int i=0;i<filterKeys.Length;i++) { int filter=i;
                _bagFilters.Add(ForgeUi.Button(_main,filterKeys[i],284+i*64,96,60,30,()=>{_bagFilter=filter;Inspected=null;InspectedIndex=-1;InspectedRaw=null;Items?.Cancel();RefreshNow();_bagScroll.verticalNormalizedPosition=1;}));
            }
            _bagSortButton=ForgeUi.Button(_main,"UI_REVIEW_SORT",544,96,64,30,()=>{_bagSort=!_bagSort;RefreshNow();_bagScroll.verticalNormalizedPosition=1;});
            var scroll=ForgeUi.Rect(_main,"BagScroll",284,134,324,338); _bagContent=UiScroll.Create(scroll,out var bagScroll, false);_bagScroll=bagScroll;
            _bagEmpty=ForgeUi.Copy(scroll,"UI_COLLECTION_EMPTY",10,110,286,90,16,UiTheme.TextMid);ForgeUi.Wrap(_bagEmpty);_bagEmpty.alignment=TextAlignmentOptions.Center;
            _bagEquip=ForgeUi.Button(_main,"UI_EX_GEAR_EQUIP",516,478,92,30,EquipInspected,true);
            // Five 56px cells occupy 304px. Reserve the remaining gutter for a draggable bar.
            bagScroll.viewport.offsetMax=new Vector2(-18,0);
            var track=ForgeUi.Panel(scroll,"InventoryScrollTrack",312,0,12,338,new Color(.10f,.12f,.15f,.9f));
            var handle=UiTheme.Flat(track.transform,"Handle",new Color(.66f,.61f,.48f,.95f));
            handle.rectTransform.anchorMin=Vector2.zero;handle.rectTransform.anchorMax=Vector2.one;
            handle.rectTransform.offsetMin=new Vector2(2,0);handle.rectTransform.offsetMax=new Vector2(-2,0);
            var bar=track.gameObject.AddComponent<Scrollbar>();bar.direction=Scrollbar.Direction.BottomToTop;
            bar.handleRect=handle.rectTransform;bar.targetGraphic=handle;
            bar.navigation=new Navigation{mode=Navigation.Mode.None};
            track.raycastTarget=handle.raycastTarget=true;
            bagScroll.verticalScrollbar=bar;
            bagScroll.verticalScrollbarVisibility=ScrollRect.ScrollbarVisibility.AutoHide;
            // Only transient action feedback remains; item details live in the hover tooltip.
            _notice=ForgeUi.Text(_main,"ActionNotice","",284,478,224,32,12); ForgeUi.Wrap(_notice);
            _notice.enableAutoSizing=true;_notice.fontSizeMin=11;_notice.fontSizeMax=12;_notice.raycastTarget=false;
            _blacksmithButton=ForgeUi.Button(_main,"UI_BLACKSMITH",12,516,112,34,()=>Side(ActiveSide==SideForge?SideNone:SideForge));
            _trainingButton=ForgeUi.Button(_main,"UI_TRAINING",133,516,112,34,()=>Side(ActiveSide==SideTraining?SideNone:SideTraining));
            _socketButton=ForgeUi.Button(_main,"UI_FLOAT_SOCKET",254,516,112,34,()=>Side(ActiveSide==SideSocket?SideNone:SideSocket));
            _forge=new ForgePanel(_frame,Model,this); _socket=new GemSocketPanel(_frame,this);
            var trainingModel=gameObject.AddComponent<TrainingModel>(); trainingModel.Bind(Model.Driver);
            _training=new TrainingPanel(_frame,trainingModel,this);
            _fullStats=new FloatingFullStatsPanel(_frame,this);
            BuildCrests();
            Result=ForgeResult.Create(_frame);
            Items=gameObject.AddComponent<FloatingItemInteraction>();Items.Bind(this);
            _frame.gameObject.SetActive(false); Side(0);
        }
        public void SetCompanionContext(bool companion)
        {
            if(CompanionContext==companion)return; Items?.Cancel(); RememberPosition(); CompanionContext=companion;
            _frame.anchoredPosition=companion?_companionPosition:_battlePosition; Arrange(); LayoutChanged?.Invoke();
        }
        public void Toggle() { if(IsOpen)Close(); else Open(); }
        public void Open()
        {
            _frame.gameObject.SetActive(true); _frame.anchoredPosition=CompanionContext?_companionPosition:_battlePosition;
            UiPageTransition.Play(_frame);
            RefreshNow(); Arrange(); LayoutChanged?.Invoke();
        }
        public void Close()
        { bool leavingForge=ActiveSide==SideForge; Items.Cancel(); ResetDisenchant(); SetFullStatsVisible(false); RememberPosition(); CancelConfirmation(); Result.Hide(); if(leavingForge)Model.ResetDraft(); _frame.gameObject.SetActive(false); LayoutChanged?.Invoke(); }
        public void SetFullStatsVisible(bool visible)
        {
            float oldWidth=WindowWidth;
            _fullStats?.SetVisible(visible);
            float nextWidth=WindowWidth;
            if(Mathf.Abs(nextWidth-oldWidth)>.01f)_frame.anchoredPosition-=new Vector2((nextWidth-oldWidth)*.5f,0);
            RefreshToolHighlights();
            Arrange(); RefreshNow(); LayoutChanged?.Invoke();
        }
        public void Side(int side)
        {
            if(Model.Busy || Result!=null&&Result.Visible)return;
            Items?.Cancel();
            bool leavingForge=ActiveSide==SideForge&&side!=SideForge;
            if(ActiveSide==SideDisenchant&&side!=SideDisenchant)ResetDisenchant();
            float oldWidth = _frame.rect.width;
            bool changed=ActiveSide!=side;
            ActiveSide=side;
            if(leavingForge)Model.ResetDraft();
            _frame.anchoredPosition += new Vector2((WindowWidth - oldWidth) * .5f, 0);
            _forge.Root.gameObject.SetActive(side==SideForge); _socket.Root.gameObject.SetActive(side==SideSocket); _training.Root.gameObject.SetActive(side==SideTraining); _crests.Root.gameObject.SetActive(side==SideCrests);
            _disenchantPanel.Root.gameObject.SetActive(side==SideDisenchant);
            RefreshToolHighlights();
            CancelConfirmation(); Arrange(); RefreshNow(); LayoutChanged?.Invoke();
            if(changed&&side!=SideNone)
            {
                RectTransform panel=side==SideForge?_forge.Root:side==SideSocket?_socket.Root:side==SideTraining?_training.Root:side==SideCrests?_crests.Root:_disenchantPanel.Root;
                UiPageTransition.Play(panel);
            }
        }
        public bool Back()
        {
            if(!IsOpen)return false;
            if(Items.Dragged!=null){Items.Cancel();return true;}
            if(_confirm!=null)CancelConfirmation(); else if(Result.Visible)Result.Hide(); else if(FullStatsOpen)SetFullStatsVisible(false); else if(ActiveSide!=SideNone)Side(SideNone); else Close(); return true;
        }
        void Member(int delta)
        {
            Items.Cancel();
            if(_roster?.Members.Count>0) { int i=_roster.Members.FindIndex(m=>m.Id==_memberId); _memberId=_roster.Members[(i+delta+_roster.Members.Count)%_roster.Members.Count].Id; RefreshNow(); }
        }
        public void RefreshNow()
        {
            if(Model==null||!IsOpen)return;
            if(Model.Driver==null)Model.Bind(FindFirstObjectByType<SimDriver>());
            if(!Model.Refresh())return;
            _inventoryNotices.Observe(Model.Driver.Session,Model.Bag,Model.Driver.Session.ViewJson());
            SyncDisenchant();
            if(!ReferenceEquals(_lastSession,Model.Driver.Session))
            { _lastSession=Model.Driver.Session; Inspected=null; InspectedIndex=-1; InspectedRaw=null; CancelConfirmation(); Result.Hide(); }
            _roster=ExemplarReadModel.Fetch(Model.Driver);
            var member=_roster.Find(_memberId)??_roster.Members.FirstOrDefault(); _memberId=member?.Id??0;
            CurrentUnitDef=member?.UnitDef??"";
            _equipment=ExemplarEquipmentActions.Read(Model.Driver,_memberId);
            if(_portraitUnit!=CurrentUnitDef)
            {
                _portraitUnit=CurrentUnitDef;
                var art=CurrentUnitDef=="seraphine"?Resources.Load<Texture2D>("EquipmentPortraits/Seraphine"):null;
                _equipmentPortrait.SetTexture(art);
                _equipmentPortrait.gameObject.SetActive(art!=null);
            }
            _autoEquip.interactable=ItemsAvailable&&_equipment.Available&&_equipment.Inventory.Length>0;
            _memberName.text=member==null?Loc.Get("UI_FLOAT_NO_CHARACTER"):GameText.UnitName(member.UnitDef);
            _avatar.sprite=member==null?null:ExemplarPortrait.Avatar(member.UnitDef); _avatar.enabled=_avatar.sprite!=null && !_equipmentPortrait.gameObject.activeSelf;
            RefreshStatSummary(member?.Stats);
            _fullStatsButton.interactable=member?.Stats!=null;
            if(FullStatsOpen)_fullStats.Refresh(_memberId,CurrentUnitDef);
            for(int i=0;i<10;i++)
            {
                string slot=Slots[i]; var row=_equipment?.Equipped?.FirstOrDefault(r=>r.Item.slot==slot);
                _worn[i].Bind(row?.Item,row==null?null:()=>Inspect(row.Item,row.Index,row.OwnerId,row.Raw),InspectedOwner==_memberId&&InspectedIndex==row?.Index,false,0,Loc.GetOr("UI_SLOT_"+slot,slot),slot);
                bool socketFilter=ActiveSide==SideSocket&&_socket.GemFirst&&_socket.HasSelectedGem;
                _worn[i].SetCompatibility(socketFilter,row==null||_socket.IsCompatibleEquipment(row.Item,row.Index,row.OwnerId));
            }
            // Socket mode needs to calculate its compatibility set before the bag cells
            // are painted. This lets either side of the pairing act as the starting point.
            _forge.Bind(); _socket.Bind(); _training.Bind();
            bool gems=ActiveSide==SideForge&&Model.GemMode;
            var entries=gems?Model.HeldGems.Where(Model.IsGem).ToArray():Model.Bag;
            _bagTitle.text=Loc.Get(gems?"UI_FORGE_GEMS":"UI_FLOAT_BAG")+"  "+entries.Length.ToString("N0")
                +(gems?"":"/"+_inventoryNotices.Capacity.ToString("N0"));
            int visibleCells=Mathf.Max(BagMinimumCells,((entries.Length+BagColumns-1)/BagColumns)*BagColumns);
            while(_bag.Count<visibleCells) { var cell=new ForgeCell(_bagContent,BagCellSize); _bag.Add(cell); }
            for(int i=0;i<_bag.Count;i++)
            {
                _bag[i].Root.gameObject.SetActive(i<visibleCells); if(i>=visibleCells)continue;
                ForgeUi.Place(_bag[i].Root,(i%BagColumns)*BagPitch,(i/BagColumns)*BagPitch,BagCellSize,BagCellSize);
                if(i>=entries.Length)
                {
                    _bag[i].Bind(null,null,empty:"");_bag[i].SetCompatibility(false,false);
                    // A cell that previously held loot must not retain a hover/drag payload.
                    Items.Source(_bag[i].Root,()=>null,()=>null);continue;
                }
                int index=i; var item=entries[i];
                bool picked=ActiveSide==SideDisenchant?_disenchantBatch.Contains(index):
                    gems?Model.Gems.Contains(item.def_id):ActiveSide==SideForge?Model.Gear.Any(p=>p.Index==index):InspectedOwner==0&&InspectedIndex==index;
                _bag[i].Bind(item,()=>
                {
                    if(!gems)MarkBagRead(index);
                    string raw=gems?null:_equipment?.Inventory?.FirstOrDefault(r=>r.Index==index)?.Raw;
                    Inspect(item,gems?-1:index,0,raw,false);
                    // Model selection raises Changed once. Do not rebuild the full UI before
                    // selection and again after the same event (formerly three refreshes/click).
                    if(ActiveSide==SideDisenchant)ToggleDisenchant(index,raw);
                    else if(ActiveSide==SideSocket&&_socket.GemFirst&&_socket.HasSelectedGem)
                    {
                        // A compatible equipment click completes the gem-first choice and
                        // returns the panel to its normal socket view for final confirmation.
                        _socket.SelectEquipmentAfterGem(); RefreshNow();
                    }
                    else if(ActiveSide==SideForge) { if(gems)Model.PickGem(item.def_id);else Model.PickGear(index); }
                    else RefreshNow();
                },picked,gems,gems?Model.Gems.Count(id=>id==item.def_id):0);
                _bag[i].SetNew(!gems&&_inventoryNotices.IsNew(index));
                bool socketFilter=ActiveSide==SideSocket&&_socket.GemFirst&&_socket.HasSelectedGem;
                _bag[i].SetCompatibility(socketFilter,!socketFilter||_socket.IsCompatibleInventory(index));
            }
            LayoutBag(entries,gems);
            if(Inspected!=null&&InspectedIndex>=0)
            {
                var source=InspectedOwner==0?_equipment?.Inventory:_equipment?.Candidates;
                var row=source?.FirstOrDefault(r=>r.Index==InspectedIndex&&r.OwnerId==InspectedOwner);
                if(row?.Raw!=InspectedRaw) { Inspected=null; InspectedIndex=-1; InspectedRaw=null; }
            }
            var selected=_equipment?.Inventory?.FirstOrDefault(r=>r.Index==InspectedIndex&&InspectedOwner==0&&r.Raw==InspectedRaw);
            bool gear=selected!=null&&(selected.Item.kind=="Weapon"||selected.Item.kind=="Armor");
            bool normal=ActiveSide==SideNone||ActiveSide==SideTraining||ActiveSide==SideCrests;
            _bagEquip.gameObject.SetActive(normal);_bagEquip.interactable=normal&&ItemsAvailable&&gear&&ExemplarEquipmentActions.EquipIssue(_equipment,selected)==null;
            _notice.text=Time.unscaledTime<_autoEquipMessageUntil?_autoEquipMessage:Inspected!=null?GameText.ItemName(Inspected):Loc.Get("UI_REVIEW_BAG_HINT");
            RefreshCrests();
            BindItemInteractions(entries,gems);Items.Refreshed();
        }
        void LayoutBag(ItemView[] entries,bool gems)
        {
            var order=Enumerable.Range(0,entries.Length).Where(i=>gems||_bagFilter==0
                ||(_bagFilter==1&&(entries[i].kind=="Weapon"||entries[i].kind=="Armor"))
                ||(_bagFilter==2&&Model.IsGem(entries[i]))
                ||(_bagFilter==3&&entries[i].kind!="Weapon"&&entries[i].kind!="Armor"&&!Model.IsGem(entries[i]))).ToList();
            if(_bagSort)order=order.OrderByDescending(i=>entries[i].grade_step).ThenByDescending(i=>entries[i].level_req).ThenBy(i=>i).ToList();
            for(int i=0;i<entries.Length;i++)_bag[i].Root.gameObject.SetActive(order.Contains(i));
            for(int i=entries.Length;i<_bag.Count;i++)_bag[i].Root.gameObject.SetActive(false);
            for(int n=0;n<order.Count;n++){var cell=_bag[order[n]];cell.Root.gameObject.SetActive(true);ForgeUi.Place(cell.Root,(n%BagColumns)*BagPitch,(n/BagColumns)*BagPitch,BagCellSize,BagCellSize);}
            // Only geometry is sorted. Every callback and drag payload keeps its native bag index.
            int cells=Mathf.Max(BagMinimumCells,((order.Count+BagColumns-1)/BagColumns)*BagColumns);
            for(int n=order.Count,k=entries.Length;n<cells&&k<_bag.Count;n++,k++){_bag[k].Root.gameObject.SetActive(true);ForgeUi.Place(_bag[k].Root,(n%BagColumns)*BagPitch,(n/BagColumns)*BagPitch,BagCellSize,BagCellSize);}
            UiScroll.Fit(_bagContent,Mathf.Max(0,(cells/BagColumns-1)*BagPitch+BagCellSize));
            _bagEmpty.gameObject.SetActive(order.Count==0);_bagEmpty.transform.SetAsLastSibling();
            SetToolHighlight(_bagSortButton,_bagSort);
            for(int i=0;i<_bagFilters.Count;i++){SetToolHighlight(_bagFilters[i],i==(gems?2:_bagFilter));_bagFilters[i].interactable=!gems;}
        }
        void EquipInspected()
        {
            var row=_equipment?.Inventory?.FirstOrDefault(r=>r.Index==InspectedIndex&&InspectedOwner==0&&r.Raw==InspectedRaw);
            if(row==null||!ItemsAvailable)return;
            var fresh=FreshSnapshot(row);if(fresh==null){RefreshNow();return;}
            var choice=ExemplarEquipmentActions.FreezeEquip(fresh,row);
            var result=ExemplarEquipmentActions.Apply(Model.Driver,choice,_memberId);
            if(result!=ExemplarEquipmentActions.Result.Applied)ActionNotice(VerifiedInputResult.Stale);
            else { Inspected=null;InspectedIndex=-1;InspectedRaw=null; }
            RefreshNow();
        }
        void Inspect(ItemView item,int index,long owner,string raw,bool refresh=true)
        { Inspected=item; InspectedIndex=index; InspectedOwner=owner; InspectedRaw=raw; if(refresh)RefreshNow(); }
        // Call only after this window has successfully mutated the frozen target in place.
        public void ReloadInspected(int index,long owner)
        {
            var fresh = ExemplarEquipmentActions.Read(Model.Driver, _memberId);
            var rows = owner == 0 ? fresh.Inventory : fresh.Candidates;
            var row = rows.FirstOrDefault(r => r.Index == index && r.OwnerId == owner);
            if (row != null) Inspect(row.Item,row.Index,row.OwnerId,row.Raw,false);
        }
        void OnResult(ForgeModel.Receipt receipt)
        { RefreshNow(); }
        public void Confirm(string message,Action accepted)
        {
            Items.Cancel();CancelConfirmation();
            _confirm=ForgeUi.Panel(_frame,"Confirmation",0,0,WindowWidth,Height,new Color(.04f,.05f,.065f,.9f)).rectTransform;
            var confirmation=_confirm;
            float boxWidth=Mathf.Min(620,WindowWidth-80);
            var box=ForgeUi.Panel(_confirm,"ConfirmBox",(WindowWidth-boxWidth)*.5f,166,boxWidth,198,ForgeUi.Raised).rectTransform;
            DialogSurface.Apply(box.GetComponent<Image>());
            DialogSurface.CloseIcon(box,()=>{if(_confirm==confirmation)CancelConfirmation();});
            float w=box.rect.width;
            var messageView=ForgeUi.Rect(box,"MessageScroll",22,48,w-44,66);
            var messageContent=UiScroll.Create(messageView,out _, false);
            var copy=ForgeUi.Text(messageContent,"Warning",message,0,0,w-58,82,15); ForgeUi.Wrap(copy);
            float messageHeight=Mathf.Max(82,copy.GetPreferredValues(message,w-58,10000).y+8);
            copy.rectTransform.sizeDelta=new Vector2(w-58,messageHeight);UiScroll.Fit(messageContent,messageHeight);
            ForgeUi.Button(box,"UI_FLOAT_CANCEL",22,130,w/2-32,44,()=>{if(_confirm==confirmation)CancelConfirmation();});
            ForgeUi.Button(box,"UI_FLOAT_CONFIRM",w/2+10,130,w/2-32,44,()=>
            {
                if(_confirm!=confirmation)return;
                CancelConfirmation();accepted?.Invoke();RefreshNow();
            },true);
        }
        void CancelConfirmation()
        {
            if(_confirm!=null){_confirm.gameObject.SetActive(false);Destroy(_confirm.gameObject);}
            _confirm=null;
        }
        void Arrange()
        {
            if(_frame==null)return;
            float width=WindowWidth; _frame.sizeDelta=new Vector2(width,Height);
            float left=FullStatsOpen?330:0;
            _main.anchoredPosition=new Vector2(left,0);
            if(_forge!=null)_forge.Root.anchoredPosition=new Vector2(left+MainWidth+10,0);
            if(_socket!=null)_socket.Root.anchoredPosition=new Vector2(left+MainWidth+10,0);
            if(_training!=null)_training.Root.anchoredPosition=new Vector2(left+MainWidth+10,0);
            if(_crests!=null)_crests.Root.anchoredPosition=new Vector2(left+MainWidth+10,0);
            if(_disenchantPanel!=null)_disenchantPanel.Root.anchoredPosition=new Vector2(left+MainWidth+10,0);
            float requestedScale=CompanionContext?_companionUiScale:Mathf.Max(.75f,Screen.height/720f*.90f);
            _canvas.scaleFactor=Mathf.Max(.1f,Mathf.Min(requestedScale,Mathf.Min((Screen.width-16)/width,(Screen.height-16)/Height)));
            if (CompanionContext)
                _frame.anchoredPosition = new Vector2(0, (_companionHeight / _canvas.scaleFactor - Height) * .5f - 8 / _canvas.scaleFactor);
            ClampPosition();
        }
        void RefreshToolHighlights()
        {
            SetToolHighlight(_blacksmithButton,ActiveSide==SideForge);
            SetToolHighlight(_trainingButton,ActiveSide==SideTraining);
            SetToolHighlight(_socketButton,ActiveSide==SideSocket);
            SetToolHighlight(_crestsButton,ActiveSide==SideCrests);
            SetToolHighlight(_disenchantButton,ActiveSide==SideDisenchant);
            SetToolHighlight(_fullStatsButton,FullStatsOpen);
        }
        static void SetToolHighlight(Button button,bool selected)
        {
            if(button==null)return;
            button.targetGraphic.color=selected?new Color(.34f,.30f,.21f):ForgeUi.Raised;
            var border=button.transform.Find("Border")?.GetComponent<Image>();
            if(border!=null)border.color=selected?ForgeUi.Gold:new Color(.32f,.34f,.37f);
            var label=button.GetComponentInChildren<TextMeshProUGUI>();
            if(label!=null)label.color=selected?ForgeUi.Gold:ForgeUi.Ink;
        }
        void RememberPosition() { if(_frame==null)return; if(CompanionContext)_companionPosition=_frame.anchoredPosition;else _battlePosition=_frame.anchoredPosition; }
        void ClampPosition()
        {
            if(_frame==null)return; float s=Mathf.Max(.1f,_canvas.scaleFactor);
            float margin=CompanionContext?8/s:8;
            var max=new Vector2(Mathf.Max(0,(Screen.width/s-_frame.rect.width)*.5f-margin),Mathf.Max(0,(Screen.height/s-Height)*.5f-margin));
            _frame.anchoredPosition=new Vector2(Mathf.Clamp(_frame.anchoredPosition.x,-max.x,max.x),Mathf.Clamp(_frame.anchoredPosition.y,-max.y,max.y));
        }
        void Update()
        {
            if(!UiInputFocus.Typing&&!UiInputFocus.Modal&&CompanionContext&&Keyboard.current?.iKey.wasPressedThisFrame==true)Toggle();
            if(!UiInputFocus.Typing&&!UiInputFocus.Modal&&CompanionContext&&Keyboard.current?.escapeKey.wasPressedThisFrame==true)Back();
            if(IsOpen)Arrange();
            if(Time.unscaledTime>=_nextRefresh)
            {
                _nextRefresh=Time.unscaledTime+.4f;
                if(IsOpen)RefreshNow();
                else
                {
                    var session=Model?.Driver?.Session;
                    if(session!=null&&session.IsAlive)
                        _inventoryNotices.Observe(session,ForgeModel.Items(session.QueryJson("inventory")),session.ViewJson());
                }
            }
        }
    }
}
