using TMPro;
using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
    // Shared modal palette and typography; callers retain content layout and callbacks.
    // WuWa idiom: dark cut-corner glass, a centred title over a ——◇—— divider, the confirm action
    // an ivory plate with a ◯, the cancel a dark plate with a ✕.
    public static class DialogSurface {
        // Each owner supplies its existing cancel/back action; never infer a confirmation.
        public static Button CloseIcon(Transform panel,System.Action dismiss,Color? tint=null) {
            var hit=UiTheme.Flat(panel,"Close",Color.clear);
            var rt=hit.rectTransform;rt.anchorMin=rt.anchorMax=rt.pivot=Vector2.one;
            rt.anchoredPosition=new Vector2(-12,-10);rt.sizeDelta=new Vector2(44,44);
            var button=hit.gameObject.AddComponent<Button>();button.navigation=new Navigation{mode=Navigation.Mode.None};
            button.onClick.AddListener(()=>dismiss?.Invoke());
            var glyph=TitleGlyph.Add(hit.transform,TitleGlyph.Symbol.Close);
            glyph.rectTransform.sizeDelta=new Vector2(24,24);glyph.color=tint??UiTheme.TextHigh;
            button.targetGraphic=glyph;button.colors=UiTheme.ButtonColors;UiButtonFeedback.Add(button);
            return button;
        }
        public static void Apply(Image panel) { Add(panel.rectTransform); }
        public static void Add(RectTransform panel) {
            if(panel.GetComponent<NoticeDialogStyle>()==null)panel.gameObject.AddComponent<NoticeDialogStyle>();
        }
    }
    public sealed class NoticeDialogStyle : MonoBehaviour {
        static readonly Color Glass=new Color(.072f,.075f,.085f,.97f);
        static readonly Color Row=new Color(.115f,.118f,.13f,.9f);
        public bool CompactLogin;
        bool dirty=true;
        void OnEnable(){dirty=true;}
        void OnTransformChildrenChanged(){dirty=true;}
        void LateUpdate(){if(!dirty)return;dirty=false;Refresh();}
        public void MarkDirty(){dirty=true;}

        // A label carrying MEANING in its colour (a price in gold, a warning in red, a rarity)
        // keeps it. Only neutral ink is repainted.
        static bool Semantic(Color c){Color.RGBToHSV(c,out _,out float s,out float v);return s>.3f&&v>.35f;}

        public void Refresh(){
            foreach(var t in GetComponentsInChildren<Transform>(true)) {
                var observer=t.GetComponent<DialogContentObserver>();
                if(observer==null)observer=t.gameObject.AddComponent<DialogContentObserver>();
                observer.Owner=this;
            }
            var panel=GetComponent<Image>();
            if(panel==null)panel=gameObject.AddComponent<Image>();
            panel.sprite=UiTheme.CutFill(8);panel.type=Image.Type.Sliced;panel.color=Glass;
            var frame=transform.Find("WuwaPanel")?.GetComponent<WuwaPanel>();
            if(frame==null){
                frame=WuwaPanel.Add(transform,14,true);
                frame.Top=new Color(1,1,1,.035f);frame.Bottom=new Color(1,1,1,0);
                frame.Edge=UiTheme.WithAlpha(UiTheme.Accent,.32f);frame.TopAccent=true;
            }
            foreach(var art in GetComponentsInChildren<GameMenuSurface>(true))art.enabled=false;
            foreach(var art in GetComponentsInChildren<HomeCelestialArt>(true))art.enabled=false;
            foreach(var img in GetComponentsInChildren<Image>(true)) {
                if(img==panel)continue;
                string n=img.name;
                if(n=="SettingRow" || (CompactLogin && n.Length>1 && n[0]=='M' && char.IsDigit(n[1])))img.color=Row;
                if(n=="FocusRing")img.color=UiTheme.WithAlpha(UiTheme.Accent,.8f);
                if(n=="Border"||n=="HeaderWash"||n=="HeaderRule"||n=="FantasyDivider"||n.Contains("Diamond"))
                    img.color=UiTheme.WithAlpha(UiTheme.TextHigh,.12f);
            }
            foreach(var field in GetComponentsInChildren<TMP_InputField>(true)) {
                if(field.targetGraphic is Image bg){bg.sprite=UiTheme.CutFill(4);bg.type=Image.Type.Sliced;bg.color=Row;}
                field.selectionColor=UiTheme.WithAlpha(UiTheme.Accent,.3f);
            }

            // Which button is the dialog's CONFIRM. Marked ones win; otherwise the one filled
            // button standing beside a ghost (cancel) button is the action being asked for.
            Button primary=null;int filled=0,ghosts=0;Button lastFilled=null;
            foreach(var button in GetComponentsInChildren<Button>(true)) {
                if(button.transform==transform||button.name=="Close"||button.name=="Option")continue;
                if(button.GetComponentInParent<ScrollRect>()!=null)continue;
                if(button.GetComponent<UiPrimaryAction>()!=null){primary=button;break;}
                if(button.name.StartsWith("Ghost_"))ghosts++;
                else if(button.name.StartsWith("Btn_")){filled++;lastFilled=button;}
            }
            if(primary==null&&filled==1&&ghosts>=1)primary=lastFilled;

            foreach(var label in GetComponentsInChildren<TMP_Text>(true)) {
                var button=label.GetComponentInParent<Button>();
                bool inButton=button!=null&&button.transform!=transform&&button.transform.IsChildOf(transform);
                if(inButton) label.color=button==primary?UiTheme.OnAccent:UiTheme.TextHigh;
                else if(!Semantic(label.color)) label.color=label.name=="Body"||label.fontSize<15?UiTheme.TextMid:UiTheme.TextHigh;
                if(!inButton && (label.name=="Body" || (label.fontSize>=18 && label.fontSize<=26))) {
                    label.fontSize=16; label.enableAutoSizing=false;
                }
                if(!inButton&&(label.name=="Title"||label.name=="Head"||label.name=="Heading")) {
                    if(CompactLogin && (label.transform.parent==transform || label.transform.parent.name=="TitleBar")){
                        label.color=UiTheme.TextHigh;label.fontSize=19;label.enableAutoSizing=false;
                        label.alignment=TextAlignmentOptions.MidlineLeft;label.margin=Vector4.zero;
                        continue;
                    }
                    label.fontSize=24;label.enableAutoSizing=false;
                    label.alignment=TextAlignmentOptions.Midline;
                    if(label is TextMeshProUGUI ugui){
                        if(Loc.Language!="th")UiTheme.Display(ugui);
                        else{ugui.font=UiTheme.Font;ugui.fontSharedMaterial=UiTheme.Material;}
                    }
                    label.margin=new Vector4(48,0,48,0);
                    var old=label.transform.Find("NoticeDivider");if(old!=null)old.gameObject.SetActive(false);
                    if(label.transform.Find("WuwaDivider")==null){
                        var line=WuwaDivider.Add(label.transform);
                        var r=line.rectTransform;r.anchorMin=r.anchorMax=new Vector2(.5f,0);r.pivot=new Vector2(.5f,.5f);
                        r.sizeDelta=new Vector2(Mathf.Clamp(label.rectTransform.rect.width*.7f,160,420),12);r.anchoredPosition=new Vector2(0,-2);
                    }
                }
            }
            foreach(var button in GetComponentsInChildren<Button>(true)) {
                if(button.transform==transform)continue;
                var image=button.GetComponent<Image>();
                if(image==null)continue;
                if(button.name=="Close") {
                    image.color=Color.clear;
                    if(button.transform.GetSiblingIndex()!=button.transform.parent.childCount-1)button.transform.SetAsLastSibling();
                    foreach(var glyph in button.GetComponentsInChildren<TitleGlyph>(true))glyph.color=UiTheme.TextHigh;
                    continue;
                }
                if(button.name=="Option") {
                    image.color=Row;
                    var oldArt=button.transform.Find("NoticeButtonArt");if(oldArt!=null)oldArt.gameObject.SetActive(false);
                    foreach(var text in button.GetComponentsInChildren<TMP_Text>(true))if(!Semantic(text.color))text.color=UiTheme.TextHigh;
                    continue;
                }
                // Plain text controls share a 44-unit height. Item cards and scroll
                // entries retain their content-driven dimensions.
                var labels=button.GetComponentsInChildren<TMP_Text>(true);
                var rt=(RectTransform)button.transform;
                bool plain=labels.Length==1 && button.GetComponentInParent<ScrollRect>()==null
                    && rt.anchorMin.y==rt.anchorMax.y && rt.rect.height>=44 && rt.rect.height<=100;
                if(plain) {
                    float oldHeight=rt.rect.height;
                    rt.SetSizeWithCurrentAnchors(RectTransform.Axis.Vertical,44);
                    rt.anchoredPosition+=new Vector2(0,(.5f-rt.pivot.y)*(oldHeight-44));
                    var layout=button.GetComponent<LayoutElement>();
                    if(layout!=null){layout.minHeight=44;layout.preferredHeight=44;}
                    labels[0].font=UiTheme.Font; labels[0].fontSharedMaterial=UiTheme.Material;
                    labels[0].fontSize=17; labels[0].enableAutoSizing=false;
                    labels[0].alignment=TextAlignmentOptions.Midline;
                }
                foreach(var child in button.GetComponentsInChildren<Image>(true))
                    if(child!=image&&(child.name=="Border"||child.name=="Plate"))child.enabled=false;
                if(image is CodexButtonImage codex)codex.NoticeStyle=true;
                image.color=Color.clear;
                var art=button.transform.Find("NoticeButtonArt")?.GetComponent<NoticeButtonGraphic>();
                if(art==null){
                    var go=new GameObject("NoticeButtonArt",typeof(RectTransform));
                    go.transform.SetParent(button.transform,false);go.transform.SetAsFirstSibling();
                    art=go.AddComponent<NoticeButtonGraphic>();art.raycastTarget=false;
                    var ar=art.rectTransform;ar.anchorMin=Vector2.zero;ar.anchorMax=Vector2.one;
                    ar.offsetMin=ar.offsetMax=Vector2.zero;
                }
                bool isPrimary=button==primary;
                if(art.Primary!=isPrimary){art.Primary=isPrimary;art.SetVerticesDirty();}
                // ◯ on the confirm, ✕ on the cancel — the WuWa tell, only on roomy plain buttons.
                bool cancel=button.name.StartsWith("Ghost_")&&primary!=null;
                var glyphT=button.transform.Find("ActionGlyph");
                if(plain&&(isPrimary||cancel)&&rt.rect.width>=130) {
                    var glyph=glyphT!=null?glyphT.GetComponent<WuwaActionGlyph>():null;
                    if(glyph==null){
                        var go=new GameObject("ActionGlyph",typeof(RectTransform));go.transform.SetParent(button.transform,false);
                        glyph=go.AddComponent<WuwaActionGlyph>();glyph.raycastTarget=false;
                        var gr=glyph.rectTransform;gr.anchorMin=gr.anchorMax=new Vector2(0,.5f);gr.sizeDelta=new Vector2(16,16);gr.anchoredPosition=new Vector2(24,0);
                    }
                    glyph.Confirm=isPrimary;glyph.color=isPrimary?UiTheme.OnAccent:UiTheme.TextMid;glyph.SetVerticesDirty();
                } else if(glyphT!=null) glyphT.gameObject.SetActive(false);
                art.color=Color.white;
                button.targetGraphic=art;
                var colors=button.colors;
                colors.normalColor=Color.white;
                colors.highlightedColor=isPrimary?new Color(1.06f,1.04f,1f):new Color(1.45f,1.42f,1.35f);
                colors.selectedColor=colors.highlightedColor;
                colors.pressedColor=new Color(.78f,.78f,.76f);
                colors.disabledColor=new Color(.5f,.5f,.5f,.7f);
                colors.colorMultiplier=1;button.colors=colors;
                art.CrossFadeColor(button.interactable?colors.normalColor:colors.disabledColor,0,true,true);
            }
        }
    }
}

