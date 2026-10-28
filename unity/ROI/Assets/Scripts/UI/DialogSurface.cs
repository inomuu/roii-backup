using TMPro;
using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
    // One modal palette; callers retain sizing, input ownership and callbacks.
    public static class DialogSurface {
        public static void Apply(Image panel) { Add(panel.rectTransform); }
        public static void Add(RectTransform panel) {
            if(panel.GetComponent<NoticeDialogStyle>()==null)panel.gameObject.AddComponent<NoticeDialogStyle>();
        }
    }
    public sealed class NoticeDialogStyle : MonoBehaviour {
        static readonly Color Ink=new Color(.075f,.09f,.095f);
        static readonly Color Paper=new Color(.965f,.965f,.95f);
        bool dirty=true;
        void OnEnable(){dirty=true;}
        void OnTransformChildrenChanged(){dirty=true;}
        void LateUpdate(){if(!dirty)return;dirty=false;Refresh();}
        public void Refresh(){
            var panel=GetComponent<Image>();
            if(panel==null)panel=gameObject.AddComponent<Image>();
            panel.sprite=UiTheme.NoticePaper;panel.type=Image.Type.Sliced;panel.color=Paper;
            foreach(var art in GetComponentsInChildren<GameMenuSurface>(true))art.enabled=false;
            foreach(var art in GetComponentsInChildren<HomeCelestialArt>(true))art.enabled=false;
            foreach(var img in GetComponentsInChildren<Image>(true)) {
                if(img==panel)continue;
                string n=img.name;
                if(n=="Border"||n=="HeaderWash"||n=="HeaderRule"||n=="FantasyDivider"||n.Contains("Diamond")) {
                    img.color=new Color(.25f,.29f,.30f,.18f);
                }
            }
            foreach(var field in GetComponentsInChildren<TMP_InputField>(true)) {
                if(field.targetGraphic is Image bg){bg.sprite=UiTheme.Fill(8);bg.type=Image.Type.Sliced;bg.color=new Color(.86f,.88f,.88f);}
                field.selectionColor=new Color(.3f,.45f,.5f,.3f);
            }
            foreach(var label in GetComponentsInChildren<TMP_Text>(true)) {
                var button=label.GetComponentInParent<Button>();
                bool inButton=button!=null&&button.transform!=transform&&button.transform.IsChildOf(transform);
                label.color=inButton?Color.white:Ink;
                if(!inButton&&(label.name=="Title"||label.name=="Head"||label.name=="Heading")) {
                    label.alignment=TextAlignmentOptions.MidlineLeft;
                    label.font=UiTheme.Font;label.fontSharedMaterial=UiTheme.Font.material;
                    label.margin=label.rectTransform.offsetMin.x < 1 ? new Vector4(24,0,24,0) : Vector4.zero;
                    if(label.transform.Find("NoticeDivider")==null){
                        var line=UiTheme.Flat(label.transform,"NoticeDivider",new Color(.25f,.29f,.30f,.22f));
                        line.raycastTarget=false;
                        var r=line.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=new Vector2(1,0);
                        r.offsetMin=new Vector2(0,0);r.offsetMax=new Vector2(0,1);
                    }
                }
            }
            foreach(var button in GetComponentsInChildren<Button>(true)) {
                if(button.transform==transform)continue;
                var image=button.GetComponent<Image>();
                if(image==null)continue;
                if(button.name=="Close") {
                    image.color=Color.clear;
                    foreach(var glyph in button.GetComponentsInChildren<TitleGlyph>(true))glyph.color=Ink;
                    continue;
                }
                foreach(var child in button.GetComponentsInChildren<Image>(true))
                    if(child!=image&&(child.name=="Border"||child.name=="Plate"))child.enabled=false;
                if(image is CodexButtonImage codex)codex.NoticeStyle=true;
                image.sprite=UiTheme.NoticePill;image.type=Image.Type.Sliced;image.color=Color.white;
                button.targetGraphic=image;
                var colors=button.colors;
                colors.normalColor=new Color(.055f,.065f,.068f);
                colors.highlightedColor=new Color(.18f,.23f,.24f);
                colors.selectedColor=colors.highlightedColor;
                colors.pressedColor=new Color(.025f,.035f,.04f);
                colors.disabledColor=new Color(.49f,.52f,.52f);
                colors.colorMultiplier=1;button.colors=colors;
                image.CrossFadeColor(button.interactable?colors.normalColor:colors.disabledColor,0,true,true);
            }
        }
    }
}
