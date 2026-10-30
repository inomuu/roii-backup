using TMPro;
using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI
{
    public sealed partial class HomeMenuLayout
    {
        TextMeshProUGUI _level;
        void ComposeTactical(RectTransform rt)
        {
            _font=Loc.Language=="th"?UiTheme.Font:Resources.Load<TMP_FontAsset>("HomeUI/HomeHeading")??UiTheme.Font;
            Picture(rt,"Landscape","HomeUI/TacticalLobby114",0,0,1,1,Color.white);
            _safe=Panel(rt,"SafeContent",0,0,1,1,Color.clear).rectTransform;
            var profile=Panel(_safe,"ProfilePlate",.009f,.828f,.241f,.952f,new Color(.01f,.02f,.03f,.65f));
            var ring=new GameObject("LevelRing",typeof(RectTransform),typeof(TacticalHomeMark));ring.transform.SetParent(profile.transform,false);
            Place((RectTransform)ring.transform,0,-.02f,.40f,1.02f);ring.GetComponent<TacticalHomeMark>().Circle=true;ring.GetComponent<TacticalHomeMark>().color=new Color(.8f,.83f,.65f,.85f);
            _level=Text(profile.transform,"Level","—",.05f,.28f,.34f,.88f,74,Color.white);_level.alignment=TextAlignmentOptions.Center;
            var levelCaption=Text(profile.transform,"LevelLabel",Loc.Get("UI_TACTICAL_LEVEL"),.06f,.07f,.34f,.29f,17,Color.white);levelCaption.alignment=TextAlignmentOptions.Center;
            _profile=Text(profile.transform,"Profile","",.48f,.43f,1,.87f,36,Color.white);_profile.fontStyle=FontStyles.Bold;
            Text(profile.transform,"SessionLabel",Loc.Get("UI_TACTICAL_COMMAND"),.48f,.19f,1,.40f,15,new Color(.65f,.7f,.74f));
            var wallet=HomeWalletBar.Create(_safe,Navigate,true);Place((RectTransform)wallet.transform,.596f,.787f,.973f,.894f);
            string[] utilities={"Options","More","Mail","Mission"};
            for(int i=0;i<utilities.Length;i++){
                var button=TacticalCard(_safe,utilities[i],"","",.021f,.695f-i*.086f,.065f,.77f-i*.086f,false,true);
                Icon(button.transform,utilities[i]=="More"?"Events":utilities[i],.10f,.08f,.90f,.92f,Color.white);
            }
            _ledger=Panel(_safe,"Expedition",.558f,.110f,.978f,.766f,Color.clear).rectTransform;
            var status=TacticalCard(_ledger,"Map","","",0,.813f,1,1,true);
            Icon(status.transform,"Map",.018f,.17f,.17f,.89f,Color.white);
            _expedition=Text(status.transform,"Stage",Loc.Get("UI_HUB_EXPEDITION"),.19f,.20f,.54f,.87f,48,Color.white);_expedition.fontStyle=FontStyles.Bold;
            _battleState=Text(status.transform,"State","",.55f,.24f,.89f,.78f,21,new Color(.8f,.84f,.87f));
            _battleState.textWrappingMode=TextWrappingModes.Normal;_battleState.fontSizeMin=13;
            Text(status.transform,"Plus","+",.91f,.22f,.98f,.90f,44,Color.white);
            _battle=TacticalCard(_ledger,"Combat",Loc.Get("UI_TACTICAL_CAMPAIGN"),Loc.Get("UI_HUB_RETURN_BATTLE"),0,.584f,1,.801f);
            Picture(_battle.transform,"CampaignArt","HomeUI/TacticalLobby114",.59f,.06f,.985f,.94f,new Color(.7f,.8f,.88f,.7f));
            Panel(_battle.transform,"CampaignArtShade",.59f,.06f,.985f,.94f,new Color(.05f,.09f,.15f,.27f));
            Icon(_battle.transform,"Combat",.79f,.11f,.94f,.73f,Color.white);
            BuildHomeMenu(_ledger);
            var banner=TacticalCard(_safe,"News","","",.023f,.165f,.252f,.365f);
            Picture(banner.transform,"NewsArt","HomeUI/TacticalLobby114",0,0,1,1,Color.white);
            Panel(banner.transform,"BannerStripe",0,.03f,1,.33f,new Color(.025f,.17f,.62f,.92f));
            var news=Text(banner.transform,"NewsLabel",Loc.Get("UI_TACTICAL_NEWS"),.035f,.035f,.97f,.32f,43,Color.white);news.fontStyle=FontStyles.Bold;
            TacticalCard(_safe,"Party",Loc.Get("UI_TACTICAL_FRIENDS"),"",.023f,.095f,.137f,.159f);
            TacticalCard(_safe,"News",Loc.Get("UI_TACTICAL_INTEL"),"",.141f,.095f,.252f,.159f);
            _driver=Object.FindFirstObjectByType<Roi.Sim.SimDriver>();LayoutForScreen();
        }
        ScrollRect _homeMenu;
        const float HomeMenuRow=76f;
        void BuildHomeMenu(RectTransform parent)
        {
            var area=Panel(parent,"HomeMenu",0,0,1,.575f,new Color(.015f,.025f,.04f,.72f)).rectTransform;
            var content=UiScroll.Create(area,out _homeMenu);
            var entries=new System.Collections.Generic.List<string>{"Party","Exemplars","Store","Excavation","Album","Inventory"};
            foreach(var entry in MoreEntries)if(!entries.Contains(entry))entries.Add(entry);
            const int columns=3;
            UiScroll.Fit(content,Mathf.CeilToInt(entries.Count/(float)columns)*HomeMenuRow);
            for(int i=0;i<entries.Count;i++)
            {
                string entry=entries[i];
                string label=entry switch {
                    "Party"=>Loc.Get("UI_TACTICAL_TEAM"),"Exemplars"=>Loc.Get("UI_TACTICAL_MEMBER"),
                    "Store"=>Loc.Get("UI_TACTICAL_STORE"),"Excavation"=>Loc.Get("UI_TACTICAL_RECRUIT"),
                    "Album"=>Loc.Get("UI_TACTICAL_ARCHIVE"),"Inventory"=>Loc.Get("UI_TACTICAL_INVENTORY"),
                    _=>GameText.ScreenName(entry)
                };
                var tile=TacticalCard(content,entry,"","",0,0,1,1,i<6);
                var rect=(RectTransform)tile.transform;
                rect.anchorMin=new Vector2((i%columns)/(float)columns,1);
                rect.anchorMax=new Vector2((i%columns+1)/(float)columns,1);
                rect.pivot=new Vector2(.5f,1);
                rect.offsetMin=new Vector2(3,-(i/columns+1)*HomeMenuRow+5);
                rect.offsetMax=new Vector2(-3,-(i/columns)*HomeMenuRow);
                tile.transform.Find("Watermark")?.gameObject.SetActive(false);
                Icon(tile.transform,entry,.045f,.23f,.24f,.77f,Gold);
                var caption=Text(tile.transform,"Label",label,.28f,.12f,.97f,.88f,23,Color.white);
                caption.font=UiTheme.Font;caption.fontSharedMaterial=UiTheme.Material;
                caption.fontStyle=FontStyles.Normal;caption.fontSizeMin=15;caption.fontSizeMax=23;
                caption.textWrappingMode=TextWrappingModes.NoWrap;
            }
        }
        void RevealHomeMenu()
        {
            if(_homeMenu==null)return;
            Canvas.ForceUpdateCanvases();
            float range=Mathf.Max(1,_homeMenu.content.rect.height-_homeMenu.viewport.rect.height);
            _homeMenu.verticalNormalizedPosition=1-Mathf.Clamp01(2*HomeMenuRow/range);
        }

        Button TacticalCard(Transform parent,string id,string label,string sub,float x,float y,float r,float t,bool blue=false,bool transparent=false)
        {
            var image=Panel(parent,id,x,y,r,t,transparent?Color.clear:blue?new Color(.025f,.11f,.22f,1f):new Color(.018f,.025f,.035f,1f));
            image.raycastTarget=true;image.material=HomeIconSet.Material;
            var button=image.gameObject.AddComponent<Button>();button.targetGraphic=image;
            button.onClick.AddListener(()=>Navigate(id));
            var colors=button.colors;colors.highlightedColor=new Color(1.5f,1.6f,1.75f);colors.pressedColor=new Color(.6f,.7f,.8f);button.colors=colors;
            if(!transparent){
                Panel(image.transform,"TopRule",0,.988f,1,1,new Color(.62f,.71f,.81f,.5f));
                Panel(image.transform,"Edge",.997f,0,1,1,new Color(.6f,.7f,.8f,.32f));
                var mark=new GameObject("Watermark",typeof(RectTransform),typeof(TacticalHomeMark));mark.transform.SetParent(image.transform,false);
                Place((RectTransform)mark.transform,.52f,.02f,.97f,.95f);mark.GetComponent<TacticalHomeMark>().color=new Color(.67f,.72f,.79f,.075f);
            }
            if(label.Length>0){
                bool large=t-y>.19f;float font=large?55:32;
                var title=Text(image.transform,"Label",label,.06f,sub.Length>0?.32f:.10f,.96f,.94f,font,Color.white);title.fontStyle=FontStyles.Bold;title.textWrappingMode=TextWrappingModes.NoWrap;
                if(sub.Length>0){var caption=Text(image.transform,"Subtitle",sub.ToUpperInvariant(),.065f,.11f,.96f,.35f,large?19:14,new Color(.73f,.77f,.80f));caption.textWrappingMode=TextWrappingModes.NoWrap;}
            }
            return button;
        }
    }
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class TacticalHomeMark : MaskableGraphic
    {
        public bool Circle;
        public override Material defaultMaterial => HomeIconSet.Material ?? base.defaultMaterial;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;var center=r.center;float radius=Mathf.Min(r.width,r.height)*.46f;
            int count=Circle?72:3;
            for(int layer=0;layer<(Circle?2:3);layer++){
                float size=radius-layer*(Circle?radius*.13f:radius*.20f);
                for(int i=0;i<count;i++){
                    float a=(90+i*360f/count)*Mathf.Deg2Rad,b=(90+(i+1)*360f/count)*Mathf.Deg2Rad;
                    var p=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*size;var q=center+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*size;
                    var n=new Vector2(-(q-p).y,(q-p).x).normalized*(Circle?1:3);int j=vh.currentVertCount;
                    vh.AddVert(p-n,color,Vector2.zero);vh.AddVert(p+n,color,Vector2.zero);vh.AddVert(q+n,color,Vector2.zero);vh.AddVert(q-n,color,Vector2.zero);vh.AddTriangle(j,j+1,j+2);vh.AddTriangle(j,j+2,j+3);
                }
            }
        }
        protected override void Awake(){base.Awake();raycastTarget=false;}
    }
}
