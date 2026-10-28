using System;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using static Roi.UI.GachaArt;
namespace Roi.UI {
 public sealed partial class SummonWindow {
  RectTransform _design,_resultHost;
  TextMeshProUGUI _crystals,_ticketsLabel,_gemLabel;
  TextMeshProUGUI _terms,_termsTitle;
  RectTransform _termsCard,_guaranteeDetail,_pityGroup,_probabilityAction,_detailsAction;
  Button _one,_ten;
  RawImage _featuredArt;
  RectTransform _regaliaArt;
  bool _catalogPreview,_requestInFlight;
  void Build(RectTransform body) {
   _body=body;body.GetComponentInParent<UiWindow>()?.SetImmersive();
   var bg=UiTheme.Flat(body,"ExcavationBackground",Ink);bg.rectTransform.anchorMin=Vector2.zero;bg.rectTransform.anchorMax=Vector2.one;bg.rectTransform.offsetMin=bg.rectTransform.offsetMax=Vector2.zero;
   _design=Rect(body,"ExcavationDesign",0,0,1314,737);_design.anchorMin=_design.anchorMax=_design.pivot=new Vector2(.5f,.5f);_design.anchoredPosition=Vector2.zero;_design.gameObject.AddComponent<RectMask2D>();
   Picture(_design,"HomeUI/EclipseCitadel",0,0,1314,737,new Color(.22f,.26f,.28f,1),true);
   _featuredArt=Picture(_design,"CombatPortraits/Seraphine",535,22,750,847,new Color(.94f,.96f,1f,1),false);
   _regaliaArt=Rect(_design,"RegaliaArt",550,90,670,560);
   Picture(_regaliaArt,"Crests/MagicCircle",70,0,520,520,new Color(.34f,.39f,.47f,.45f),false);
   Insignia(_regaliaArt,"Regalia",175,90,320,320,new Color(.80f,.76f,.63f,.85f));
   Fade(_design,0,0,710,737,true);Fade(_design,214,490,1100,247,false);
   var rail=UiTheme.Flat(_design,"BannerRail",new Color(.018f,.027f,.031f,.76f));Place(rail.rectTransform,0,0,214,737);
   Rule(_design,213,70,1,625,new Color(.8f,.8f,.75f,.22f));
   var back=Rect(_design,"Back",16,35,180,52);Text(back,"Title","‹  "+GameText.ScreenName("Summon"),0,7,180,40,23,UiTheme.TextHigh);Hit(back,()=>UnityEngine.Object.FindFirstObjectByType<HudBar>()?.Toggle("Summon"));
   _banners=UiScroll.Create(Rect(_design,"Banners",14,116,188,579),out _,false);
   _wallet=Wallet(983,true);_crystals=Wallet(1120,false);
   _status=Text(_design,"Status","",248,594,455,42,14,UiTheme.TextHigh,false);
   _resultHost=Rect(_design,"ResultOverlay",240,355,1028,243);
   var resultBg=UiTheme.Box(_resultHost,"ResultBacking",new Color(.025f,.04f,.065f,.97f));resultBg.rectTransform.anchorMin=Vector2.zero;resultBg.rectTransform.anchorMax=Vector2.one;resultBg.rectTransform.offsetMin=resultBg.rectTransform.offsetMax=Vector2.zero;
   _results=UiScroll.Create(Rect(_resultHost,"ResultScroll",18,18,992,207),out _);
   _resultHost.gameObject.SetActive(false);FitExcavation();
  }
  void OnRectTransformDimensionsChange()=>FitExcavation();
  void FitExcavation(){if(_design!=null&&_body!=null)_design.localScale=Vector3.one*Mathf.Max(.01f,Mathf.Min(_body.rect.width/1314f,_body.rect.height/737f));}
  TextMeshProUGUI Wallet(float x,bool ticket) {
   var p=Rect(_design,ticket?"Coupons":"Crystals",x,24,ticket?125:158,50);p.gameObject.AddComponent<ExcavationChrome>();
   if(ticket)Ticket(p,11,14,30,23);
   else {
    var icon=Rect(p,"CurrencyIcon",11,12,26,26).gameObject.AddComponent<HomeCurrencyIcon>();
    icon.Kind="Gem";icon.color=HomeCurrencyIcon.GemColor;icon.raycastTarget=false;
   }
   var name=Text(p,"Name",Loc.Get(ticket?"UI_EXCAVATION_TICKETS":"UI_HOME_GEM"),46,2,ticket?64:58,21,12,UiTheme.TextMid,false);
   if(ticket)_ticketsLabel=name;else _gemLabel=name;
   var t=Text(p,"Value","—",46,20,ticket?64:58,30,18,UiTheme.TextHigh,false);t.alignment=TextAlignmentOptions.MidlineLeft;
   t.textWrappingMode=TextWrappingModes.NoWrap;t.enableAutoSizing=true;t.fontSizeMin=12;t.fontSizeMax=18;
   if(!ticket){
    var plus=Rect(p,"CrystalStore",110,3,44,44);
    var icon=Rect(plus,"Plus",11,11,22,22).gameObject.AddComponent<HomeCurrencyIcon>();
    icon.Kind="Plus";icon.color=UiTheme.TextHigh;icon.raycastTarget=false;
    Hit(plus,()=>UnityEngine.Object.FindFirstObjectByType<HudBar>()?.Toggle("Crystal"));
   }
   return t;
  }
  void ShowConnect(){_catalogPreview=true;_stateJson=JsonUtility.ToJson(new State{banners=new[]{new Banner{id="preview-standard",kind="Standard",featured="exemplar_seraphine"},new Banner{id="preview-regalia",kind="Regalia"}}});Rebuild();}
  string BannerTitle(Banner b)=>Loc.Get(b.kind=="Regalia"?"UI_EXCAVATION_REGALIA":"UI_EXCAVATION_REGULAR");
  void Rebuild() {
   if(_design==null)return;Clear();
   var st=JsonUtility.FromJson<State>(_stateJson)??new State();var banners=st.banners??Array.Empty<Banner>();
   _ticketsLabel.text=Loc.Get("UI_EXCAVATION_TICKETS");_gemLabel.text=Loc.Get("UI_HOME_GEM");
   _crystals.text=_catalogPreview?"—":(Num(st.tradeable)+Num(st.bound)).ToString("N0");
   if(banners.Length==0){_status.text=Loc.Get("UI_SUMMON_THE_SERVER_IS_OFFERING_NO");return;}
   var selected=Array.Find(banners,b=>b.id==_selected)??banners[0];_selected=selected.id;
   _wallet.text=_catalogPreview?"—":Tickets(selected,st).ToString("N0");
   bool seraphine=selected.featured=="exemplar_seraphine";
   if(_featuredArt!=null)_featuredArt.gameObject.SetActive(seraphine);_regaliaArt.gameObject.SetActive(selected.kind=="Regalia");
   int i=0;foreach(var banner in banners)BuildBannerChoice(banner,i++);
   UiScroll.Fit(_banners,banners.Length*174-22);
   var info=Owned(_design,"BannerDescription",260,100,430,300);
   Insignia(info,selected.kind=="Regalia"?"Regalia":"Summon",0,5,24,24,UiTheme.TextMid);
   Text(info,"Kind",BannerTitle(selected),34,0,356,35,18,UiTheme.TextMid);
   Rule(info,-10,46,1,168,new Color(.72f,.68f,.54f,.45f));
   Text(info,"Heading",Loc.Get(selected.kind=="Regalia"?"UI_EXCAVATION_REGALIA_TITLE":"UI_EXCAVATION_TITLE").Replace("|", "\n"),0,36,430,116,42,UiTheme.TextHigh);
   Text(info,"Summary",_catalogPreview?"":string.Format(Loc.Get("UI_EXCAVATION_BASE_RATE"),selected.base_bp/100f),0,169,366,50,18,UiTheme.TextHigh);
   BuildBannerTerms(selected);
   InfoAction("CharacterDetails",454,Loc.Get("UI_EXCAVATION_DETAILS"),()=>BannerDetails(selected));
   var name=Owned(_design,"Featured",660,88,290,100);
   if(!string.IsNullOrEmpty(selected.featured)) {
    Insignia(name,"Exemplar",-42,8,28,28,UiTheme.Accent);
    Text(name,"Name",seraphine?Loc.Get("CLASS_NAME_seraphine"):GameText.UnitName(selected.featured),0,0,242,46,34,UiTheme.TextHigh);
    if(seraphine)Text(name,"Rarity",Loc.Get("GRADE_4"),2,48,230,24,14,UiTheme.TextMid,false);
    Rule(name,0,79,170,1,new Color(.8f,.76f,.63f,.6f));
   }
   _one=PullAction(743,1);_ten=PullAction(1015,10);
   InfoAction("Probability",260,Loc.Get("UI_EXCAVATION_PROBABILITY"),()=>Probability(selected));
   RenderActionState(Roi.Net.OnlineService.Instance.LoggedIn);
   if(_quoteCount>0)ShowConfirm(_quoteCount,_quoteCrystal);else if(!string.IsNullOrEmpty(_lastPulls))PresentRewards();
   if(_quoteCount==0&&string.IsNullOrEmpty(_lastPulls)&&!_requestInFlight)_status.text="";
  }
  void BuildBannerChoice(Banner banner,int index) {
   bool active=banner.id==_selected,regalia=banner.kind=="Regalia";
   var card=Owned(_banners,"Banner_"+banner.id,0,index*174,184,152);
   var backing=Rect(card,"Ticket",0,24,184,128);
   var frame=backing.gameObject.AddComponent<ExcavationBannerSurface>();frame.Selected=active;frame.raycastTarget=false;
   // Clip the scenic inset to the cut-corner ticket, but let the portrait rise above its top edge.
   var scene=Rect(backing,"Scene",8,8,168,112);scene.gameObject.AddComponent<RectMask2D>();
   Picture(scene,"HomeUI/EclipseCitadel",0,0,168,112,new Color(.21f,.27f,.30f,1),true);
   var art=Rect(card,"Artwork",6,0,172,146);art.gameObject.AddComponent<RectMask2D>();
   if(banner.featured=="exemplar_seraphine")
    Picture(art,"CombatPortraits/Seraphine",-61,-16,340,384,active?Color.white:new Color(.78f,.83f,.90f,1),false);
   else {
    Picture(art,"Crests/MagicCircle",62,6,100,100,new Color(.47f,.55f,.64f,.18f),false);
    Insignia(art,regalia?"Regalia":"Exemplar",80,20,68,68,active?UiTheme.Accent:new Color(.76f,.74f,.65f,1));
   }
   Fade(art,0,78,172,68,false);
   var caption=Rect(card,"Caption",0,24,184,128).gameObject.AddComponent<ExcavationBannerSurface>();
   caption.Caption=true;caption.Selected=active;caption.raycastTarget=false;
   Text(card,"Name",Loc.Get(regalia?"UI_EXCAVATION_REGALIA":"UI_HOME_EXEMPLARS"),16,96,152,26,19,active?UiTheme.Accent:UiTheme.TextHigh);
   Text(card,"Type",regalia?Loc.Get("UI_SHOP_TAB_REGALIA"):BannerTitle(banner),16,124,152,22,14,UiTheme.TextMid,false);
   if(active) {
    var mark=UiTheme.Flat(card,"SelectedMarker",UiTheme.Accent);var mr=mark.rectTransform;
    Place(mr,22,44,7,7);mr.pivot=new Vector2(.5f,.5f);mr.localRotation=Quaternion.Euler(0,0,45);mark.raycastTarget=false;
   }
   var choice=Hit(card,()=>{if(_requestInFlight||_quoteCount>0||!string.IsNullOrEmpty(_pendingKey))return;_selected=banner.id;Rebuild();RevealArt();});
   choice.targetGraphic=frame;
  }
  void BuildBannerTerms(Banner banner) {
   bool progress=!_catalogPreview&&banner.has_pity&&banner.hard>0;
   var card=_termsCard=Owned(_design,"BannerTerms",260,342,376,progress?120:80);
   GameMenuSurface.Add(card);
   Rule(card,22,2,44,1,UiTheme.Accent);
   _termsTitle=Text(card,"NoticeTitle","",22,16,332,28,18,UiTheme.TextHigh,false);
   _terms=Text(card,"Terms","",22,19,289,50,18,UiTheme.TextHigh,false);
   _guaranteeDetail=Rect(card,"GuaranteeInfo",324,8,48,48);
   Insignia(_guaranteeDetail,"Info",13,12,22,24,UiTheme.TextMid);
   Hit(_guaranteeDetail,()=>Probability(banner));
   _pityGroup=null;
   if(progress) {
    _pityGroup=Rect(card,"Pity",0,0,376,120);
    int count=PityCount(banner.kind);
    Rule(_pityGroup,22,98,210,4,UiTheme.SurfaceHover);
    Rule(_pityGroup,22,98,210*Mathf.Clamp01((float)count/banner.hard),4,UiTheme.Accent);
    Text(_pityGroup,"Progress",count+" / "+banner.hard,245,81,109,35,22,UiTheme.TextHigh,false).alignment=TextAlignmentOptions.Right;
   }
  }
  static void Insignia(Transform parent,string kind,float x,float y,float w,float h,Color tint) {
   var r=Rect(parent,kind+"Insignia",x,y,w,h);var icon=r.gameObject.AddComponent<HomeIcon>();icon.Kind=kind;icon.color=tint;icon.raycastTarget=false;
  }
  void InfoAction(string name,float x,string label,Action action) {
   var banner=Array.Find((JsonUtility.FromJson<State>(_stateJson)??new State()).banners??Array.Empty<Banner>(),b=>b.id==_selected);
   bool progress=!_catalogPreview&&banner!=null&&banner.has_pity&&banner.hard>0;
   var r=Owned(_design,name,x,progress?480:440,182,56);
   if(name=="Probability")_probabilityAction=r;else _detailsAction=r;
   var skin=r.gameObject.AddComponent<CodexButtonImage>();skin.color=UiTheme.Surface;skin.Raised=true;
   Insignia(r,name=="Probability"?"Probability":"Story",13,17,22,22,UiTheme.Accent);
   var text=Text(r,"Label",label,42,12,112,32,17,UiTheme.TextHigh,false);
   text.alignment=TextAlignmentOptions.MidlineLeft;text.enableAutoSizing=true;text.fontSizeMin=15;text.fontSizeMax=17;
   Text(r,"OpenIndicator","›",155,6,22,44,26,UiTheme.Accent,false).alignment=TextAlignmentOptions.Center;
   var button=Hit(r,action);button.targetGraphic=skin;
   var colors=button.colors;colors.highlightedColor=Color.white;colors.selectedColor=Color.white;colors.pressedColor=new Color(.64f,.67f,.69f,1);colors.fadeDuration=0;button.colors=colors;
   UiButtonFeedback.Add(button);
  }
  Button PullAction(float x,int count)
  {
   var r=Owned(_design,"Pull"+count,x,638,252,68);
   var skin=r.gameObject.AddComponent<CodexButtonImage>();skin.Primary=count==10;skin.color=UiTheme.Surface;
   var ink=count==10?UiTheme.Accent:UiTheme.TextHigh;
   Text(r,"Label",Loc.Get("UI_EXCAVATION_EXCAVATE"),20,10,212,28,22,ink,false).alignment=TextAlignmentOptions.Center;
   var cost=Text(r,"Cost","×"+count,0,39,50,22,16,UiTheme.TextMid,false);
   cost.alignment=TextAlignmentOptions.MidlineLeft;
   float costWidth=cost.GetPreferredValues(cost.text).x;
   float costLeft=(252-(24+6+costWidth))*.5f;
   Ticket(r,costLeft,41,24,19);
   Place(cost.rectTransform,costLeft+30,39,costWidth+2,22);
   var hint=Text(r,"ActionHint","",0,-29,252,26,13,UiTheme.TextHigh,false);hint.alignment=TextAlignmentOptions.Center;hint.enableAutoSizing=true;hint.fontSizeMin=11;hint.fontSizeMax=13;
   var button=Hit(r,()=>{if(_requestInFlight||_catalogPreview||!string.IsNullOrEmpty(_lastPulls))return;UiConfirm.AskExcavation(count,()=>Roll(count,false,0));});button.targetGraphic=skin;
   var colors=button.colors;colors.disabledColor=new Color(.66f,.69f,.71f,1);colors.fadeDuration=.12f;button.colors=colors;
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

