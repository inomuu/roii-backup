using System.Collections.Generic;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Only character-scoped pages need a picker, and only when there is a choice.
    public sealed class PartyMemberPicker : MonoBehaviour
    {
        SimDriver _driver;
        Button _button;
        TextMeshProUGUI _label, _title;
        readonly List<Member> _members = new();
        float _next;

        public static void Create(RectTransform header, TextMeshProUGUI title)
        {
            var self=header.gameObject.AddComponent<PartyMemberPicker>();
            self._title=title;
            self._button=HudBar.MakeGhostButton(header,"",180,UiLayout.Touch,self.Open,15);
            self._button.name="MemberPicker";
            var rt=(RectTransform)self._button.transform;
            rt.anchorMin=rt.anchorMax=new Vector2(1,.5f);rt.pivot=new Vector2(1,.5f);
            rt.anchoredPosition=new Vector2(-68,0);
            self._label=self._button.GetComponentInChildren<TextMeshProUGUI>();
            self._label.enableAutoSizing=true;self._label.fontSizeMin=12;self._label.fontSizeMax=15;
            self.Refresh();
        }

        void OnEnable(){PartySelection.Changed+=Refresh;_next=0;}
        void OnDisable()=>PartySelection.Changed-=Refresh;
        void Update(){if(Time.unscaledTime>=_next){_next=Time.unscaledTime+.5f;Refresh();}}
        void Refresh()
        {
            if(_button==null)return;
            if(_driver==null)_driver=Object.FindFirstObjectByType<SimDriver>();
            _members.Clear();
            var session=_driver?.Session;
            var view=session==null?null:JsonUtility.FromJson<View>(session.ViewJson());
            if(view?.grid?.entities!=null)foreach(var member in view.grid.entities)
                if(member.team=="Player"&&!member.is_pet)_members.Add(member);
            bool visible=_members.Count>1;
            _button.gameObject.SetActive(visible);
            _title.rectTransform.offsetMax=new Vector2(visible?-260:-68,0);
            if(!visible)return;
            var ids=_members.ConvertAll(m=>m.id);
            long selected=PartySelection.Resolve(ids);
            var current=_members.Find(m=>m.id==selected);
            _label.text=GameText.UnitName(current.unit_def)+"  ›";
        }

        void Open()
        {
            Refresh();
            var options=new List<UiPicker.Option>();
            foreach(var member in _members)
            {
                long id=member.id;
                options.Add(new UiPicker.Option {
                    label=GameText.UnitName(member.unit_def),
                    detail=Loc.Fmt("UI_COMMON_LEVEL_SHORT",member.stats?.level??1),
                    onPick=()=>{Refresh();if(_members.Exists(m=>m.id==id))PartySelection.Select(id);}
                });
            }
            UiPicker.Open(Loc.Get("UI_HOME_EXEMPLARS"),options);
        }

        [System.Serializable] class View { public Grid grid; }
        [System.Serializable] class Grid { public Member[] entities; }
        [System.Serializable] class Member { public long id; public string team,unit_def; public bool is_pet; public Stats stats; }
        [System.Serializable] class Stats { public int level; }
    }
}
