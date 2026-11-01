using System;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained, screen-space typography and layout for the character management prototype.
    public static class ExemplarUi
    {
        public static readonly Color White = UiTheme.TextHigh;
        public static readonly Color Muted = UiTheme.TextMid;
        public static readonly Color Gold = UiTheme.Accent;
        // Shared medium-value surfaces support differently colored character artwork.
        public static readonly Color Surface = UiTheme.Surface;
        public static readonly Color Deep = UiTheme.Panel;
        public static readonly Color Card = UiTheme.SurfaceHover;
        public static readonly Color Selected = UiTheme.SurfaceActive;
        public static readonly Color Line = UiTheme.LineStrong;
        public static RectTransform Rect(Transform parent, string name, float x, float y, float r, float t)
        {
            var go = new GameObject(name, typeof(RectTransform)); go.transform.SetParent(parent,false);
            var rt = (RectTransform)go.transform; Place(rt,x,y,r,t); return rt;
        }
        public static void Place(RectTransform rt, float x,float y,float r,float t)
        { rt.anchorMin=new Vector2(x,y);rt.anchorMax=new Vector2(r,t);rt.offsetMin=rt.offsetMax=Vector2.zero; }
        public static Image Panel(Transform p,string name,float x,float y,float r,float t,Color color)
        {
            var rt=Rect(p,name,x,y,r,t);var im=rt.gameObject.AddComponent<Image>();
            im.color=color;im.raycastTarget=false;
            if(name is "Shade" or "EquipmentStats" or "GearInspector" or "ResonanceIdentity" or "ResonanceDetails"
                or "RankSurface" or "EffectsSurface" or "EquippedTile")
            { im.color=Color.clear; GameMenuSurface.Add(rt); }
            return im;
        }
        public static TextMeshProUGUI Text(Transform p,string name,string value,float x,float y,float r,float t,float size,Color color,
            TextAlignmentOptions align=TextAlignmentOptions.MidlineLeft, bool numeric=false)
        {
            var label=UiTheme.Label(p,name,value,size,color,align);
            label.characterSpacing=0;label.richText=false;
            if(size>=28 && !numeric)UiTheme.Display(label);
            label.enableAutoSizing=true;label.fontSizeMin=size*.75f;label.fontSizeMax=size;
            Place(label.rectTransform,x,y,r,t);return label;
        }
        public static Button Button(Transform p,string name,string value,float x,float y,float r,float t,Action click,bool bright=false)
        {
            var im=Panel(p,name,x,y,r,t,bright?new Color(.91f,.89f,.83f):Card);
            if(bright)im.gameObject.AddComponent<TacticalPrimary>();
            im.raycastTarget=true;var b=im.gameObject.AddComponent<UiStateButton>();b.targetGraphic=GameMenuSurface.Add(im.transform,bright);im.color=Color.clear;
            b.navigation=new Navigation{mode=Navigation.Mode.None};b.colors=UiTheme.ButtonColors;
            b.onClick.AddListener(()=>click?.Invoke());
            UiButtonFeedback.Add(b);


            var l=Text(im.transform,"Label",value,.075f,.08f,.925f,.92f,20,bright?Gold:White,TextAlignmentOptions.Center);
            return b;
        }
        public static void Icon(Transform p,string id,float x,float y,float r,float t,Color color)
        {
            var rt=Rect(p,id+"Icon",x,y,r,t);
            var icon=rt.gameObject.AddComponent<HomeIcon>();
            icon.Kind=id;icon.color=color;icon.raycastTarget=false;
        }
    }

    public sealed class ExemplarOverview : MonoBehaviour
    {
        TextMeshProUGUI _name,_element,_level,_xp,_status;
        readonly TextMeshProUGUI[] _values = new TextMeshProUGUI[8];
        Button _potential,_equipment;
        public static ExemplarOverview Create(Transform parent,Action<string> navigate)
        {
            var rt=ExemplarUi.Rect(parent,"Attributes",.115f,.14f,.385f,.87f);
            var v=rt.gameObject.AddComponent<ExemplarOverview>();v.Build(navigate);return v;
        }
        void Build(Action<string> navigate)
        {
            var p=transform;
            ExemplarUi.Panel(p,"IdentityRule",0,.80f,.005f,1,ExemplarUi.Gold);
            _name=ExemplarUi.Text(p,"Name","",.045f,.909f,1,1,40,ExemplarUi.White);
            _element=ExemplarUi.Text(p,"Element","",.048f,.848f,1,.906f,22,ExemplarUi.Gold);
            _level=ExemplarUi.Text(p,"Level","",.045f,.777f,1,.842f,29,ExemplarUi.White,numeric:true);
            ExemplarUi.Panel(p,"Divider",.045f,.769f,.97f,.772f,new Color(.84f,.79f,.54f,.70f));
            _xp=ExemplarUi.Text(p,"Experience","",.045f,.724f,.97f,.764f,17,ExemplarUi.Muted);
            string[] keys={"UI_EXEMPLAR_HP","UI_EXEMPLAR_ATK","UI_EXEMPLAR_DEF","UI_EXEMPLAR_CRIT","UI_EXEMPLAR_CRIT_DMG","UI_EXEMPLAR_DODGE","UI_EXEMPLAR_ASPD","UI_EXEMPLAR_MITIGATION"};
            for(int i=0;i<keys.Length;i++)
            {
                float top=.695f-i*.052f,bottom=top-.047f;
                ExemplarUi.Panel(p,"StatBacking"+i,.025f,bottom,.985f,top,new Color(.10f,.18f,.21f,.55f));
                ExemplarUi.Text(p,"StatName"+i,Loc.Get(keys[i]),.045f,bottom,.70f,top,17,ExemplarUi.Muted);
                _values[i]=ExemplarUi.Text(p,"StatValue"+i,"—",.70f,bottom,.965f,top,19,ExemplarUi.White,TextAlignmentOptions.MidlineRight);
            }

            _status=ExemplarUi.Text(p,"Status","",.045f,.183f,.98f,.239f,17,ExemplarUi.Muted);
            _status.textWrappingMode=TextWrappingModes.Normal;
            _potential=ExemplarUi.Button(p,"OpenPotential",GameText.ScreenName("Skills"),.045f,0,.98f,.065f,()=>navigate("Skills"));
            _equipment=ExemplarUi.Button(p,"OpenEquipment",Loc.Get("UI_EXEMPLAR_EQUIPMENT"),.045f,.080f,.98f,.16f,()=>navigate("Equipment"),true);
        }
        public void Show(ExemplarReadModel.Member member)
        {
            if(member==null)
            {
                _name.text=Loc.Get("UI_EXEMPLAR_NO_CHARACTER");_element.text=_level.text=_xp.text="";
                _status.text="";foreach(var v in _values)v.text="—";_potential.interactable=_equipment.interactable=false;return;
            }
            var s=member.Stats;
            _name.text=GameText.UnitName(member.UnitDef);
            _element.text=string.IsNullOrEmpty(member.Element)?Loc.Get("UI_EXEMPLAR_RESERVE"):GameText.ElementName(member.Element);
            _level.text=Loc.Get("UI_EXEMPLAR_LEVEL")+" "+member.Level+(s!=null&&s.LevelCap>0?" / "+s.LevelCap:"");
            _xp.text=Loc.Get("UI_EXEMPLAR_JOB_LEVEL")+" "+member.JobLevel+(s!=null?"   ·   "+Loc.Get("UI_EXEMPLAR_XP")+" "+s.Experience.ToString("N0"):"");
            _status.text=member.Benched?Loc.Get("UI_EXEMPLAR_BENCH_STATS"):Loc.Get("UI_EXEMPLAR_LIVE_STATS");
            _potential.interactable=_equipment.interactable=s!=null&&!member.Benched;
            if(s==null){foreach(var v in _values)v.text="—";return;}
            string[] values={s.MaxHp.ToString("N0"),s.Attack.ToString("N0"),s.Defense.ToString("N0"),
                Percent(s.CritBp),Percent(s.CritDamageBp),Percent(s.DodgeBp),(s.AttackSpeedX100/100m).ToString("0.00")+"/s",Percent(s.MitigationBp)};
            for(int i=0;i<values.Length;i++)_values[i].text=values[i];
        }
        static string Percent(int bp)=>(bp/100f).ToString("0.#")+"%";
    }
}

