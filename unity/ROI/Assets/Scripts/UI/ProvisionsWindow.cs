// PROVISIONS — what each character eats, drinks and falls back on. Ported from
// godot/views/provisions_view.gd: one row per deployed member, three EMERGENCY slots that auto-use
// on their trigger, and two RESERVED slots, a Meal and a Tonic.
//
// NO DRAG (user, 2026-09-07 — mobile is planned). Filling a slot is: tap the slot, tap the item in
// the picker that opens. That is the Godot flow too (`CarrierPicker`), so this is parity.
//
// WHY THE TWO KINDS OF SLOT DIFFER, because the code would otherwise look inconsistent:
//   emergency — takes the WHOLE stack out of the bag. It is a panic button; splitting it would
//               leave a character with two potions and no way to know.
//   reserved  — takes a QUANTITY and leaves the rest in the bag. Slots are per-character while
//               stackables merge into one inventory stack, so moving the whole stack would let the
//               first member take every Meal and leave the other four unable to eat. The sim says
//               so outright at `InputEvent::AddProvision`, and the picker offers 1 / 10 / all
//               because of it.
//
// Everything here is a read of the per-tick view plus the inventory query, and every write is an
// `InputEvent`. No new FFI.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ProvisionsWindow : MonoBehaviour
    {
        const int Emergency = 3;                       // EMERGENCY_SLOTS
        const float HintH = 34f, HeadH = 26f, RowH = 96f, Gap = 8f;
        const float NameW = 148f, CellW = 138f;

        Roi.Sim.SimDriver _driver;
        RectTransform _body, _content;
        TextMeshProUGUI _hint;
        GameObject _picker;
        static ProvisionsWindow _pickerOwner;
        static int _pickerClosedFrame=-1;
        public static bool BlocksNavigation=>_pickerClosedFrame==Time.frameCount||(_pickerOwner!=null&&_pickerOwner._picker!=null);
        void Update(){if(_picker!=null&&UnityEngine.InputSystem.Keyboard.current?.escapeKey.wasPressedThisFrame==true)ClosePicker();}
        void OnDisable(){ClosePicker();}
        readonly List<GameObject> _built = new();
        string _fingerprint = "";

        public static ProvisionsWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<ProvisionsWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            _body = body;
            float pad = UiTheme.PadLarge;
            _hint = UiTheme.Label(body, "Hint",
                Loc.Get("UI_PROVISIONS_EMERGENCY_SLOTS_AUTO_USE_ON"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
            var hr = _hint.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(pad, -HintH); hr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(HintH + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _hint.text = Loc.Get("UI_PROVISIONS_NO_SESSION"); return; }

            var view = JsonUtility.FromJson<ProvView>(session.ViewJson());
            var members = new List<ProvEntity>();
            if (view != null && view.grid != null && view.grid.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet && e.alive) members.Add(e);

            // Fingerprint the SLOTS, not the fight: hp and cooldowns tick ten times a second, and
            // rebuilding a button under the pointer eats the click that was already on its way.
            var fp = new System.Text.StringBuilder();
            foreach (var m in members)
            {
                fp.Append(m.id).Append('[');
                foreach (var c in Slots(m.consumables)) fp.Append(c.item.def_id).Append(':').Append(c.item.quantity).Append(',');
                fp.Append('|');
                foreach (var p in Slots(m.provisions)) fp.Append(p.item.def_id).Append(':').Append(p.item.quantity).Append(',');
                fp.Append(']');
            }
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();
            Rebuild(members);
        }

        static ConsumableSlotDTO[] Slots(ConsumableSlotDTO[] a) => a ?? new ConsumableSlotDTO[0];

        void Rebuild(List<ProvEntity> members)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            ClosePicker();

            float y = 0f;
            Header(ref y);
            if (members.Count == 0) { Note(ref y, "Nobody is deployed."); UiScroll.Fit(_content, y); return; }

            foreach (var m in members)
            {
                var row = new GameObject("Row_" + m.id, typeof(RectTransform));
                row.transform.SetParent(_content, false);
                var rt = (RectTransform)row.transform;
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.sizeDelta = new Vector2(0f, RowH);
                rt.anchoredPosition = new Vector2(0f, -y);
                _built.Add(row);
                GameMenuSurface.Add(rt);

                var nameT = UiTheme.Label(rt, "Name", GameText.UnitName(m.unit_def), UiTheme.BodySize,
                                          UiTheme.TextHigh, TextAlignmentOptions.Left);
                var nr = nameT.rectTransform;
                nr.anchorMin = nr.anchorMax = new Vector2(0f, 0.5f);
                nr.pivot = new Vector2(0f, 0.5f);
                nr.sizeDelta = new Vector2(NameW, 20f);
                nr.anchoredPosition = new Vector2(18f, 0f);
                nr.sizeDelta=new Vector2(NameW-24,44);
                nameT.textWrappingMode=TextWrappingModes.Normal;
                UiTheme.Display(nameT);nameT.fontSize=20;

                long id = m.id;
                var cons = Slots(m.consumables);
                for (int i = 0; i < Emergency; i++)
                {
                    int slot = i;
                    var held = i < cons.Length ? cons[i].item : null;
                    Cell(rt, ColX(i), held, UiTheme.TextHigh,
                         held != null
                            ? (System.Action)(() => Send($"{{\"RemoveConsumable\":{{\"entity_id\":{id},\"slot\":{slot}}}}}"))
                            : () => OpenPicker(id, null));
                }
                // The reserved slots are found BY KIND, not by position: the arrays shrink when a
                // stack runs out, so a Tonic would slide onto the Meal column the moment the meal
                // was eaten.
                var provs = Slots(m.provisions);
                Reserved(rt, ColX(Emergency), id, provs, "Meal");
                Reserved(rt, ColX(Emergency + 1), id, provs, "Tonic");

                y += RowH + 4f;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        static float ColX(int col) => NameW + Gap + col * (CellW + Gap);

        void Reserved(RectTransform row, float x, long id, ConsumableSlotDTO[] provs, string kind)
        {
            int idx = -1;
            for (int i = 0; i < provs.Length; i++)
                if (provs[i].item != null && provs[i].item.consumable != null
                    && provs[i].item.consumable.provision == kind) { idx = i; break; }
            var held = idx >= 0 ? provs[idx].item : null;
            int slot = idx;
            Cell(row, x, held, kind == "Meal" ? new Color(0.45f, 0.78f, 0.50f) : new Color(0.44f, 0.66f, 0.92f),
                 held != null
                    ? (System.Action)(() => Send($"{{\"RemoveProvision\":{{\"entity_id\":{id},\"slot\":{slot}}}}}"))
                    : () => OpenPicker(id, kind));
        }

        /// One slot. Filled slots carry the item's name; an empty one is a "+" and nothing else —
        /// a row of five identical "Empty" labels reads as five things rather than as free space.
        void Cell(RectTransform row, float x, ItemView held, Color tint, System.Action onTap)
        {
            bool has = held != null && !string.IsNullOrEmpty(held.def_id);
            string label = has
                ? (held.quantity > 1 ? held.name + "  ×" + held.quantity : held.name)
                : "+";
            var btn = HudBar.MakeButton(row, label, "", CellW, 64f, onTap);
            var t = btn.GetComponentInChildren<TextMeshProUGUI>();
            t.fontSize = UiTheme.CaptionSize;
            t.color = has ? tint : UiTheme.TextLow;
            var rt = btn.GetComponent<RectTransform>();
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
            rt.pivot = new Vector2(0f, 0.5f);
            rt.anchoredPosition = new Vector2(x, 0f);
            if (!has) { btn.GetComponent<Image>().color = UiTheme.Panel; t.fontSize=26; }
        }

        void Header(ref float y)
        {
            string[] names = { "Emergency", "", "", "Meal", "Tonic" };
            for (int i = 0; i < names.Length; i++)
            {
                if (names[i] == "") continue;
                var t = UiTheme.Caps(_content, "H" + i, names[i], UiTheme.CaptionSize,
                                     UiTheme.TextLow, TextAlignmentOptions.Left);
                var rt = t.rectTransform;
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.sizeDelta = new Vector2(CellW * 2f, HeadH);
                rt.anchoredPosition = new Vector2(ColX(i), -y);
                _built.Add(t.gameObject);
            }
            y += HeadH + 2f;
        }

        void Note(ref float y, string text)
        {
            var t = UiTheme.Label(_content, "Note", text, UiTheme.CaptionSize,
                                  UiTheme.TextLow, TextAlignmentOptions.Left);
            var rt = t.rectTransform;
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, 26f);
            rt.anchoredPosition = new Vector2(0f, -y);
            _built.Add(t.gameObject);
            y += 26f;
        }

        // --- the picker ----------------------------------------------------------------------

        /// `kind` null = the emergency bar, which takes any Consumable and takes the whole stack.
        /// Otherwise the reserved slot for that provision kind, which takes a quantity.
        void OpenPicker(long entityId, string kind)
        {
            ClosePicker();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;

            var list = JsonUtility.FromJson<ItemList>("{\"items\":" + session.QueryJson("inventory") + "}");
            var items = list != null && list.items != null ? list.items : new ItemView[0];

            var root = new GameObject("Picker", typeof(RectTransform));
            root.transform.SetParent(_body, false);
            _picker = root;_pickerOwner=this;
            var pickerRect=(RectTransform)root.transform;
            pickerRect.anchorMin=Vector2.zero;pickerRect.anchorMax=Vector2.one;
            pickerRect.offsetMin=pickerRect.offsetMax=Vector2.zero;
            var scrim = UiTheme.Flat(root.transform, "Scrim", UiTheme.Scrim);
            var sr = scrim.rectTransform;
            sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
            sr.offsetMin = Vector2.zero; sr.offsetMax = Vector2.zero;
            var close = scrim.gameObject.AddComponent<Button>();
            close.transition = Selectable.Transition.None;
            close.onClick.AddListener(ClosePicker);   // a tap outside dismisses, as everywhere else

            var panel = UiTheme.Box(root.transform, "Panel", UiTheme.Panel);
            DialogSurface.Apply(panel);
            DialogSurface.CloseIcon(panel.transform,ClosePicker);
            var pr = panel.rectTransform;
            pr.anchorMin = pr.anchorMax = new Vector2(0.5f, 0.5f);
            pr.pivot = new Vector2(0.5f, 0.5f);
            pr.sizeDelta = new Vector2(400f, 340f);

            var titleT = UiTheme.Caps(pr, "Title", Loc.Get(kind=="Meal"?"UI_UX_MEAL":kind=="Tonic"?"UI_UX_TONIC":"UI_UX_EMERGENCY"), UiTheme.HeadSize,
                                      UiTheme.TextMid, TextAlignmentOptions.Left);
            var title = titleT.rectTransform;
            title.anchorMin = new Vector2(0f, 1f); title.anchorMax = new Vector2(1f, 1f);
            title.pivot = new Vector2(0.5f, 1f);
            title.sizeDelta = new Vector2(-UiTheme.Pad * 2f, 34f);
            title.anchoredPosition = new Vector2(0f, -UiTheme.PadSmall);

            var listHost = new GameObject("List", typeof(RectTransform));
            listHost.transform.SetParent(pr, false);
            var lh = (RectTransform)listHost.transform;
            lh.anchorMin = Vector2.zero; lh.anchorMax = Vector2.one;
            lh.offsetMin = new Vector2(UiTheme.PadSmall, 68f);
            lh.offsetMax = new Vector2(-UiTheme.PadSmall, -46f);
            var content = UiScroll.Create(lh, out _);

            float y = 0f;
            int offered = 0;
            for (int i = 0; i < items.Length; i++)
            {
                var it = items[i];
                if (it.kind != "Consumable") continue;
                bool isProv = it.consumable != null && !string.IsNullOrEmpty(it.consumable.provision);
                if (kind == null) { if (isProv) continue; }        // reserved items go in their own slot
                else if (!isProv || it.consumable.provision != kind) continue;

                int invIdx = i;
                if (kind == null)
                {
                    AddPickRow(content, ref y, it.name, it.quantity > 1 ? "×" + it.quantity : "",
                        () => Send($"{{\"AddConsumable\":{{\"entity_id\":{entityId},\"inv_idx\":{invIdx}}}}}"));
                    offered++;
                }
                else
                {
                    // 1 / 10 / all, deduped — the Godot picker's own offer. A reserved slot takes a
                    // quantity precisely so one member cannot take the party's whole supply.
                    var qtys = new List<int> { 1 };
                    if (it.quantity >= 10) qtys.Add(10);
                    if (it.quantity > 1 && !qtys.Contains(it.quantity)) qtys.Add(it.quantity);
                    foreach (var q in qtys)
                    {
                        int qty = q;
                        AddPickRow(content, ref y, it.name, "×" + qty,
                            () => Send($"{{\"AddProvision\":{{\"entity_id\":{entityId},\"inv_idx\":{invIdx},\"qty\":{qty}}}}}"));
                        offered++;
                    }
                }
            }
            if (offered == 0)
            {
                var empty = UiTheme.Label(content, "None",
                    Loc.Get(kind==null?"UI_UX_NO_CONSUMABLES":kind=="Meal"?"UI_UX_NO_MEALS":"UI_UX_NO_TONICS"),
                    UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
                var er = empty.rectTransform;
                er.anchorMin = new Vector2(0f, 1f); er.anchorMax = new Vector2(1f, 1f);
                er.pivot = new Vector2(0.5f, 1f);
                empty.textWrappingMode=TextWrappingModes.Normal;
                er.sizeDelta = new Vector2(0f, 90f);
                y = 90f;
                pr.sizeDelta=new Vector2(400,220);
                if(kind==null){
                    var shop=HudBar.MakeButton(pr,GameText.ScreenName("Shop"),"",160,44,()=>{
                        ClosePicker();Object.FindFirstObjectByType<HudBar>()?.Toggle("Shop");
                        Object.FindFirstObjectByType<ShopWindow>()?.BrowseSupplies();
                    },true);
                    shop.name="BrowseSupplies";
                    var br=shop.GetComponent<RectTransform>();br.anchorMin=br.anchorMax=Vector2.zero;br.pivot=Vector2.zero;br.anchoredPosition=new Vector2(12,12);
                }
            }
            UiScroll.Fit(content, y);

            var cancel = HudBar.MakeGhostButton(pr, Loc.Get("UI_COMMON_CANCEL"), 140f, 44f, ClosePicker, UiTheme.CaptionSize);
            var cr = cancel.GetComponent<RectTransform>();
            cr.anchorMin = cr.anchorMax = new Vector2(1f, 0f);
            cr.pivot = new Vector2(1f, 0f);
            cr.anchoredPosition = new Vector2(-UiTheme.PadSmall, UiTheme.PadSmall);
        }

        void AddPickRow(RectTransform content, ref float y, string label, string value, System.Action onPick)
        {
            var row = UiRow.Create(content);
            row.Set(label, value, () => { onPick(); ClosePicker(); });
            var rt = (RectTransform)row.transform;
            rt.anchoredPosition = new Vector2(0f, -y);
            y += UiRow.Height;
        }

        void ClosePicker()
        {
            if(_picker!=null){_pickerClosedFrame=Time.frameCount;_picker.SetActive(false);Destroy(_picker);}
            if(_pickerOwner==this)_pickerOwner=null;
            _picker = null;
        }

        void Send(string eventJson)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            session.ApplyInput(eventJson);
            _fingerprint = "";
            Refresh();
        }

        // --- the shapes this screen reads ----------------------------------------------------

        [System.Serializable] class ProvView { public GridDTO grid; }
        [System.Serializable] class GridDTO { public ProvEntity[] entities; }
        [System.Serializable]
        class ProvEntity
        {
            public long id; public string team, unit_def;
            public bool is_pet, alive;
            public ConsumableSlotDTO[] consumables, provisions;
        }
        [System.Serializable] public class ConsumableSlotDTO { public ItemView item; }
    }
}
