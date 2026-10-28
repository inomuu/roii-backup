using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using static Roi.UI.GachaArt;
namespace Roi.UI {
 public sealed partial class SummonWindow {
  UiConfirm _rewardModal;
  Banner CurrentBanner()=>Array.Find((JsonUtility.FromJson<State>(_stateJson)??new State()).banners??Array.Empty<Banner>(),b=>b.id==_selected);
  long Tickets(Banner b,State state)=>b!=null&&!string.IsNullOrEmpty(b.ticket_kind)?b.tickets:Num(state.coupons);
  bool BannerOpen(Banner b)=>b!=null&&(string.IsNullOrEmpty(b.ticket_kind)||b.open);
  void RenderActionState(bool online) {
   var b=CurrentBanner();var state=JsonUtility.FromJson<State>(_stateJson)??new State();
   RenderBannerTerms(b,online);
   foreach(var button in new[]{_one,_ten}) {
    if(button==null)continue;int count=button==_one?1:10;
    bool ready=online&&!_catalogPreview&&BannerOpen(b)&&!_requestInFlight&&_quoteCount==0&&string.IsNullOrEmpty(_lastPulls)&&(string.IsNullOrEmpty(_pendingKey)||_pendingCount==count);
    button.interactable=ready;
    var label=button.transform.Find("Label")?.GetComponent<TextMeshProUGUI>();
    if(label!=null){label.text=Loc.Get(_requestInFlight?"UI_EXCAVATION_WAIT":"UI_EXCAVATION_EXCAVATE");label.enableAutoSizing=true;label.fontSizeMin=14;label.fontSizeMax=22;label.color=ready?(count==10?UiTheme.Accent:UiTheme.TextHigh):UiTheme.TextMid;}
    var ticket=button.transform.Find("Ticket")?.GetComponent<HomeCurrencyIcon>();
    if(ticket!=null)ticket.color=ready?HomeCurrencyIcon.TicketColor:new Color(.58f,.57f,.52f,1);
    var hint=button.transform.Find("ActionHint")?.GetComponent<TextMeshProUGUI>();
    if(hint!=null) {
     // Page availability is explained once in the left panel. Only a cost difference
     // belongs beside an individual pull; the wallet already communicates enough tickets.
     hint.text=ready&&Tickets(b,state)<count?string.Format(Loc.Get("UI_EXCAVATION_SHORT"),count-Tickets(b,state)):"";
     hint.color=UiTheme.TextMid;
    }
   }
  }
  void RenderBannerTerms(Banner banner,bool online) {
   if(_termsCard==null||_terms==null)return;
   bool disconnected=!online||_catalogPreview;
   bool locked=!disconnected&&!BannerOpen(banner);
   bool notice=disconnected||locked;
   bool progress=!notice&&banner!=null&&banner.has_pity&&banner.hard>0;
   float height=notice||progress?120:80;
   _termsCard.sizeDelta=new Vector2(376,height);
   _termsTitle.gameObject.SetActive(notice);
   _guaranteeDetail.gameObject.SetActive(!notice);
   if(_pityGroup!=null)_pityGroup.gameObject.SetActive(progress);
   _termsTitle.text=Loc.Get(disconnected?"UI_NOTICE_CONNECTION":"UI_EXCAVATION_LOCKED");
   _terms.text=disconnected?Loc.Get("UI_EXCAVATION_CONNECTION_HELP"):
     locked?Loc.Get("UI_EXCAVATION_LOCKED_HELP"):
     progress?string.Format(Loc.Get("UI_EXCAVATION_GUARANTEE"),banner.hard):Loc.Get("UI_EXCAVATION_NO_PITY");
   _terms.fontSize=notice?16:18;
   _terms.color=notice?UiTheme.TextMid:UiTheme.TextHigh;
   Place(_terms.rectTransform,22,notice?48:19,notice?332:289,notice?56:50);
   foreach(var action in new[]{_probabilityAction,_detailsAction}) {
    if(action!=null)action.anchoredPosition=new Vector2(action.anchoredPosition.x,-(342+height+18));
   }
  }
  void BannerDetails(Banner b) {
   UiConfirm.ExcavationSheet(Loc.Get("UI_EXCAVATION_DETAILS"),host=>{
    var list=UiScroll.Create(host,out _);float y=0;
    void Line(string value,float size,Color color) { var text=Text(list,"Detail",value,0,y,872,42,size,color);float h=Mathf.Max(36,text.GetPreferredValues(value,872,Mathf.Infinity).y+12);text.rectTransform.sizeDelta=new Vector2(872,h);y+=h; }
    Line(BannerTitle(b),24,Gold);
    Line(_catalogPreview?Loc.Get("UI_GACHA_UNAVAILABLE"):string.Format(Loc.Get("UI_EXCAVATION_BASE_RATE"),b.base_bp/100f),18,Ivory);
    Line(Loc.Get("UI_EXCAVATION_SSR_POOL"),20,Gold);
    var pool=new List<string>(b.pool??Array.Empty<string>());
    if(!string.IsNullOrEmpty(b.featured)&&!pool.Contains(b.featured))pool.Insert(0,b.featured);
    if(_catalogPreview||pool.Count==0)Line(Loc.Get("UI_EXCAVATION_POOL_UNAVAILABLE"),18,Ivory);
    else foreach(string id in pool)Line("•  "+RewardName(id),19,Ivory);
    Line(Loc.Get("UI_EXCAVATION_POOL_NOTE"),16,new Color(.68f,.73f,.80f));
    if(!string.IsNullOrEmpty(b.featured)) {string lore=Loc.GetOr("ITEM_LORE_"+b.featured,"");if(!string.IsNullOrEmpty(lore)){y+=18;Line(RewardName(b.featured),22,Gold);Line(lore,18,Ivory);}}
    UiScroll.Fit(list,y+12);
   });
  }
  static string RewardName(string id)=>GameText.ItemName(id,GameText.UnitName(id!=null&&id.StartsWith("exemplar_")?id.Substring(9):id));
  void PresentRewards() {
   if(string.IsNullOrEmpty(_lastPulls)||_rewardModal!=null)return;
   var result=JsonUtility.FromJson<RollResult>(_lastPulls);var pulls=result?.pulls??Array.Empty<PullRow>();
   var rewardCards = new List<RectTransform>();
   _rewardModal=UiConfirm.ExcavationSheet(Loc.Get("UI_EXCAVATION_RESULTS"),host=>{
    Text(host,"Collection",string.IsNullOrEmpty(_collected)?Loc.Get("UI_EXCAVATION_RESULT_RECORDED"):_collected,0,0,908,40,17,Ivory);
    var list=UiScroll.Create(Rect(host,"RewardScroll",0,48,908,326),out _);int index=0;
    if(pulls.Length==0)Text(list,"Empty",Loc.Get("UI_EXCAVATION_NO_RESULTS"),0,0,870,60,20,Ivory);
    foreach(var row in pulls) {
     bool solo=pulls.Length==1;
     int col=index%5,r=index/5;float width=solo?600:166,height=solo?280:148;
     var cell=Rect(list,"Reward_"+index,solo?154:(pulls.Length<5?(908-pulls.Length*178)/2:6)+col*178,r*160,width,height);index++;rewardCards.Add(cell);
     var surface=Panel(cell,"Fill",0,0,width,height,new Color(.06f,.085f,.13f,1),8);
     UiTheme.Border(surface.rectTransform,UiTheme.LineStrong);
     var accent=row.rarity=="Ssr"?Gold:row.rarity=="Sr"?new Color(.73f,.58f,.9f):new Color(.55f,.69f,.8f);
     Rule(cell,0,0,width,2,accent);
     string unit=row.def_id??"";
     if(unit=="exemplar_seraphine") {
      var art=Rect(cell,"Portrait",solo?55:38,solo?18:8,solo?210:90,solo?238:68).gameObject.AddComponent<RawImage>();art.texture=Resources.Load<Texture2D>("CombatPortraits/Seraphine");art.uvRect=solo?new Rect(.15f,.24f,.70f,.70f):new Rect(.32f,.63f,.4f,.30f);art.raycastTarget=false;
     } else {string icon=unit.StartsWith("regalia_")?"ExemplarUI/GearSymbols/Sword":"ExemplarUI/GearSymbols/Aura";Picture(cell,icon,solo?90:57,solo?74:17,solo?130:52,solo?130:52,accent,false);}
     Text(cell,"Rarity",(row.rarity??"").ToUpperInvariant(),solo?310:10,solo?80:76,solo?265:146,30,solo?22:15,accent,false).alignment=TextAlignmentOptions.Center;
     var name=Text(cell,"Name",RewardName(unit),solo?310:10,solo?125:101,solo?265:146,solo?100:40,solo?30:17,Ivory);name.alignment=TextAlignmentOptions.Center;name.enableAutoSizing=true;name.fontSizeMin=13;name.fontSizeMax=solo?30:17;
    }
    UiScroll.Fit(list,Mathf.Max(pulls.Length==1?296:80,((pulls.Length+4)/5)*160));
   },()=>{_rewardModal=null;_lastPulls="";_collected="";if(this!=null&&isActiveAndEnabled)Rebuild();});
   if(_rewardModal!=null)_rewardModal.gameObject.AddComponent<ExcavationResultSequence>().Begin(_rewardModal,rewardCards);
  }
 }
}