namespace Roi.UI {
    // A WuWa dialog button: a cut-corner plate. Confirm = ivory with an inner rule; anything
    // else = dark glass with a hairline and a one-pixel top highlight.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class NoticeButtonGraphic : MaskableGraphic {
        public bool Primary;
        protected override void OnPopulateMesh(VertexHelper vh) {
            vh.Clear(); var r=GetPixelAdjustedRect();
            r.xMin+=1;r.xMax-=1;r.yMin+=1;r.yMax-=1;
            if(r.width<4||r.height<4)return;
            float cut=Mathf.Clamp(r.height*.22f,4,9);
            var p=WuwaMesh.Cut(r,cut);
            if(Primary){
                WuwaMesh.Fill(vh,p,r,UiTheme.IvoryShade*color,UiTheme.Ivory*color);
                var inner=new Rect(r.xMin+3,r.yMin+3,r.width-6,r.height-6);
                WuwaMesh.Outline(vh,WuwaMesh.Cut(inner,cut*.7f),1f,new Color(.25f,.22f,.16f,.35f)*color);
                return;
            }
            WuwaMesh.Fill(vh,p,r,new Color(.085f,.088f,.098f,.95f)*color,new Color(.15f,.153f,.165f,.95f)*color);
            WuwaMesh.Outline(vh,p,1f,UiTheme.LineStrong*color);
            WuwaMesh.Line(vh,new Vector2(r.xMin+cut+2,r.yMax-1.5f),new Vector2(r.xMax-2,r.yMax-1.5f),1f,new Color(1,1,1,.08f)*color);
        }
    }
}

namespace Roi.UI {
    public sealed class DialogContentObserver : MonoBehaviour {
        public NoticeDialogStyle Owner;
        void OnTransformChildrenChanged(){if(Owner!=null)Owner.MarkDirty();}
        void OnEnable(){if(Owner!=null)Owner.MarkDirty();}
    }
}
