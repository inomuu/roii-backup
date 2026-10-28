using System.Text;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Comparison is of the host-projected item fields, never a prediction of total character stats.
    public sealed class ExemplarGearDetails : MonoBehaviour
    {
        sealed class StatRow
        {
            public RectTransform Root;
            public TextMeshProUGUI Label,Value,Delta;
        }
        TextMeshProUGUI _slot,_title,_meta,_section,_columns,_note,_bonuses,_requirement,_lore,_empty;
        ExemplarEquipmentIcon _icon;
        RectTransform _iconHost,_scrollHost,_content;
        ScrollRect _scroll;
        Image _rule,_loreRule;
        readonly StatRow[] _stats=new StatRow[3];
        string _signature="";
        bool _picking,_comparing,_layoutReady,_fitting,_standalone;

        public static ExemplarGearDetails Create(Transform parent,bool standalone=false)
        {
            // The inspector's parent owns the continuous slate surface.
            var root=ExemplarUi.Panel(parent,"ItemDetails",0,standalone?0:.218f,1,1,Color.clear);
            var d=root.gameObject.AddComponent<ExemplarGearDetails>();
            d._standalone=standalone;d.Build();return d;
        }
        void Build()
        {
            _rule=ExemplarUi.Panel(transform,"RarityRule",0,0,1,1,ExemplarUi.Gold);
            _slot=Label(transform,"Slot",17,ExemplarUi.Gold);
            _iconHost=ExemplarUi.Rect(transform,"ItemArt",0,0,1,1);
            _icon=ExemplarEquipmentIcon.Create(_iconHost,"Weapon");
            _title=Label(transform,"ItemName",27,ExemplarUi.White);
            _title.enableAutoSizing=true;_title.fontSizeMin=23;_title.fontSizeMax=27;
            _title.overflowMode=TextOverflowModes.Ellipsis;
            _meta=Label(transform,"ItemMeta",17,ExemplarUi.Muted);
            _scrollHost=ExemplarUi.Rect(transform,"EffectScroll",.065f,.035f,.94f,.68f);
            _content=UiScroll.Create(_scrollHost,out _scroll, false);
            var track=ExemplarUi.Panel(_scrollHost,"ScrollTrack",.992f,0,1,1,ExemplarUi.Line);
            var handle=ExemplarUi.Panel(track.transform,"Handle",0,0,1,1,ExemplarUi.Muted);
            var bar=track.gameObject.AddComponent<Scrollbar>();
            bar.direction=Scrollbar.Direction.BottomToTop;bar.handleRect=handle.rectTransform;
            track.raycastTarget=true;handle.raycastTarget=true;bar.targetGraphic=handle;
            _scroll.verticalScrollbar=bar;_scroll.verticalScrollbarVisibility=ScrollRect.ScrollbarVisibility.AutoHide;
            _scroll.viewport.anchorMax=new Vector2(.967f,1);
            _section=Label(_content,"AttributeHeading",17,ExemplarUi.Gold);
            _columns=Label(_content,"CompareHeading",15,ExemplarUi.Muted);
            string[] keys={"UI_EXEMPLAR_ATK","UI_EXEMPLAR_MATK","UI_EXEMPLAR_DEF"};
            for(int i=0;i<_stats.Length;i++)
            {
                var row=new StatRow{Root=ExemplarUi.Rect(_content,"PrimaryStat"+i,0,0,1,1)};
                row.Label=Label(row.Root,"Label",19,ExemplarUi.Muted);row.Label.text=Loc.Get(keys[i]);
                row.Value=Label(row.Root,"Value",27,ExemplarUi.White,TextAlignmentOptions.TopRight);
                row.Value.enableAutoSizing=true;row.Value.fontSizeMin=21;row.Value.fontSizeMax=27;
                row.Delta=Label(row.Root,"Delta",16,ExemplarUi.Muted,TextAlignmentOptions.TopRight);
                ExemplarUi.Panel(row.Root,"Rule",0,0,1,.015f,new Color(.76f,.81f,.86f,.12f));
                _stats[i]=row;
            }
            _note=Label(_content,"ComparisonScope",16,ExemplarUi.Muted);
            _bonuses=Label(_content,"NativeEffects",19,ExemplarUi.White);
            _bonuses.lineSpacing=3;
            _requirement=Label(_content,"RequiredLevel",16,ExemplarUi.Muted);
            _loreRule=ExemplarUi.Panel(_content,"LoreRule",0,0,1,1,ExemplarUi.Line);
            _lore=Label(_content,"Lore",17,ExemplarUi.Muted);_lore.lineSpacing=3;
            _empty=Label(_content,"EmptyHint",19,ExemplarUi.Muted);
            _layoutReady=true;
        }
        public void Show(string slot,ItemView worn,ItemView candidate,bool picking)
        {
            var item=candidate??worn;_picking=picking;_comparing=candidate!=null;
            _slot.text=slot;
            _title.text=item==null?Loc.Get("UI_EXEMPLAR_EQUIP_EMPTY"):GameText.ItemName(item);
            _icon.SetItem(item,item?.slot??"Weapon");_icon.gameObject.SetActive(item!=null);
            _rule.color=item==null?ExemplarUi.Gold:ItemSlot.ItemColor(item);
            _meta.text=item==null?"":Loc.Get("UI_EXEMPLAR_LEVEL")+" "+item.item_level+"   "+GameText.GradeLabel(item.grade_step);
            _section.text=item==null?"":Loc.Get(_comparing?"UI_EX_GEAR_COMPARE":"UI_EX_GEAR_ITEM_STATS");
            _columns.text=_comparing?Loc.Get("UI_EX_GEAR_COMPARE_COLUMNS"):"";
            _note.text=_comparing?Loc.Get("UI_EX_GEAR_ITEM_ONLY"):"";
            int[] from={Atk(worn),Matk(worn),Def(worn)},to={Atk(item),Matk(item),Def(item)};
            var signature=new StringBuilder().Append(slot).Append('|').Append(_title.text).Append('|').Append(_meta.text).Append('|').Append(picking).Append('|').Append(_comparing);
            for(int i=0;i<_stats.Length;i++)
            {
                var row=_stats[i];row.Root.gameObject.SetActive(item!=null&&(_comparing||to[i]!=0));
                row.Value.text=_comparing?from[i]+" → "+to[i]:to[i].ToString();
                int delta=to[i]-from[i];
                row.Delta.text=_comparing&&delta!=0?"("+(delta>0?"+":"")+delta+")":"";
                row.Delta.color=delta<0?new Color(.89f,.66f,.61f):
                    _standalone?new Color(.61f,.80f,.67f):ExemplarUi.Gold;
                signature.Append('|').Append(from[i]).Append(':').Append(to[i]);
            }
            var effects=new StringBuilder();
            if(item!=null)
            {
                Append(effects,item.bonus_lines);Append(effects,item.card_lines);
                Append(effects,item.overlay_lines);Append(effects,item.bind_lines);
            }
            _bonuses.text=effects.ToString().TrimEnd();
            _requirement.text=item!=null&&item.level_req>0?Loc.Get("UI_EX_GEAR_REQUIRED_LEVEL")+" "+item.level_req:"";
            _lore.text=item==null?"":Loc.GetOr("ITEM_LORE_"+item.def_id,"");
            _empty.text=item==null?Loc.Get("UI_EX_GEAR_PICK_HINT"):"";
            signature.Append('|').Append(_bonuses.text).Append('|').Append(_requirement.text).Append('|').Append(_lore.text);
            string next=signature.ToString();bool reveal=_signature.Length>0&&next!=_signature;
            bool changed=next!=_signature;_signature=next;
            // The embedded inspector shares its height with actions and candidates.
            // A standalone comparison panel keeps the bounds assigned by its owning overlay.
            if(!_standalone)
            {
                var rt=(RectTransform)transform;
                rt.anchorMin=new Vector2(0,picking?.495f:.218f);rt.anchorMax=Vector2.one;
                rt.offsetMin=rt.offsetMax=Vector2.zero;
            }
            Fit();
            if(changed){_scroll.StopMovement();_scroll.verticalNormalizedPosition=1;}
            if(reveal)EquipmentDetailReveal.Play(gameObject);
        }
        void OnRectTransformDimensionsChange(){if(_layoutReady)Fit();}
        void Fit()
        {
            if(!_layoutReady||_fitting)return;
            _fitting=true;
            float panelWidth=((RectTransform)transform).rect.width;
            if(panelWidth<1)panelWidth=316;
            bool compact=_picking&&!_standalone;
            float pad=Mathf.Clamp(panelWidth*.085f,20f,28f),art=compact?84:92,header=compact?176:186;
            Top(_rule.rectTransform,0,1,8,2,pad,-pad);
            Top(_slot.rectTransform,0,1,24,25,pad,-pad);
            Top(_iconHost,0,0,60,art,pad,pad+art);
            Top(_title.rectTransform,0,1,60,art,pad+art+12,-pad);
            Top(_meta.rectTransform,0,1,header-28,24,pad,-pad);
            _scrollHost.anchorMin=Vector2.zero;_scrollHost.anchorMax=Vector2.one;
            _scrollHost.offsetMin=new Vector2(pad,14);_scrollHost.offsetMax=new Vector2(-pad,-header);
            float width=_content.rect.width>1?_content.rect.width:panelWidth-pad*2-12;
            float y=0;
            Flow(_section,ref y,width,12);Flow(_columns,ref y,width,12);
            foreach(var row in _stats)
            {
                if(!row.Root.gameObject.activeSelf)continue;
                float height=_comparing?(compact?49:_standalone?57:53):43;
                Top(row.Root,0,1,y,height);y+=height+16;
                Top(row.Label.rectTransform,0,.37f,4,30);
                Top(row.Value.rectTransform,.37f,1,0,_comparing?32:38);
                Top(row.Delta.rectTransform,.37f,1,32,compact?17:_standalone?24:20);
            }
            Flow(_note,ref y,width,15);Flow(_bonuses,ref y,width,15);
            Flow(_requirement,ref y,width,18);
            bool hasLore=!string.IsNullOrEmpty(_lore.text);_loreRule.gameObject.SetActive(hasLore);
            if(hasLore){Top(_loreRule.rectTransform,0,1,y,1);y+=18;}
            Flow(_lore,ref y,width,10);Flow(_empty,ref y,width,10);
            UiScroll.Fit(_content,y+6);
            _fitting=false;
        }
        static TextMeshProUGUI Label(Transform parent,string name,float size,Color color,TextAlignmentOptions align=TextAlignmentOptions.TopLeft)
        {
            var label=ExemplarUi.Text(parent,name,"",0,0,1,1,size,color,align);
            label.enableAutoSizing=false;label.richText=false;
            label.textWrappingMode=TextWrappingModes.Normal;label.overflowMode=TextOverflowModes.Overflow;
            return label;
        }
        static void Flow(TextMeshProUGUI label,ref float y,float width,float gap)
        {
            bool active=!string.IsNullOrEmpty(label.text);label.gameObject.SetActive(active);if(!active)return;
            float height=Mathf.Ceil(label.GetPreferredValues(label.text,Mathf.Max(1,width),Mathf.Infinity).y)+2;
            Top(label.rectTransform,0,1,y,height);y+=height+gap;
        }
        static void Top(RectTransform rt,float left,float right,float top,float height,float leftOffset=0,float rightOffset=0)
        {
            rt.anchorMin=new Vector2(left,1);rt.anchorMax=new Vector2(right,1);rt.pivot=new Vector2(.5f,1);
            rt.offsetMin=new Vector2(leftOffset,-top-height);rt.offsetMax=new Vector2(rightOffset,-top);
        }
        static void Append(StringBuilder text,string[] lines)
        {
            if(lines==null)return;
            bool wrote=false;
            foreach(var line in lines)if(!string.IsNullOrEmpty(line)){text.AppendLine(line);wrote=true;}
            if(wrote)text.AppendLine();
        }
        static int Atk(ItemView x)=>x==null?0:x.graded_atk;
        static int Matk(ItemView x)=>x==null?0:x.graded_matk;
        static int Def(ItemView x)=>x==null?0:x.graded_def;
    }
}
