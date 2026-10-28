using System;
using System.Linq;
using System.Collections.Generic;
using System.Text;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // WuWa/Exemplar visual theme; TBH informs information grouping only.
    public sealed class FloatingItemTooltip : MonoBehaviour
    {
        RectTransform _root,_viewport,_content,_head,_headerRule;
        TMP_Text _copy,_hint,_title,_meta;
        Image _border,_iconFrame,_iconBorder,_gradeAccent;
        ItemGradeSurface _iconSurface;
        EternalSparkles _iconSparkles;
        ExemplarEquipmentIcon _icon;
        ForgeSigil _gem;
        readonly List<Image> _rules=new();
        float _contentHeight,_viewHeight,_titleHeight,_metaHeight;
        string _text;
        public bool Visible=>gameObject.activeSelf;
        public bool CanScroll=>_contentHeight>_viewHeight;
        public string Text=>_text;
        public static FloatingItemTooltip Create(Transform parent)
        {
            var root=ForgeUi.Panel(parent,"FloatingItemTooltip",0,0,330,200,new Color(ExemplarUi.Deep.r,ExemplarUi.Deep.g,ExemplarUi.Deep.b,1)).rectTransform;
            var t=root.gameObject.AddComponent<FloatingItemTooltip>();t._root=root;
            t._border=UiTheme.Border(root,ExemplarUi.Line,2);
            t._head=ForgeUi.Rect(root,"Title",12,8,306,32);
            t._gradeAccent=ForgeUi.Panel(root,"GradeAccent",0,0,3,84,ExemplarUi.Muted);
            t._title=ForgeUi.Text(t._head,"Name","",0,0,306,32,17,ExemplarUi.White);t._title.fontStyle=FontStyles.Bold;t._title.richText=false;
            ForgeUi.Wrap(t._title);t._title.overflowMode=TextOverflowModes.Overflow;
            t._iconFrame=ForgeUi.Panel(root,"ItemFrame",12,46,38,38,Color.white);
            t._iconSurface=t._iconFrame.gameObject.AddComponent<ItemGradeSurface>();
            t._iconSparkles=ForgeUi.Rect(t._iconFrame.transform,"EternalSparkles",0,0,38,38).gameObject.AddComponent<EternalSparkles>(); t._iconSparkles.raycastTarget=false;
            t._iconBorder=UiTheme.Border(t._iconFrame.rectTransform,ExemplarUi.Line,2);
            var symbol=ForgeUi.Rect(t._iconFrame.transform,"Symbol",4,4,30,30);
            t._icon=ExemplarEquipmentIcon.Create(symbol,"Weapon");t._gem=symbol.gameObject.AddComponent<ForgeSigil>();t._gem.color=new Color(.5f,.83f,.95f);
            t._meta=ForgeUi.Text(root,"GradeSlot","",60,45,258,42,12);ForgeUi.Wrap(t._meta);t._meta.richText=false;
            t._headerRule=ForgeUi.Panel(root,"HeaderRule",12,92,306,1,ExemplarUi.Line).rectTransform;
            t._viewport=ForgeUi.Rect(root,"Viewport",12,101,306,156);t._viewport.gameObject.AddComponent<RectMask2D>();
            t._content=ForgeUi.Rect(t._viewport,"Content",0,0,306,156);
            t._copy=ForgeUi.Text(t._content,"FullItemDetails","",0,0,306,156,13);ForgeUi.Wrap(t._copy);
            t._copy.overflowMode=TextOverflowModes.Overflow;t._copy.richText=true;
            t._hint=ForgeUi.Copy(root,"UI_FLOAT_HOVER_SCROLL",12,178,306,18,10,ExemplarUi.Muted);
            foreach(var g in root.GetComponentsInChildren<Graphic>(true))g.raycastTarget=false;
            root.gameObject.SetActive(false);return t;
        }
        public void Show(string text,Vector2 point,ItemView item=null,bool gem=false)
        {
            if(string.IsNullOrEmpty(text)){Hide();return;}
            gameObject.SetActive(true);transform.SetAsLastSibling();
            float scale=GetComponentInParent<Canvas>().scaleFactor;
            float width=Mathf.Min(330,Screen.width/scale-16),maxHeight=Screen.height/scale-16,inner=width-24;
            bool changed=_text!=text||Mathf.Abs(_root.rect.width-width)>.1f;
            if(changed)
            {
                _text=text;var lines=text.Split('\n');_title.text=lines[0];_meta.text=lines.Length>1?lines[1]:"";
                _title.color=item==null?ExemplarUi.White:ItemGradeSurface.TextAccent(item,gem);
                _meta.color=_gradeAccent.color=item==null?ExemplarUi.Muted:ItemGradeSurface.Accent(item,gem);
                _iconSurface.Bind(item,gem);_iconBorder.color=ItemGradeSurface.Accent(item,gem);
                _iconSparkles.Bind(item!=null&&item.grade_step>=8,item!=null&&item.grade_step==8);
                _iconFrame.gameObject.SetActive(item!=null);_icon.enabled=item!=null&&!gem;_gem.enabled=gem;
                if(item!=null)_icon.SetItem(item,item.slot);
                _copy.text=Style(string.Join("\n",lines.Skip(2)));
                _contentHeight=_copy.GetPreferredValues(_copy.text,inner,10000).y+6;
                _content.anchoredPosition=Vector2.zero;
                _titleHeight=Mathf.Max(25,_title.GetPreferredValues(_title.text,inner,10000).y+2);
                _metaHeight=Mathf.Max(38,_meta.GetPreferredValues(_meta.text,width-72,10000).y+2);
            }
            float titleHeight=_titleHeight,metaHeight=_metaHeight;
            float bodyTop=titleHeight+metaHeight+36;
            float height=Mathf.Min(maxHeight,_contentHeight+bodyTop+30);_viewHeight=height-bodyTop-30;
            _root.sizeDelta=new Vector2(width,height);
            ForgeUi.Place(_head,12,8,inner,titleHeight);_title.rectTransform.sizeDelta=new Vector2(inner,titleHeight);
            ForgeUi.Place(_iconFrame.rectTransform,12,titleHeight+16,38,38);
            ForgeUi.Place(_meta.rectTransform,item==null?12:60,titleHeight+16,item==null?inner:width-72,metaHeight);
            ForgeUi.Place(_headerRule,12,bodyTop-8,inner,1);_gradeAccent.rectTransform.sizeDelta=new Vector2(3,bodyTop-8);
            ForgeUi.Place(_viewport,12,bodyTop,inner,_viewHeight);
            _content.sizeDelta=new Vector2(inner,_contentHeight);_copy.rectTransform.sizeDelta=_content.sizeDelta;
            ForgeUi.Place(_hint.rectTransform,12,height-24,inner,18);_hint.gameObject.SetActive(CanScroll);
            if(changed)DrawRules(inner);
            var canvas=(RectTransform)transform.parent;
            RectTransformUtility.ScreenPointToLocalPointInRectangle(canvas,point,null,out var at);
            _root.anchorMin=_root.anchorMax=new Vector2(.5f,.5f);_root.pivot=new Vector2(0,1);
            float halfW=Screen.width/scale/2,halfH=Screen.height/scale/2,x=at.x+18,y=at.y+16;
            if(x+width>halfW-8)x=at.x-width-18;
            _root.anchoredPosition=new Vector2(Mathf.Clamp(x,-halfW+8,halfW-width-8),Mathf.Clamp(y,-halfH+height+8,halfH-8));
        }
        static string Style(string body)
        {
            var s=new StringBuilder();string ink="EDEBE4";
            var effects=new[]{"UI_FLOAT_AUTHORED","UI_FLOAT_BONUSES","UI_FLOAT_CARD_EFFECTS","UI_FLOAT_FOIL","UI_FLOAT_FOIL_INACTIVE","UI_FLOAT_BIND_EFFECTS","UI_FLOAT_OVERLAY_EFFECTS","UI_FLOAT_GEM_ROLLS"}.Select(Loc.Get).ToArray();
            string cardSlot=Loc.Get("UI_FLOAT_CARD_SOCKET_ROW"),gemSlot=Loc.Get("UI_FLOAT_GEM_SOCKET_ROW");
            foreach(var line in body.Split('\n'))
            {
                if(line.Length==0){ink="EDEBE4";s.Append('\n');continue;}
                bool effect=effects.Contains(line);
                // Native text remains literal; it cannot inject TMP formatting tags.
                string literal="<noparse>"+line.Replace("</noparse>","")+"</noparse>";
                if(line==cardSlot)s.Append("<b><color=#F1C86B>").Append(literal).Append("</color></b>");
                else if(line==gemSlot)s.Append("<b><color=#73D7C2>").Append(literal).Append("</color></b>");
                else if(effect||line==Loc.Get("UI_FLOAT_SOCKET_DETAILS"))s.Append("<b><color=#D9DFE4>").Append(literal).Append("</color></b>");
                else s.Append("<color=#").Append(line.StartsWith("   ",StringComparison.Ordinal)?"85D4BF":ink).Append('>').Append(literal).Append("</color>");
                s.Append('\n');
                if(effect)ink="98B2FF";
            }
            return s.ToString().TrimEnd('\n');
        }
        void DrawRules(float width)
        {
            _copy.ForceMeshUpdate();int used=0;
            foreach(var line in _copy.textInfo.lineInfo.Take(_copy.textInfo.lineCount))
            {
                if(line.characterCount>1||line.firstCharacterIndex>=_copy.textInfo.characterCount)continue;
                if(_copy.textInfo.characterInfo[line.firstCharacterIndex].character!='\n')continue;
                if(used==_rules.Count){var im=ForgeUi.Panel(_content,"SectionRule",0,0,width,1,ExemplarUi.Line);im.raycastTarget=false;_rules.Add(im);}
                var rule=_rules[used++];rule.gameObject.SetActive(true);ForgeUi.Place(rule.rectTransform,0,-line.baseline-5,width,1);
            }
            for(int i=used;i<_rules.Count;i++)_rules[i].gameObject.SetActive(false);
        }
        public void Scroll(float delta)
        {var p=_content.anchoredPosition;p.y=Mathf.Clamp(p.y-delta*36,0,Mathf.Max(0,_contentHeight-_viewHeight));_content.anchoredPosition=p;}
        public void Hide(){gameObject.SetActive(false);_text=null;}
    }
}
