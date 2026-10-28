using System;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public static class PartySlotPurchase
    {
        [Serializable] public sealed class Meta
        {
            public ulong gil;
            public long slot_cost=-1;
            public int party_slots, party_max, slot_reward_tickets;
        }
        public static Meta Read(SimSession session)
            => session==null ? null : ForgeModel.Parse<Meta>(session.QueryJson("party_meta"));
        public static bool CanBuy(Meta m) => m!=null && m.party_slots<m.party_max
            && m.slot_cost>=0 && m.gil>=(ulong)m.slot_cost;
        public static VerifiedInputResult Buy(SimDriver driver,SimSession expected,int slots,long price)
        {
            if(driver==null||driver.Session!=expected)return VerifiedInputResult.Stale;
            var before=Read(expected);
            if(!CanBuy(before)||before.party_slots!=slots||before.slot_cost!=price)return VerifiedInputResult.Stale;
            return driver.ApplyVerifiedInputs(expected,new[]{"\"BuyPartySlot\""},copy=>
            {
                var after=Read(copy);
                return after!=null&&after.party_slots==slots+1&&after.gil==before.gil-(ulong)price;
            });
        }
    }

    // Shared by the combat strip and character member tabs. Only the next locked slot can buy.
    public sealed class PartySlotUnlock : MonoBehaviour
    {
        SimDriver _driver;
        int _index;
        Button _button;
        TMP_Text _label;
        float _next,_noticeUntil;
        string _notice;
        public static PartySlotUnlock Create(RectTransform parent,int index)
        {
            var bg=UiTheme.Flat(parent,"PartySlotUnlock",new Color(.055f,.07f,.09f,.84f));
            var rt=bg.rectTransform; rt.anchorMin=Vector2.zero;rt.anchorMax=Vector2.one;rt.offsetMin=rt.offsetMax=Vector2.zero;
            UiTheme.Border(rt,UiTheme.Line);
            var self=bg.gameObject.AddComponent<PartySlotUnlock>();self._index=index;
            self._button=bg.gameObject.AddComponent<Button>();self._button.targetGraphic=bg;
            self._button.onClick.AddListener(self.Buy);
            self._label=UiTheme.Label(rt,"UnlockLabel","",11,UiTheme.TextMid,TextAlignmentOptions.Center);
            var lr=self._label.rectTransform;lr.anchorMin=Vector2.zero;lr.anchorMax=Vector2.one;
            lr.offsetMin=new Vector2(5,3);lr.offsetMax=new Vector2(-5,-3);
            self._label.enableAutoSizing=true;self._label.fontSizeMin=7;self._label.fontSizeMax=11;
            self._label.textWrappingMode=TextWrappingModes.Normal;self._label.raycastTarget=false;
            self.Refresh();return self;
        }
        void Update(){if(Time.unscaledTime>=_next){_next=Time.unscaledTime+.4f;Refresh();}}
        void Refresh()
        {
            if(_driver==null)_driver=FindFirstObjectByType<SimDriver>();
            var m=PartySlotPurchase.Read(_driver?.Session);
            _button.interactable=m!=null&&_index==m.party_slots&&PartySlotPurchase.CanBuy(m);
            if(Time.unscaledTime<_noticeUntil){_label.text=_notice;return;}
            if(m==null){_label.text="";return;}
            if(_index<m.party_slots){_label.text=Loc.Get("UI_PARTY_SLOT_EMPTY");return;}
            _label.text=_index==m.party_slots
                ? Loc.Fmt("UI_PARTY_SLOT_UNLOCK",UiNumber.Exact((ulong)Math.Max(0,m.slot_cost)))+"\n"+Loc.Fmt("UI_PARTY_SLOT_REWARD",m.slot_reward_tickets)
                : Loc.Get("UI_PARTY_SLOT_PREVIOUS");
            _label.color=_button.interactable?ForgeUi.Gold:UiTheme.TextLow;
        }
        void Buy()
        {
            var session=_driver?.Session;var m=PartySlotPurchase.Read(session);
            if(m==null||_index!=m.party_slots||!PartySlotPurchase.CanBuy(m))return;
            var result=PartySlotPurchase.Buy(_driver,session,m.party_slots,m.slot_cost);
            _notice=result==VerifiedInputResult.Applied?Loc.Fmt("UI_PARTY_SLOT_BOUGHT",m.slot_reward_tickets):Loc.Get("UI_TRAIN_STALE");
            _noticeUntil=Time.unscaledTime+8;Refresh();
        }
    }
}
