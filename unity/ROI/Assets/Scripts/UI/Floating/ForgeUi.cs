using System;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    public static class ForgeUi
    {
        static TMP_FontAsset _serif,_condensed;
        public static void Heading(TMP_Text text,float size)
        {
            _serif??=UiTheme.DisplayFont;
            SetFont(text,_serif);text.fontSize=size;text.characterSpacing=.7f;
        }
        public static void Display(TMP_Text text,float size)
        {
            _condensed??=UiTheme.Font;
            SetFont(text,_condensed);text.fontSize=size;text.characterSpacing=.5f;
        }
        static void SetFont(TMP_Text text,TMP_FontAsset font)
        {
            if(font==null)return;
            text.font=font;text.fontSharedMaterial=font.material;
        }
        public static readonly Color Ground = UiTheme.Panel, Raised = UiTheme.SurfaceHover, Gold = UiTheme.Accent, Ink = UiTheme.TextHigh;
        public static RectTransform Rect(Transform parent, string name, float x, float y, float w, float h)
        { var go = new GameObject(name,typeof(RectTransform)); go.transform.SetParent(parent,false); var rt=(RectTransform)go.transform; Place(rt,x,y,w,h); return rt; }
        public static void Place(RectTransform rt, float x,float y,float w,float h)
        { rt.anchorMin=rt.anchorMax=rt.pivot=new Vector2(0,1); rt.anchoredPosition=new Vector2(x,-y); rt.sizeDelta=new Vector2(w,h); }
        public static Image Panel(Transform p,string name,float x,float y,float w,float h,Color color)
        { var im=UiTheme.Flat(p,name,color); Place(im.rectTransform,x,y,w,h); return im; }
        public static TextMeshProUGUI Text(Transform p,string name,string text,float x,float y,float w,float h,float size=13,Color? color=null)
        { var t=UiTheme.Label(p,name,text,size,color??Ink,TextAlignmentOptions.MidlineLeft); Place(t.rectTransform,x,y,w,h); t.characterSpacing=0; return t; }
        public static TextMeshProUGUI Copy(Transform p,string key,float x,float y,float w,float h,float size=13,Color? color=null)
            =>Text(p,key,Loc.Get(key),x,y,w,h,size,color);
        public static Button Button(Transform p,string key,float x,float y,float w,float h,Action onClick, bool accent=false)
        {
          if(key=="UI_FLOAT_CLOSE") {
            var close=DialogSurface.CloseIcon(p,onClick,Ink);
            Place((RectTransform)close.transform,x,y,w,h);
            close.GetComponentInChildren<TitleGlyph>().rectTransform.sizeDelta=new Vector2(Mathf.Min(28,w),Mathf.Min(28,h));
            return close;
          }
          var im=CodexButtonImage.Create(p,key,accent); Place(im.rectTransform,x,y,w,h);
          var b=im.gameObject.AddComponent<UiStateButton>();b.targetGraphic=im;b.onClick.AddListener(()=>onClick?.Invoke());
          b.navigation=new Navigation{mode=Navigation.Mode.None};
          b.colors=UiTheme.ButtonColors;
          UiButtonFeedback.Add(b);
          float inset=w<50?3:10;
          var text=Copy(im.transform,key,inset,2,w-inset*2,h-4,13);Display(text,13);
          if(accent){Heading(text,16);text.color=UiTheme.Accent;}
          text.enableAutoSizing=true;text.fontSizeMin=12;text.fontSizeMax=accent?16:14;
          text.alignment=TextAlignmentOptions.Center;return b;
        }
        public static void Wrap(TMP_Text text) { text.textWrappingMode=TextWrappingModes.Normal; text.overflowMode=TextOverflowModes.Truncate; text.alignment=TextAlignmentOptions.TopLeft; }
        public static string Name(ItemView item) => item==null?"":GameText.ItemName(item);
        public static string Description(ItemView item)
        {
            if(item==null)return Loc.Get("UI_FLOAT_SELECT");
            var lines=ForgeUi.Name(item)+"\n"+ItemSlot.StatLine(item);
            foreach(var line in item.authored_lines??Array.Empty<string>())lines+="\n"+line;
            if(item.kind=="Weapon"||item.kind=="Armor")
            { lines+="\n"+Loc.Fmt("UI_FLOAT_SOCKETS",item.socket_count,item.gem_slots);
              foreach(var group in item.gem_line_groups??Array.Empty<GemLineGroup>())foreach(var line in group.lines??Array.Empty<string>())lines+="\n"+Loc.Get("UI_FORGE_GEMS")+": "+line;
              if(item.gems?.Length>0)lines+="\n"+Loc.Get("UI_FLOAT_MARKET_GEM"); }
            return lines;
        }
    }

    public sealed class ForgeCell
    {
        public readonly RectTransform Root;
        readonly Image _background,_border,_selectionDot;
        readonly TextMeshProUGUI _newDot;
        readonly ItemGradeSurface _surface;
        readonly EternalSparkles _sparkles;
        readonly ForgeSigil _gemBadge;
        readonly Button _button;
        readonly ExemplarEquipmentIcon _icon;
        readonly ForgeSigil _gem;
        readonly TextMeshProUGUI _label,_badge,_mark;
        Action _click;
        public ForgeCell(Transform parent,float size=54)
        {
            _background=ForgeUi.Panel(parent,"Item",0,0,size,size,Color.white); Root=_background.rectTransform;
            _background.sprite=UiTheme.CutFill(4);_background.type=Image.Type.Sliced;
            _surface=_background.gameObject.AddComponent<ItemGradeSurface>();
            _sparkles=ForgeUi.Rect(Root,"EternalSparkles",0,0,size,size).gameObject.AddComponent<EternalSparkles>(); _sparkles.raycastTarget=false;
            _border=UiTheme.Border(Root,new Color(.36f,.39f,.42f),4);
            _button=Root.gameObject.AddComponent<Button>(); _button.targetGraphic=_background; _button.onClick.AddListener(()=>_click?.Invoke());
            var colors=_button.colors; colors.disabledColor=Color.white; _button.colors=colors;
            var iconRect=ForgeUi.Rect(Root,"Symbol",9,5,size-18,size-18); _icon=ExemplarEquipmentIcon.Create(iconRect,"Weapon");
            _gem=iconRect.gameObject.AddComponent<ForgeSigil>(); _gem.raycastTarget=false; _gem.color=new Color(.53f,.8f,.91f);
            _label=ForgeUi.Text(Root,"Name","",3,0,size-6,size,15); _label.alignment=TextAlignmentOptions.Center; _label.raycastTarget=false;
            var badgeBg=ForgeUi.Panel(Root,"CaptionShade",4,size-17,size-8,13,new Color(.025f,.032f,.038f,.82f));badgeBg.raycastTarget=false;
            _badge=ForgeUi.Text(Root,"LevelOrQuantity","",6,size-16,size-12,12,10,UiTheme.TextMid); _badge.alignment=TextAlignmentOptions.BottomRight;
            _badge.enableAutoSizing=true; _badge.fontSizeMin=7; _badge.fontSizeMax=10;
            _badge.textWrappingMode=TextWrappingModes.NoWrap; _badge.raycastTarget=false;
            _selectionDot=ForgeUi.Panel(Root,"SelectedMark",4,4,5,5,ForgeUi.Gold); _selectionDot.raycastTarget=false;
            _gemBadge=ForgeUi.Rect(Root,"GemBadge",3,3,10,10).gameObject.AddComponent<ForgeSigil>(); _gemBadge.color=ForgeUi.Gold;_gemBadge.raycastTarget=false;
            _mark=ForgeUi.Text(Root,"Selected","",2,1,24,15,11,ForgeUi.Gold);
            _newDot=ForgeUi.Text(Root,"NewItemDot","●",size-13,0,12,13,11,new Color(1,.18f,.22f));
            _newDot.raycastTarget=false; _newDot.gameObject.SetActive(false);
        }
        public void SetNew(bool value)=>_newDot.gameObject.SetActive(value);
        public void Bind(ItemView item,Action click,bool selected=false,bool gem=false,int count=0,string empty="+",string slot=null)
        {
            _click=click; SetNew(false); _background.color=Color.white; _surface.Bind(item,gem,selected);
            _sparkles.Bind(item!=null&&item.grade_step>=8,item!=null&&item.grade_step==8);
            _border.color=selected?UiTheme.Accent:item==null?new Color(.31f,.38f,.40f,.42f):Color.Lerp(ItemGradeSurface.Accent(item,gem),UiTheme.LineStrong,.35f);
            Root.Find("CaptionShade").gameObject.SetActive(item!=null);
            _icon.gameObject.SetActive(true); _icon.enabled=(item!=null||slot!=null)&&!gem; _gem.enabled=item!=null&&gem;
            _icon.SetItem(item,slot??item?.slot??"Weapon"); _icon.color=new Color(1,1,1,item==null?.35f:1);
            // The corner shows the sim-authored equip requirement, not the artwork tier/item level.
            _label.text=item==null&&slot==null?empty:"";
            _label.color=ForgeUi.Ink;
            _badge.text=item==null?"":gem?"×"+item.quantity:(item.kind=="Weapon"||item.kind=="Armor")&&item.level_req>0?Loc.Fmt("UI_COMMON_LEVEL_SHORT",item.level_req):"";
            _mark.text=selected&&count>0?count.ToString():"";
            _selectionDot.enabled=selected&&count==0; _gemBadge.enabled=!selected&&item?.gems?.Length>0;
            _button.interactable=click!=null;
        }
        public void SetCompatibility(bool filtered,bool compatible)
        {
            if(!filtered){_background.color=Color.white;_button.interactable=_click!=null;return;}
            _background.color=compatible?new Color(.24f,.62f,.37f,.88f):new Color(.16f,.18f,.22f,.48f);
            _button.interactable=compatible&&_click!=null;
        }
    }

    // Crisp UI geometry for empty-result preview and gem symbols; no raster resizing artifacts.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ForgeSigil : MaskableGraphic
    {
        public bool Rays;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear(); var r=rectTransform.rect; var c=r.center; float rad=Mathf.Min(r.width,r.height)*.48f;
            if(Rays)
            {
                // Feather every ray to zero alpha; outlined diamonds keep the reward readable.
                for(int i=0;i<16;i++)
                {
                    float a=i*Mathf.PI/8;
                    Gradient(vh,c,c+new Vector2(Mathf.Cos(a-.018f),Mathf.Sin(a-.018f))*rad,c+new Vector2(Mathf.Cos(a+.018f),Mathf.Sin(a+.018f))*rad,.4f);
                }
                for(int i=0;i<64;i++)
                {
                    float a=i*Mathf.PI/32,b=(i+1)*Mathf.PI/32;
                    Gradient(vh,c,c+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*rad*.7f,c+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*rad*.7f,.32f);
                }
                Ring(vh,c,rad*.51f,1.2f); Ring(vh,c,rad*.57f,.7f);
            }
            else { Diamond(vh,c,rad,color); AddTriangle(vh,c+Vector2.up*rad,c+Vector2.right*rad,c,new Color(1,1,1,.48f)); }
        }
        void Gradient(VertexHelper vh,Vector2 c,Vector2 a,Vector2 b,float alpha)
        { int n=vh.currentVertCount; vh.AddVert(c,new Color(color.r,color.g,color.b,alpha),Vector2.zero); vh.AddVert(a,new Color(color.r,color.g,color.b,0),Vector2.zero); vh.AddVert(b,new Color(color.r,color.g,color.b,0),Vector2.zero);vh.AddTriangle(n,n+1,n+2); }
        void Ring(VertexHelper vh,Vector2 c,float r,float thickness)
        {
            var corners=new[]{Vector2.up,Vector2.right,Vector2.down,Vector2.left};
            for(int i=0;i<4;i++)
            {var a=corners[i];var b=corners[(i+1)%4];var col=new Color(color.r,color.g,color.b,.75f);AddTriangle(vh,c+a*r,c+b*r,c+b*(r-thickness),col);AddTriangle(vh,c+a*r,c+b*(r-thickness),c+a*(r-thickness),col);}
        }
        static void Diamond(VertexHelper vh,Vector2 c,float r,Color col)
        { AddTriangle(vh,c+Vector2.up*r,c+Vector2.right*r,c+Vector2.down*r,col); AddTriangle(vh,c+Vector2.up*r,c+Vector2.down*r,c+Vector2.left*r,col); }
        static void AddTriangle(VertexHelper vh,Vector2 a,Vector2 b,Vector2 c,Color col)
        { int n=vh.currentVertCount; vh.AddVert(a,col,Vector2.zero); vh.AddVert(b,col,Vector2.zero); vh.AddVert(c,col,Vector2.zero); vh.AddTriangle(n,n+1,n+2); }
    }

    public sealed class ForgeDrag : MonoBehaviour, IBeginDragHandler, IDragHandler
    {
        public RectTransform Target; public Action Moved;
        Vector2 _start,_pointer;
        public void OnBeginDrag(PointerEventData e) { _start=Target.anchoredPosition; _pointer=e.position; }
        public void OnDrag(PointerEventData e)
        { var canvas=Target.GetComponentInParent<Canvas>(); Target.anchoredPosition=_start+(e.position-_pointer)/canvas.scaleFactor; Moved?.Invoke(); }
    }
}
