using System.Linq;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // One host-owned Crest at a time; presentation shared by battle and companion windows.
    public sealed class CrestPanel
    {
        public readonly RectTransform Root;
        readonly FloatingInventoryHost _host;
        readonly TMP_Text _name,_level,_effect,_current,_next,_held,_cost,_status,_artNote;
        readonly Button _upgrade;
        readonly Button[] _groups=new Button[4],_tiers=new Button[5];
        readonly CrestArtwork _art;
        static readonly string[] Categories={"weapon","armor","accessory","gem"};
        int _group,_tier=3;
        string _id,_notice;
        float _noticeUntil;
        public CrestPanel(Transform parent,FloatingInventoryHost host)
        {
            _host=host;Root=ForgeUi.Panel(parent,"Crests",630,0,320,560,ForgeUi.Ground).rectTransform;
            UiTheme.Border(Root,ForgeUi.Gold,1);
            ForgeUi.Heading(ForgeUi.Copy(Root,"UI_CRESTS",14,4,240,30,17,ForgeUi.Ink),22);
            ForgeUi.Button(Root,"UI_FLOAT_CLOSE",286,5,26,26,()=>host.Side(FloatingInventoryHost.SideNone));
            for(int i=0;i<5;i++)
            {
                int tier=i+3;
                _tiers[i]=ForgeUi.Button(Root,"UI_CRESTS",10+i*61,42,56,29,()=>{_tier=tier;ClearNotice();Bind();});
                _tiers[i].name="Tier"+tier;
                _tiers[i].GetComponentInChildren<TMP_Text>().text="T"+tier;
            }
            for(int i=0;i<4;i++)
            {
                int index=i;
                _groups[i]=ForgeUi.Button(Root,"UI_CREST_CATEGORY_"+Categories[i],10+i*76,79,72,27,()=>{_group=index;ClearNotice();Bind();});
                var text=_groups[i].GetComponentInChildren<TMP_Text>();Fit(text,12,9);
            }
            var stage=ForgeUi.Rect(Root,"CrestStage",10,114,300,232);
            _art=stage.gameObject.AddComponent<CrestArtwork>();_art.Build();
            _artNote=Label("ArtNote","",16,326,288,17,8,new Color(.67f,.73f,.79f));
            _name=Label("Name","",12,347,296,26,17,ForgeUi.Ink);ForgeUi.Heading(_name,22);Fit(_name,22,14);
            _level=Label("Level","",14,375,292,20,11,ForgeUi.Gold);
            ForgeUi.Panel(Root,"BonusWell",10,403,300,66,new Color(.045f,.058f,.085f));
            _effect=Label("Effect","",16,405,288,19,11,new Color(.72f,.78f,.84f));Fit(_effect,11,8);
            _current=Label("Current","",17,428,122,32,23,ForgeUi.Ink);Fit(_current,23,12);
            Label("Arrow","→",141,430,38,28,20,ForgeUi.Gold);
            _next=Label("Next","",181,428,122,32,23,new Color(.74f,.89f,.84f));Fit(_next,23,12);
            ForgeUi.Display(_current,25);Fit(_current,25,12);ForgeUi.Display(_next,25);Fit(_next,25,12);
            _held=Label("Fragments","",14,475,292,21,11,ForgeUi.Ink);Fit(_held,11,8);
            _upgrade=ForgeUi.Button(Root,"UI_CREST_UPGRADE_ACTION",12,503,296,34,Upgrade,true);
            _cost=_upgrade.GetComponentInChildren<TMP_Text>();Fit(_cost,13,9);
            _status=Label("CrestStatus","",12,540,296,15,9,ForgeUi.Gold);Fit(_status,9,7);
            Root.gameObject.SetActive(false);
        }
        TMP_Text Label(string name,string value,float x,float y,float w,float h,float size,Color color)
        { var t=ForgeUi.Text(Root,name,value,x,y,w,h,size,color);t.alignment=TextAlignmentOptions.Center;t.raycastTarget=false;return t; }
        static void Fit(TMP_Text text,float max,float min)
        {text.enableAutoSizing=true;text.fontSizeMax=max;text.fontSizeMin=min;text.textWrappingMode=TextWrappingModes.NoWrap;}
        void ClearNotice(){_noticeUntil=0;}
        public void Bind()
        {
            if(!Root.gameObject.activeInHierarchy)return;
            var rules=CrestModel.Read(_host.Model.Driver?.Session);
            var slot=rules?.crests?.FirstOrDefault(s=>s.category==Categories[_group]&&s.tier==_tier);
            for(int i=0;i<5;i++)Select(_tiers[i],i+3==_tier);
            for(int i=0;i<4;i++)Select(_groups[i],i==_group);
            _art.Bind(_group,_tier);
            _artNote.text=_art.HasArtwork(_tier)?"":Loc.Get("UI_CREST_ART_TEMP");
            _id=slot?.id;
            _name.text=slot==null?Loc.Get("UI_CREST_UNAVAILABLE"):Loc.Get("CREST_"+slot.id);
            _level.text=slot==null?"":Loc.Fmt("UI_CREST_LEVEL",slot.level,slot.max_level);
            var preview=CrestPresentation.Bonus(slot);
            _effect.text=preview.Label;_current.text=preview.Current;_next.text=preview.Next;
            ulong per=rules?.shards_per_material??1;
            _held.text=slot==null?"":Loc.Fmt("UI_CREST_HELD",CrestModel.Material(slot.category,slot.tier),CrestModel.Amount(slot.held,per));
            bool maxed=slot!=null&&slot.level>=slot.max_level;
            _upgrade.interactable=CrestModel.CanUpgrade(slot);
            _cost.text=slot==null?Loc.Get("UI_CREST_UNAVAILABLE"):maxed?Loc.Get("UI_CREST_MAXED"):
                Loc.Fmt("UI_CREST_UPGRADE_COST",CrestModel.Amount(slot.next_cost,per));
            _status.text=Time.unscaledTime<_noticeUntil?_notice:slot==null?"":maxed?Loc.Get("UI_CREST_MAXED"):
                Loc.Get(_upgrade.interactable?"UI_CREST_PARTY_BONUS":"UI_CREST_NEED_FRAGMENTS");
        }
        static void Select(Button button,bool selected)
        {
            button.targetGraphic.color=selected?new Color(.27f,.31f,.34f):new Color(.13f,.15f,.18f);
            button.GetComponentInChildren<TMP_Text>().color=selected?ForgeUi.Gold:new Color(.65f,.69f,.73f);
        }
        void Upgrade()
        {
            if(string.IsNullOrEmpty(_id))return;
            var result=CrestModel.Upgrade(_host.Model.Driver,_id);
            _notice=Loc.Get(result==Roi.Sim.VerifiedInputResult.Applied?"UI_CREST_DONE":"UI_TRAIN_STALE");
            _noticeUntil=Time.unscaledTime+5;_host.RefreshNow();
        }
    }
}
