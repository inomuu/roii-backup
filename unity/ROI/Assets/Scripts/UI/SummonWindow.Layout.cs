using System;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using static Roi.UI.GachaArt;
namespace Roi.UI {
 // Layout follows the user's Excavation reference (2026-11-01): serif type, a flat banner rail,
 // capsule wallets, a name plate with stars beside the featured art, and ornamented pull buttons.
 // Design space is 1314x737 and scales uniformly (FitExcavation).
 public sealed partial class SummonWindow {
  RectTransform _design,_resultHost;
  TextMeshProUGUI _crystals;
  TextMeshProUGUI _terms,_termsTitle;
  RectTransform _termsCard,_pityGroup,_probabilityAction,_detailsAction;
  Button _one,_ten;
  RawImage _featuredArt;
  RectTransform _regaliaArt;
  bool _catalogPreview,_requestInFlight;
  static readonly Color Hairline=new Color(1f,.96f,.86f,.30f), RailInk=new Color(.012f,.018f,.026f,.88f);
  const float TermsTop=342;
  void Build(RectTransform body) {
   _body=body;body.GetComponentInParent<UiWindow>()?.SetImmersive();
   var bg=UiTheme.Flat(body,"ExcavationBackground",Ink);bg.rectTransform.anchorMin=Vector2.zero;bg.rectTransform.anchorMax=Vector2.one;bg.rectTransform.offsetMin=bg.rectTransform.offsetMax=Vector2.zero;
   _design=Rect(body,"ExcavationDesign",0,0,1314,737);_design.anchorMin=_design.anchorMax=_design.pivot=new Vector2(.5f,.5f);_design.anchoredPosition=Vector2.zero;_design.gameObject.AddComponent<RectMask2D>();
   Picture(_design,"HomeUI/EclipseCitadel",0,0,1314,737,new Color(.15f,.18f,.24f,1),true);
   _featuredArt=Picture(_design,"CombatPortraits/Seraphine",560,30,740,836,Color.white,false);
   _regaliaArt=Rect(_design,"RegaliaArt",600,90,670,560);
   Picture(_regaliaArt,"Crests/MagicCircle",70,0,520,520,new Color(.40f,.36f,.55f,.45f),false);
   Insignia(_regaliaArt,"Regalia",175,90,320,320,new Color(.80f,.76f,.63f,.85f));
   // Readability: darken behind the copy column and along the bottom action row only.
   Fade(_design,214,0,520,737,true);Fade(_design,214,560,1100,177,false);
   var rail=UiTheme.Flat(_design,"BannerRail",RailInk);Place(rail.rectTransform,0,0,214,737);rail.raycastTarget=false;
   Rule(_design,213,70,1,620,Hairline);
   var back=Rect(_design,"Back",8,40,196,56);
   Text(back,"Chevron","‹",6,8,18,40,26,new Color(1f,.96f,.86f,.55f));
   Text(back,"Title",GameText.ScreenName("Summon"),24,10,170,40,26,Ivory);
   Hit(back,()=>UnityEngine.Object.FindFirstObjectByType<HudBar>()?.Toggle("Summon"));
   _banners=UiScroll.Create(Rect(_design,"Banners",18,150,182,540),out _,false);
   _wallet=Wallet(975,true);_crystals=Wallet(1113,false);
   _status=Text(_design,"Status","",250,606,260,44,14,UiTheme.TextMid,false);
   _resultHost=Rect(_design,"ResultOverlay",240,355,1028,243);
   var resultBg=UiTheme.Box(_resultHost,"ResultBacking",new Color(.025f,.04f,.065f,.97f));resultBg.rectTransform.anchorMin=Vector2.zero;resultBg.rectTransform.anchorMax=Vector2.one;resultBg.rectTransform.offsetMin=resultBg.rectTransform.offsetMax=Vector2.zero;
   _results=UiScroll.Create(Rect(_resultHost,"ResultScroll",18,18,992,207),out _);
   _resultHost.gameObject.SetActive(false);FitExcavation();
  }
  void OnRectTransformDimensionsChange()=>FitExcavation();
  void FitExcavation(){if(_design!=null&&_body!=null)_design.localScale=Vector3.one*Mathf.Max(.01f,Mathf.Min(_body.rect.width/1314f,_body.rect.height/737f));}
  // Capsule wallet: icon left, amount right-aligned; the Gem capsule ends in a round top-up button.
  TextMeshProUGUI Wallet(float x,bool ticket) {
   float w=ticket?126:164;
   var p=Rect(_design,ticket?"Coupons":"Crystals",x,28,w,38);
   var chrome=p.gameObject.AddComponent<ExcavationChrome>();chrome.raycastTarget=false;
   if(ticket)Ticket(p,14,10,26,19);
   else {
    var icon=Rect(p,"CurrencyIcon",14,9,20,20).gameObject.AddComponent<HomeCurrencyIcon>();
    icon.Kind="Gem";icon.color=HomeCurrencyIcon.GemColor;icon.raycastTarget=false;
   }
   float right=ticket?w-16:w-48;
   var t=Text(p,"Value","—",44,6,right-44,26,18,Ivory,false);t.alignment=TextAlignmentOptions.MidlineRight;
   t.textWrappingMode=TextWrappingModes.NoWrap;t.enableAutoSizing=true;t.fontSizeMin=12;t.fontSizeMax=18;
   if(!ticket){
    // 44x44 hit area around a 28px disc that sits inside the capsule.
    var plus=Rect(p,"CrystalStore",w-41,-3,44,44);
    var disc=Rect(plus,"Disc",8,8,28,28).gameObject.AddComponent<ExcavationChrome>();disc.Solid=true;disc.color=new Color(.95f,.93f,.88f,1);disc.raycastTarget=false;
    var icon=Rect(plus,"Plus",14,14,16,16).gameObject.AddComponent<HomeCurrencyIcon>();
    icon.Kind="Plus";icon.color=Ink;icon.raycastTarget=false;
    Hit(plus,()=>UnityEngine.Object.FindFirstObjectByType<HudBar>()?.Toggle("Crystal"));
   }
   return t;
  }
  void ShowConnect(){_catalogPreview=true;_stateJson=JsonUtility.ToJson(new State{banners=new[]{new Banner{id="preview-standard",kind="Standard",featured="exemplar_seraphine"},new Banner{id="preview-regalia",kind="Regalia"}}});Rebuild();}
  string BannerTitle(Banner b)=>Loc.Get(b.kind=="Regalia"?"UI_EXCAVATION_REGALIA":"UI_EXCAVATION_REGULAR");
  static string GoldText(object value)=>"<color=#"+ColorUtility.ToHtmlStringRGB(Gold)+">"+value+"</color>";
  void Rebuild() {
   if(_design==null)return;Clear();
   var st=JsonUtility.FromJson<State>(_stateJson)??new State();var banners=st.banners??Array.Empty<Banner>();
   _crystals.text=_catalogPreview?"—":(Num(st.tradeable)+Num(st.bound)).ToString("N0");
   if(banners.Length==0){_status.text=Loc.Get("UI_SUMMON_THE_SERVER_IS_OFFERING_NO");return;}
   var selected=Array.Find(banners,b=>b.id==_selected)??banners[0];_selected=selected.id;
   _wallet.text=_catalogPreview?"—":Tickets(selected,st).ToString("N0");
   bool seraphine=selected.featured=="exemplar_seraphine",regalia=selected.kind=="Regalia";
   if(_featuredArt!=null)_featuredArt.gameObject.SetActive(seraphine);_regaliaArt.gameObject.SetActive(regalia);
   int i=0;foreach(var banner in banners)BuildBannerChoice(banner,i++);
   UiScroll.Fit(_banners,banners.Length*98);
   // Copy column.
   var info=Owned(_design,"BannerDescription",250,26,440,300);
   Text(info,"Kind",BannerTitle(selected),0,0,420,30,20,Gold);
   var heading=Text(info,"Heading",Loc.Get(regalia?"UI_EXCAVATION_REGALIA_TITLE":"UI_EXCAVATION_TITLE").Replace("|","\n"),0,50,440,120,44,Ivory);
   heading.lineSpacing=-8;
   string summary=_catalogPreview?"":selected.has_pity&&selected.hard>0?string.Format(Loc.Get("UI_EXCAVATION_GUARANTEE"),GoldText(selected.hard)):Loc.Get("UI_EXCAVATION_NO_PITY");
   Text(info,"Summary",summary,0,184,250,52,16,Ivory).lineSpacing=10;
   // Only permanent banner types state a duration; the state carries no end date for others.
   if(selected.kind=="Standard"||regalia){var d=Text(info,"Duration",Loc.Get("UI_EXCAVATION_DURATION_PERMANENT"),0,250,300,28,18,Gold);d.characterSpacing=6;}
   Rule(info,-5,300,232,1,Hairline);
   BuildBannerTerms(selected);
   DetailsLink(selected);
   // Featured name plate.
   if(!string.IsNullOrEmpty(selected.featured)) {
    var plate=Owned(_design,"Featured",636,128,240,64);
    var tile=UiTheme.Flat(plate,"Tile",new Color(.56f,.66f,.79f,1));Place(tile.rectTransform,0,4,54,54);tile.raycastTarget=false;
    var glow=UiTheme.Flat(plate,"TileShade",new Color(.12f,.18f,.28f,.45f));Place(glow.rectTransform,0,31,54,27);glow.raycastTarget=false;
    var spark=Picture(plate,"GachaUI/Stars",9,13,36,36,Color.white,false);if(spark!=null)spark.uvRect=new Rect(0,0,53f/363f,1);
    Text(plate,"Name",seraphine?Loc.Get("CLASS_NAME_seraphine"):GameText.UnitName(selected.featured),66,-4,180,38,30,Ivory).textWrappingMode=TextWrappingModes.NoWrap;
    Picture(plate,"GachaUI/Stars",66,38,130,19,Color.white,false);
    Hit(plate,()=>BannerDetails(selected));
   }
   // Action row.
   InfoAction("Probability",529,Loc.Get("UI_EXCAVATION_PROBABILITY"),()=>Probability(selected));
   _one=PullAction(764,1);_ten=PullAction(1030,10);
   RenderActionState(Roi.Net.OnlineService.Instance.LoggedIn);
   if(_quoteCount>0)ShowConfirm(_quoteCount,_quoteCrystal);else if(!string.IsNullOrEmpty(_lastPulls))PresentRewards();
   if(_quoteCount==0&&string.IsNullOrEmpty(_lastPulls)&&!_requestInFlight)_status.text="";
  }
  // Flat banner card: scene + art, a left-side shade for the caption, small-caps category over a serif name.
  void BuildBannerChoice(Banner banner,int index) {
   bool active=banner.id==_selected,regalia=banner.kind=="Regalia";
   var card=Owned(_banners,"Banner_"+banner.id,0,index*98,178,84);
   var fill=UiTheme.Flat(card,"Fill",new Color(.05f,.07f,.10f,1));Place(fill.rectTransform,0,0,178,84);
   var scene=Rect(card,"Scene",0,0,178,84);scene.gameObject.AddComponent<RectMask2D>();
   Picture(scene,"HomeUI/EclipseCitadel",0,0,178,84,new Color(.30f,.34f,.42f,1),true);
   if(banner.featured=="exemplar_seraphine")
    Picture(scene,"CombatPortraits/Seraphine",58,-22,250,282,active?Color.white:new Color(.62f,.66f,.74f,1),false);
   else Insignia(scene,regalia?"Regalia":"Exemplar",92,4,76,76,active?new Color(.84f,.80f,.68f,.75f):new Color(.62f,.60f,.54f,.45f));
   Fade(scene,0,0,178,84,true);Fade(scene,0,30,178,54,false);
   if(!active){var dim=UiTheme.Flat(card,"Dim",new Color(0,0,0,.28f));Place(dim.rectTransform,0,0,178,84);dim.raycastTarget=false;}
   var category=Text(card,"Type",Loc.Get(regalia?"UI_EXCAVATION_REGALIA":"UI_HOME_EXEMPLARS").ToUpperInvariant(),12,38,160,18,11,new Color(1f,.96f,.86f,active?.72f:.55f),false);
   category.characterSpacing=6;category.textWrappingMode=TextWrappingModes.NoWrap;
   string name=regalia?(string.IsNullOrEmpty(banner.featured)?Loc.Get("UI_SHOP_TAB_REGALIA"):RewardName(banner.featured)):BannerTitle(banner);
   var label=Text(card,"Name",name,12,54,162,26,17,active?Ivory:new Color(.86f,.84f,.78f,1));
   label.textWrappingMode=TextWrappingModes.NoWrap;label.enableAutoSizing=true;label.fontSizeMin=13;label.fontSizeMax=17;
   var frame=Rect(card,"Frame",0,0,178,84);Border(frame,active?new Color(1f,.96f,.86f,.55f):new Color(1f,.96f,.86f,.14f),2);
   if(active)Rule(card,0,0,3,84,Ivory);
   var choice=Hit(card,()=>{if(_requestInFlight||_quoteCount>0||!string.IsNullOrEmpty(_pendingKey))return;_selected=banner.id;Rebuild();RevealArt();});
   choice.targetGraphic=fill;
  }
  // Below the divider: connection/locked notice, or the pity ladder. No card chrome.
  void BuildBannerTerms(Banner banner) {
   var card=_termsCard=Owned(_design,"BannerTerms",250,TermsTop,232,90);
   _termsTitle=Text(card,"NoticeTitle","",0,0,232,26,17,Ivory);
   _terms=Text(card,"Terms","",0,28,232,56,15,UiTheme.TextMid,false);
   _pityGroup=null;
   if(!_catalogPreview&&banner.has_pity&&banner.hard>0) {
    _pityGroup=Rect(card,"Pity",0,0,232,30);
    int count=PityCount(banner.kind);
    Rule(_pityGroup,0,14,160,3,new Color(1f,.96f,.86f,.16f));
    Rule(_pityGroup,0,14,160*Mathf.Clamp01((float)count/banner.hard),3,Gold);
    Text(_pityGroup,"Progress",count+" / "+banner.hard,168,2,64,26,16,Ivory).alignment=TextAlignmentOptions.Right;
   }
  }
  void DetailsLink(Banner banner) {
   var r=_detailsAction=Owned(_design,"CharacterDetails",246,TermsTop,150,44);
   var t=Text(r,"Label",Loc.Get("UI_EXCAVATION_DETAILS")+"  ›",4,10,146,26,17,Gold);t.textWrappingMode=TextWrappingModes.NoWrap;
   var b=Hit(r,()=>BannerDetails(banner));UiButtonFeedback.Add(b);
  }
  static void Insignia(Transform parent,string kind,float x,float y,float w,float h,Color tint) {
   var r=Rect(parent,kind+"Insignia",x,y,w,h);var icon=r.gameObject.AddComponent<HomeIcon>();icon.Kind=kind;icon.color=tint;icon.raycastTarget=false;
  }
  void InfoAction(string name,float x,string label,Action action) {
   var r=_probabilityAction=Owned(_design,name,x,652,220,44);
   var skin=Panel(r,"Skin",0,0,220,44,new Color(.035f,.05f,.075f,.92f),10);
   Border(skin.rectTransform,new Color(1f,.96f,.86f,.42f),10);
   var text=Text(r,"Label",label,8,6,204,32,19,Ivory);
   text.alignment=TextAlignmentOptions.Center;text.enableAutoSizing=true;text.fontSizeMin=14;text.fontSizeMax=19;text.textWrappingMode=TextWrappingModes.NoWrap;
   var button=Hit(r,action);button.targetGraphic=skin;
   UiButtonFeedback.Add(button);
  }
  Button PullAction(float x,int count) {
   var r=Owned(_design,"Pull"+count,x,642,248,64);
   var skin=r.gameObject.AddComponent<ExcavationButtonGraphic>();skin.color=Color.white;
   var label=Text(r,"Label",string.Format(Loc.Get("UI_EXCAVATION_PULL"),count),36,6,176,30,21,Ivory);
   label.alignment=TextAlignmentOptions.Center;label.textWrappingMode=TextWrappingModes.NoWrap;
   var cost=Text(r,"Cost","×"+count,0,37,50,20,14,Ivory,false);
   cost.alignment=TextAlignmentOptions.MidlineLeft;
   float costWidth=cost.GetPreferredValues(cost.text).x;
   float costLeft=(248-(24+6+costWidth))*.5f;
   Ticket(r,costLeft,38,24,18);
   Place(cost.rectTransform,costLeft+30,37,costWidth+2,20);
   var hint=Text(r,"ActionHint","",0,-26,248,24,13,UiTheme.TextHigh,false);hint.alignment=TextAlignmentOptions.Center;hint.enableAutoSizing=true;hint.fontSizeMin=11;hint.fontSizeMax=13;
   var button=Hit(r,()=>{if(_requestInFlight||_catalogPreview||!string.IsNullOrEmpty(_lastPulls))return;UiConfirm.AskExcavation(count,()=>Roll(count,false,0));});button.targetGraphic=skin;
   var colors=button.colors;colors.disabledColor=new Color(.62f,.62f,.62f,1);colors.fadeDuration=.12f;button.colors=colors;
   UiButtonFeedback.Add(button);
   return button;
  }
  void SetPullEnabled(bool value){if(_one!=null)_one.interactable=value;if(_ten!=null)_ten.interactable=value;}
  void Probability(Banner b){string body=_catalogPreview?Loc.Get("UI_GACHA_UNAVAILABLE"):string.Format(Loc.Get("UI_EXCAVATION_BASE_RATE"),b.base_bp/100f)+"\n"+(b.has_pity?string.Format(Loc.Get("UI_EXCAVATION_GUARANTEE"),b.hard):Loc.Get("UI_EXCAVATION_NO_PITY"));ShowInformation(Loc.Get("UI_EXCAVATION_PROBABILITY"),body);}
  void ShowInformation(string title,string body) {
   UiConfirm.ExcavationSheet(title,host=>{
    var list=UiScroll.Create(host,out _);
    var copy=Text(list,"Information",body,0,0,872,154,22,UiTheme.TextHigh);
    copy.textWrappingMode=TextWrappingModes.Normal;
    float height=Mathf.Max(154,copy.GetPreferredValues(body,872,Mathf.Infinity).y+24);
    copy.rectTransform.sizeDelta=new Vector2(872,height);UiScroll.Fit(list,height);
   },sheetHeight:360);
  }
  void RevealArt() {
   var target=_featuredArt!=null&&_featuredArt.gameObject.activeSelf?_featuredArt.rectTransform:_regaliaArt;
   if(target!=null&&target.gameObject.activeSelf){var reveal=target.GetComponent<ExcavationReveal>();if(reveal==null)reveal=target.gameObject.AddComponent<ExcavationReveal>();reveal.Play();}
  }
  RectTransform Owned(Transform p,string name,float x,float y,float w,float h){var r=Rect(p,name,x,y,w,h);_built.Add(r.gameObject);return r;}
  static void Rule(Transform p,float x,float y,float w,float h,Color c){var a=UiTheme.Flat(p,"Rule",c);Place(a.rectTransform,x,y,w,h);a.raycastTarget=false;}
  static RawImage Picture(Transform p,string resource,float x,float y,float w,float h,Color tint,bool cover){var tex=Resources.Load<Texture2D>(resource);if(tex==null)return null;var r=Rect(p,resource,x,y,w,h);var im=r.gameObject.AddComponent<RawImage>();im.texture=tex;im.color=tint;im.raycastTarget=false;if(cover)r.gameObject.AddComponent<HomeImageCrop>();return im;}
 }
}
