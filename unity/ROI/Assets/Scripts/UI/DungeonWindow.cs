// DUNGEONS — the instanced runs you can enter. Ported from godot/views/dungeon_menu_view.gd.
//
// The whole screen is one const table and one input event. The dungeon list is NOT sim data there
// either: it is a literal in the view file, because a dungeon is a start zone plus three strings
// and the sim only needs the zone id. That is copied rather than "improved" into a query — the sim
// has no dungeon catalog to ask.
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class DungeonWindow : MonoBehaviour
    {
        const float CardH = 220f, Gap = UiTheme.PadSmall;

        // dungeon_menu_view.gd's DUNGEONS, verbatim — one entry today, and the shape is what
        // matters: adding the second is a row here.
        static readonly (string Zone, string Name, string Levels, string Desc, Color Tint)[] Dungeons =
        {
            ("zone_b10_1", "Duskmourn Manor", "Lv 46 – 50",
             "A gothic manor at the end of the Duskmourn arc. Its halls do not let go easily.",
             new Color(0.30f, 0.20f, 0.34f)),
        };

        Roi.Sim.SimDriver _driver;

        public static DungeonWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<DungeonWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            // Master/detail layout: independently scrollable destinations and information,
            // with the entry action pinned outside both scroll areas.
            var listHost=ExemplarUi.Rect(body,"Destinations",.025f,.04f,.445f,.96f);
            var list=UiScroll.Create(listHost,out _);
            var detail=ExemplarUi.Rect(body,"DungeonDetails",.49f,.22f,.975f,.96f);
            var picture=ExemplarUi.Rect(detail,"Destination",0,.59f,.38f,1);
            var image=picture.gameObject.AddComponent<UnityEngine.UI.RawImage>();
            image.texture=Resources.Load<Texture2D>("HomeUI/CastleBanner");
            image.color=Color.white;image.raycastTarget=false;picture.gameObject.AddComponent<HomeImageCrop>();
            var name=ExemplarUi.Text(detail,"Name","",.42f,.79f,1,1,UiTheme.TitleSize,UiTheme.Accent);
            var level=ExemplarUi.Text(detail,"Level","",.42f,.59f,1,.77f,UiTheme.BodySize,UiTheme.TextHigh);
            ExemplarUi.Panel(detail,"Divider",0,.53f,1,.532f,UiTheme.LineStrong);
            var infoHost=ExemplarUi.Rect(detail,"Information",0,0,1,.48f);
            var info=UiScroll.Create(infoHost,out _);
            var description=UiTheme.Label(info,"Description","",UiTheme.BodySize,UiTheme.TextMid,TextAlignmentOptions.TopLeft);
            description.textWrappingMode=TextWrappingModes.Normal;
            var dr=description.rectTransform;dr.anchorMin=new Vector2(0,1);dr.anchorMax=Vector2.one;
            dr.pivot=new Vector2(.5f,1);dr.anchoredPosition=Vector2.zero;dr.sizeDelta=new Vector2(0,160);
            UiScroll.Fit(info,160);
            var hint=ExemplarUi.Text(body,"Hint",Loc.Get("UI_DUNGEON_ENTERING_A_DUNGEON_TAKES_THE"),
                .49f,.115f,.975f,.205f,UiTheme.CaptionSize,UiTheme.TextMid);
            hint.textWrappingMode=TextWrappingModes.Normal;
            int selected=0;
            var buttons=new System.Collections.Generic.List<MenuNavigationButton>();
            System.Action<int> select=index=>
            {
                selected=index;
                name.text=Loc.Get("UI_DUNGEON_DUSKMOURN_NAME");
                level.text=Loc.Get("UI_DUNGEON_DUSKMOURN_LV");
                description.text=Loc.Get("UI_DUNGEON_DUSKMOURN_DESC");
                foreach(var button in buttons)button.SetActiveState(buttons.IndexOf(button)==index);
                Canvas.ForceUpdateCanvases();
                float height=Mathf.Max(120,description.GetPreferredValues(description.text,Mathf.Max(1,infoHost.rect.width),10000).y+16);
                dr.sizeDelta=new Vector2(0,height);UiScroll.Fit(info,height);
            };
            for(int i=0;i<Dungeons.Length;i++)
            {
                int index=i;
                var row=CollectionBrowser.Quiet(list,"D_"+Dungeons[i].Zone,Loc.Get("UI_DUNGEON_DUSKMOURN_NAME"),0,1,1,1,()=>select(index));
                var rt=(RectTransform)row.transform;rt.pivot=new Vector2(.5f,1);
                rt.offsetMin=new Vector2(0,-i*100-88);rt.offsetMax=new Vector2(0,-i*100);
                row.Caption.fontSize=UiTheme.HeadSize;row.Caption.enableAutoSizing=true;
                row.Caption.fontSizeMin=13;row.Caption.fontSizeMax=UiTheme.HeadSize;
                row.Caption.alignment=TextAlignmentOptions.Left;
                ExemplarUi.Place(row.Caption.rectTransform,.045f,.40f,.96f,.92f);
                ExemplarUi.Text(rt,"Level",Loc.Get("UI_DUNGEON_DUSKMOURN_LV"),.045f,.08f,.96f,.4f,UiTheme.CaptionSize,UiTheme.TextMid);
                row.AccentMark=ExemplarUi.Panel(rt,"Selected",0,.08f,.006f,.92f,UiTheme.Accent);
                UiTheme.Border(rt,UiTheme.LineStrong);buttons.Add(row);
            }
            UiScroll.Fit(list,Dungeons.Length*100);
            var enter=HudBar.MakeButton(body,Loc.Get("UI_DUNGEON_ENTER"),"",240,UiLayout.Touch,
                ()=>Send($"{{\"EnterInstance\":{{\"start_zone_id\":\"{Dungeons[selected].Zone}\"}}}}"),true);
            var er=(RectTransform)enter.transform;er.anchorMin=er.anchorMax=new Vector2(.975f,.025f);
            er.pivot=new Vector2(1,0);er.anchoredPosition=Vector2.zero;
            select(0);
        }

        void Send(string eventJson)
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            _driver?.Session?.ApplyInput(eventJson);
        }
    }
}
