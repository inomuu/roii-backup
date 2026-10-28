using System;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class GemSocketPanel
    {
        [Serializable] public sealed class Line { public string key,lo,hi; }
        [Serializable] public sealed class Held { public string def_id,name,set,refusal; public int count,grade_step; public bool fits; public Line[] lines; }
        [Serializable] public sealed class Socketed { public string def_id,name; public int grade_step; public string[] lines; }
        [Serializable] public sealed class Plan { public int gem_slots,card_slots,next_socket; public Held[] held; public Socketed[] socketed; }
        public readonly RectTransform Root;
        readonly FloatingInventoryHost _host;
        readonly TextMeshProUGUI _title,_cardSummary,_gemSummary,_current,_status;
        readonly RectTransform _content;
        readonly List<Button> _slots=new(),_held=new();
        readonly Dictionary<int,bool> _inventoryCompatibility=new();
        readonly Dictionary<string,bool> _equippedCompatibility=new();
        Button _modeButton;
        Plan _plan; string _targetKey,_compatibilitySignature,_selectedGemId; int _socket;
        public bool GemFirst { get; private set; }
        public bool HasSelectedGem => GemFirst&&!string.IsNullOrEmpty(_selectedGemId);
        public GemSocketPanel(Transform parent,FloatingInventoryHost host)
        {
            _host=host; Root=ForgeUi.Panel(parent,"SocketGems",510,0,320,560,ForgeUi.Ground).rectTransform;
            UiTheme.Border(Root,new Color(.48f,.43f,.32f),2);
            ForgeUi.Heading(ForgeUi.Copy(Root,"UI_FLOAT_SOCKET",14,4,260,30,17,ForgeUi.Ink),22);
            ForgeUi.Button(Root,"UI_FLOAT_CLOSE",286,5,26,26,()=>host.Side(FloatingInventoryHost.SideNone));
            _title=ForgeUi.Text(Root,"Equipment","",14,47,180,42,14);ForgeUi.Wrap(_title);
            _modeButton=ForgeUi.Button(Root,"UI_FLOAT_SOCKET_PICK_GEM",204,51,102,28,ToggleGemFirst);
            _cardSummary=ForgeUi.Text(Root,"CardSlotCount","",14,94,136,22,12,new Color(.95f,.79f,.42f,1));
            _gemSummary=ForgeUi.Text(Root,"GemSlotCount","",158,94,148,22,12,new Color(.45f,.84f,.76f,1));
            _current=ForgeUi.Text(Root,"CurrentGem","",14,167,290,85,12);ForgeUi.Wrap(_current);
            var view=ForgeUi.Rect(Root,"GemList",12,260,296,227);_content=UiScroll.Create(view,out _);
            _status=ForgeUi.Copy(Root,"UI_FLOAT_SOCKET_HELP",14,500,292,48,12,UiTheme.TextMid);ForgeUi.Wrap(_status);
        }
        string Key=>_host.InspectedOwner==0?"gem_plan:inv:"+_host.InspectedIndex:"gem_plan:eq:"+_host.InspectedOwner+","+_host.InspectedIndex;
        public bool IsCompatibleInventory(int index)
            => !HasSelectedGem || _inventoryCompatibility.TryGetValue(index,out var ok) && ok;
        public bool IsCompatibleEquipment(ItemView item,int index,long owner)
        {
            if(!HasSelectedGem||item==null)return !HasSelectedGem;
            if(owner==0)return IsCompatibleInventory(index);
            var key=owner+":"+index;
            if(_equippedCompatibility.TryGetValue(key,out var cached))return cached;
            var session=_host.Model.Driver?.Session;
            var plan=session==null?null:ForgeModel.Parse<Plan>(session.QueryJson("gem_plan:eq:"+owner+","+index));
            var fits=plan?.held?.Any(g=>g.def_id==_selectedGemId&&g.fits&&g.count>0)==true;
            _equippedCompatibility[key]=fits; return fits;
        }
        void ToggleGemFirst()
        {
            GemFirst=!GemFirst;_selectedGemId=null;_targetKey=null;_plan=null;_compatibilitySignature=null;_inventoryCompatibility.Clear();_equippedCompatibility.Clear();_host.RefreshNow();
        }
        public void SelectGemFirst(string defId)
        {
            if(string.IsNullOrEmpty(defId))return;
            GemFirst=true;_selectedGemId=defId;_compatibilitySignature=null;_inventoryCompatibility.Clear();_equippedCompatibility.Clear();_host.RefreshNow();
        }
        public void SelectEquipmentAfterGem()
        {
            if(!GemFirst)return;
            GemFirst=false;_targetKey=null;_compatibilitySignature=null;_inventoryCompatibility.Clear();_equippedCompatibility.Clear();_socket=0;
        }
        public void Bind()
        {
            if(!Root.gameObject.activeSelf)return;
            _equippedCompatibility.Clear();
            var session=_host.Model.Driver?.Session;
            _modeButton.GetComponentInChildren<TMP_Text>().text=Loc.Get(GemFirst?"UI_FLOAT_SOCKET_PICK_EQUIPMENT":"UI_FLOAT_SOCKET_PICK_GEM");
            if(GemFirst){BindGemFirst(session);return;}
            var item=_host.Inspected;string target=item==null||_host.InspectedIndex<0?null:Key;
            if(target!=_targetKey){_targetKey=target;_socket=0;}
            _plan=target==null?null:ForgeModel.Parse<Plan>(session?.QueryJson(target));
            _title.text=item==null?Loc.Get("UI_FLOAT_SELECT"):ForgeUi.Name(item);
            _cardSummary.gameObject.SetActive(true);_gemSummary.gameObject.SetActive(true);
            int n=_plan?.gem_slots??0;
            _cardSummary.text=_plan==null?"":Loc.Fmt("UI_FLOAT_CARD_SLOTS",_plan.card_slots);
            _gemSummary.text=_plan==null?"":Loc.Fmt("UI_FLOAT_GEM_SLOTS",n);
            _socket=Mathf.Clamp(_socket,0,Mathf.Max(0,n-1));
            BindSlots(n);
            var current=_plan?.socketed!=null&&_socket<_plan.socketed.Length?_plan.socketed[_socket]:null;
            _current.text=current==null?Loc.Get(n>0?"UI_FLOAT_EMPTY_SOCKET":"UI_FLOAT_NO_SOCKET"):
                Loc.GetOr("ITEM_"+current.def_id,current.name)+"\n"+string.Join("\n",current.lines??Array.Empty<string>());
            _status.text=Loc.Get("UI_FLOAT_SOCKET_EQUIPMENT_FIRST_HELP");
            BindHeld(_plan?.held??Array.Empty<Held>(),n);
        }
        void BindGemFirst(SimSession session)
        {
            _title.text=Loc.Get("UI_FLOAT_SOCKET_PICK_GEM");_cardSummary.gameObject.SetActive(false);_gemSummary.gameObject.SetActive(false);
            foreach(var button in _slots)button.gameObject.SetActive(false);
            RebuildCompatibility(session);
            var material=_host.Model.HeldGems.FirstOrDefault(m=>m.def_id==_selectedGemId);
            _current.text=material==null?Loc.Get("UI_FLOAT_SOCKET_GEM_FIRST_HELP"):
                ForgeUi.Name(material)+"  ×"+material.quantity+"\n"+GameText.GradeLabel(material.grade_step);
            _status.text=Loc.Get("UI_FLOAT_SOCKET_GEM_FIRST_HELP");
            var list=_host.Model.HeldGems.Where(_host.Model.IsGem).ToArray();
            while(_held.Count<list.Length){var b=ForgeUi.Button(_content,"UI_FORGE_GEMS",0,0,282,82,null);_held.Add(b);}
            for(int i=0;i<_held.Count;i++)
            {
                var b=_held[i];b.gameObject.SetActive(i<list.Length);if(i>=list.Length)continue;var gem=list[i];
                ForgeUi.Place(b.GetComponent<RectTransform>(),0,i*88,282,82);
                var text=b.GetComponentInChildren<TMP_Text>();ForgeUi.Wrap(text);text.fontSize=11;text.margin=new Vector4(7,5,5,3);
                text.text=ForgeUi.Name(gem)+"  ×"+gem.quantity+"\n"+GameText.GradeLabel(gem.grade_step);
                bool selected=gem.def_id==_selectedGemId;b.GetComponent<Image>().color=selected?new Color(.34f,.30f,.21f):ForgeUi.Raised;
                b.interactable=!_host.Model.Busy;b.onClick.RemoveAllListeners();b.onClick.AddListener(()=>SelectGemFirst(gem.def_id));
                _host.Items.Source((RectTransform)b.transform,()=>_host.Payload(gem),()=>_host.DescribeItem(gem));
            }
            UiScroll.Fit(_content,list.Length*88);
        }
        void RebuildCompatibility(SimSession session)
        {
            if(!HasSelectedGem||session==null){_inventoryCompatibility.Clear();return;}
            var signature=_selectedGemId+"|"+session.QueryJson("inventory");
            if(signature==_compatibilitySignature)return;
            _compatibilitySignature=signature;_inventoryCompatibility.Clear();_equippedCompatibility.Clear();
            for(int i=0;i<_host.Model.Bag.Length;i++)
            {
                var item=_host.Model.Bag[i];bool fits=false;
                if(item!=null&&(item.kind=="Weapon"||item.kind=="Armor")&&item.gem_slots>0)
                {
                    var plan=ForgeModel.Parse<Plan>(session.QueryJson("gem_plan:inv:"+i));
                    fits=plan?.held?.Any(g=>g.def_id==_selectedGemId&&g.fits&&g.count>0)==true;
                }
                _inventoryCompatibility[i]=fits;
            }
        }
        void BindSlots(int n)
        {
            while(_slots.Count<n)
            {int i=_slots.Count;var button=ForgeUi.Button(Root,"UI_FORGE_GEMS",14+i*48,127,42,30,()=>{_socket=i;Bind();});_slots.Add(button);}
            for(int i=0;i<_slots.Count;i++)
            {
                int socket=i;var b=_slots[i];b.gameObject.SetActive(i<n);if(i>=n)continue;
                b.GetComponentInChildren<TMP_Text>().text=(i+1).ToString();b.GetComponent<Image>().color=i==_socket?new Color(.34f,.30f,.21f):ForgeUi.Raised;b.interactable=i<=(_plan?.next_socket??0);
                _host.Items.Target((RectTransform)b.transform,"socket:"+i,p=>_host.DragStillValid(p)&&p.Row==null&&socket<=_plan.next_socket
                    &&_plan.held?.Any(g=>g.def_id==p.Item.def_id&&g.fits&&g.count>0)==true,p=>
                {var gem=_plan.held.FirstOrDefault(g=>g.def_id==p.Item.def_id);if(gem!=null){_socket=socket;Choose(gem);}});
                var fitted=_plan.socketed!=null&&i<_plan.socketed.Length?_plan.socketed[i]:null;
                _host.Items.Source((RectTransform)b.transform,null,()=>fitted==null?Loc.Get("UI_FLOAT_EMPTY_SOCKET"):
                    Loc.GetOr("ITEM_"+fitted.def_id,fitted.name)+"\n"+GameText.GradeLabel(fitted.grade_step)+"\n"+string.Join("\n",fitted.lines??Array.Empty<string>()));
            }
        }
        void BindHeld(Held[] list,int n)
        {
            while(_held.Count<list.Length){var b=ForgeUi.Button(_content,"UI_FORGE_GEMS",0,0,282,82,null);_held.Add(b);}
            for(int i=0;i<_held.Count;i++)
            {
                var b=_held[i];b.gameObject.SetActive(i<list.Length);if(i>=list.Length)continue;var gem=list[i];
                ForgeUi.Place(b.GetComponent<RectTransform>(),0,i*88,282,82);
                var text=b.GetComponentInChildren<TMP_Text>();ForgeUi.Wrap(text);text.fontSize=11;text.margin=new Vector4(7,5,5,3);
                text.text=Loc.GetOr("ITEM_"+gem.def_id,gem.name)+"  ×"+gem.count+"\n"+string.Join("\n",(gem.lines??Array.Empty<Line>()).Select(l=>l.lo+" – "+l.hi));
                b.GetComponent<Image>().color=gem.fits?new Color(.18f,.42f,.25f,.92f):new Color(.16f,.18f,.22f,.48f);
                text.color=gem.fits?ForgeUi.Ink:new Color(.48f,.50f,.55f,1);b.interactable=gem.fits&&n>0&&!_host.Model.Busy;
                b.onClick.RemoveAllListeners();b.onClick.AddListener(()=>Choose(gem));
                var material=_host.Model.HeldGems.FirstOrDefault(m=>m.def_id==gem.def_id);
                _host.Items.Source((RectTransform)b.transform,()=>material==null?null:_host.Payload(material),
                    ()=>_host.DescribeItem(material)+(gem.fits?"":"\n"+Loc.Get("UI_FLOAT_GEM_UNSUITABLE")));
                if(!gem.fits)text.text+="\n"+Loc.Get("UI_FLOAT_GEM_UNSUITABLE");
            }
            UiScroll.Fit(_content,list.Length*88);
        }
        void Choose(Held gem)
        {
            var driver=_host.Model.Driver;var session=driver.Session;var target=Key;var raw=_host.InspectedRaw;
            int socket=_socket,index=_host.InspectedIndex;long owner=_host.InspectedOwner;
            string targetJson=owner==0?"{\"inventory\":{\"idx\":"+index+"}}":"{\"equipped\":{\"entity_id\":"+owner+",\"equipped_slot\":"+index+"}}";
            bool overwrite=_plan.socketed!=null&&socket<_plan.socketed.Length;
            _host.Confirm(Loc.Get(overwrite?"UI_FLOAT_REPLACE_WARNING":"UI_FLOAT_SOCKET_WARNING"),()=>
            {
                _host.RefreshNow();
                if(!ReferenceEquals(driver.Session,session)||raw!=_host.InspectedRaw||target!=Key){_status.text=Loc.Get("UI_FORGE_STALE");return;}
                var bag=ForgeModel.Items(session.QueryJson("inventory"));
                int before=bag.Where(m=>m.def_id==gem.def_id).Sum(m=>m.quantity);
                int resultIndex=index;
                // Socketing can remove a gem stack ahead of the selected bag item. Equipment
                // order is unchanged: resolve its ordinal in the native result, not a guessed
                // debit order or the original (now stale) bag index.
                int ordinal=bag.Take(index).Count(IsEquipment);
                string input="{\"SocketGem\":{\"target\":"+targetJson+",\"socket\":"+socket+",\"gem\":"+ForgeModel.QuoteString(gem.def_id)+"}}";
                var result=driver.ApplyVerifiedInputs(session,new[]{input},copy=>
                {
                    var afterBag=ForgeModel.Items(copy.QueryJson("inventory"));
                    if(owner==0)resultIndex=afterBag.Select((item,i)=>new{item,i}).Where(row=>IsEquipment(row.item))
                        .Skip(ordinal).Select(row=>row.i).DefaultIfEmpty(-1).First();
                    var p=ForgeModel.Parse<Plan>(copy.QueryJson(owner==0?"gem_plan:inv:"+resultIndex:target));
                    int after=afterBag.Where(m=>m.def_id==gem.def_id).Sum(m=>m.quantity);
                    return before>0&&after==before-1&&p?.socketed!=null&&p.socketed.Length>socket&&p.socketed[socket].def_id==gem.def_id;
                });
                if(result==VerifiedInputResult.Applied)_host.ReloadInspected(resultIndex,owner);
                _status.text=Loc.Get(result==VerifiedInputResult.Applied?"UI_FLOAT_SOCKET_DONE":"UI_FORGE_STALE");_host.RefreshNow();
            });
        }
        static bool IsEquipment(ItemView item)=>item!=null&&(item.kind=="Weapon"||item.kind=="Armor");
    }
}
