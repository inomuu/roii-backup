#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.Sim;
using Roi.UI;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        FloatingItemPointer Source(Func<FloatingItemInteraction.Payload,bool> match) =>
            _host.GetComponentsInChildren<FloatingItemPointer>().First(p=>p.Payload?.Invoke() is {} data&&match(data));
        FloatingItemTarget Target(string key)=>_host.GetComponentsInChildren<FloatingItemTarget>().First(t=>t.Key==key);
        PointerEventData Pointer(FloatingItemPointer source)
        {
            Canvas.ForceUpdateCanvases();var rt=(RectTransform)source.transform;
            var point=RectTransformUtility.WorldToScreenPoint(null,rt.TransformPoint(rt.rect.center));
            var scroll=source.GetComponentInParent<ScrollRect>();
            if(scroll!=null&&!RectTransformUtility.RectangleContainsScreenPoint(scroll.viewport,point))
            {scroll.verticalNormalizedPosition=1;Canvas.ForceUpdateCanvases();point=RectTransformUtility.WorldToScreenPoint(null,rt.TransformPoint(rt.rect.center));}
            return new PointerEventData(EventSystem.current){position=point,button=PointerEventData.InputButton.Left,pointerDrag=source.gameObject};
        }
        void Context(FloatingItemPointer source)
        {
            var e=Pointer(source);e.button=PointerEventData.InputButton.Right;e.pointerDrag=null;
            ExecuteEvents.Execute(source.gameObject,e,ExecuteEvents.pointerClickHandler);
        }
        PointerEventData BeginItem(FloatingItemPointer source)
        {
            var e=Pointer(source);ExecuteEvents.Execute(source.gameObject,e,ExecuteEvents.beginDragHandler);
            Check(_host.Items.Dragged!=null,"begin drag creates frozen payload");return e;
        }
        void DropItem(FloatingItemPointer source,FloatingItemTarget target,PointerEventData e)
        {
            Canvas.ForceUpdateCanvases();var rt=(RectTransform)target.transform;e.position=RectTransformUtility.WorldToScreenPoint(null,rt.TransformPoint(rt.rect.center));
            ExecuteEvents.Execute(source.gameObject,e,ExecuteEvents.dragHandler);
            var hits=new System.Collections.Generic.List<RaycastResult>();EventSystem.current.RaycastAll(e,hits);
            var hit=hits.Select(h=>ExecuteEvents.GetEventHandler<IDropHandler>(h.gameObject)).FirstOrDefault(g=>g!=null);
            Check(hit==target.gameObject,"drop raycast reaches "+target.Key+" through drag ghost at "+e.position+" (hit "+(hit?.name??"none")+", rays: "+string.Join(",",hits.Select(h=>h.gameObject.name))+")");
            ExecuteEvents.Execute(hit,e,ExecuteEvents.dropHandler);ExecuteEvents.Execute(source.gameObject,e,ExecuteEvents.endDragHandler);
        }
        IEnumerator CheckItemDrag()
        {
            _driver.Paused=true;_driver.Session.Debug("{\"add_levels\":{\"entity_id\":1,\"n\":20}}");
            _host.Open();_host.Side(0);_host.RefreshNow();yield return new WaitForSecondsRealtime(.5f);
            var contextSword=Source(p=>p.Row?.OwnerId==0&&p.Item.weapon_type=="Sword1H");
            var contextSwordId=contextSword.Payload().Item.def_id;Context(contextSword);yield return new WaitForSecondsRealtime(.3f);
            Check(ExemplarEquipmentActions.Read(_driver,_host.MemberId).Equipped.Any(r=>r.Item.def_id==contextSwordId),"right-click equips gear while Forge is closed");
            _host.Side(1);_host.Model.Mode(false);_host.Model.Clear();yield return new WaitForSecondsRealtime(.3f);
            var contextGear=Source(p=>p.Row?.OwnerId==0&&p.Item.kind=="Armor"&&_host.Model.Eligible(p.Item));
            Context(contextGear);yield return null;
            Check(_host.Model.Gear.Count==1&&_host.Model.Gear[0].Index==contextGear.Payload().Row.Index,"right-click adds gear to open Forge");
            _host.Side(0);yield return null;Check(_host.Model.Gear.Count==0,"closing Forge clears its selected gear draft");
            _host.Side(1);_host.Model.Mode(true);_host.Model.Clear();yield return new WaitForSecondsRealtime(.3f);
            var contextGem=Source(p=>p.Row==null&&p.Item.def_id=="gem_strife_g0");Context(contextGem);yield return null;
            Check(_host.Model.Gems.Count==1&&_host.Model.Gems[0]=="gem_strife_g0","right-click adds Gem to open Forge");
            _host.Model.Clear();
            _host.Side(0);yield return null;
            var sword=Source(p=>p.Row?.OwnerId==0&&p.Item.weapon_type=="Sword1H");
            var e=Pointer(sword);ExecuteEvents.Execute(sword.gameObject,e,ExecuteEvents.pointerEnterHandler);
            int selected=_host.InspectedIndex;
            yield return new WaitForSecondsRealtime(.3f);
            Check(_host.Items.Tooltip.Visible,"hover opens tooltip while HudBar is hidden");
            string copy=_host.Items.Tooltip.Text;
            Check(copy.Contains(ForgeUi.Name(sword.Payload().Item))&&copy.Contains(Loc.Get("UI_EX_GEAR_REQUIRED_LEVEL"))
                &&copy.Contains(Loc.Get("UI_EXEMPLAR_ATK")+"  "+sword.Payload().Item.graded_atk),"hover includes item name, requirement and actual graded ATK");
            Check(_host.InspectedIndex==selected,"hover does not change clicked selection");
            ExecuteEvents.Execute(sword.gameObject,e,ExecuteEvents.pointerExitHandler);
            Check(!_host.Items.Tooltip.Visible,"pointer exit hides tooltip");
            var session=_driver.Session;string hash=session.StateHash();
            e=BeginItem(sword);
            Check(session.StateHash()==hash,"native slot preview does not mutate live simulation");
            Check(Target("equip:Weapon").Highlight==true&&Target("equip:Helmet").Highlight==false,"native-valid weapon slot is green and incompatible helmet is red");
            yield return Shot("13-drag-eligible");
            DropItem(sword,Target("equip:Helmet"),e);
            Check(session.StateHash()==hash,"red drop leaves original item and loadout untouched");
            e=BeginItem(sword);var chosen=sword.Payload().Row;
            DropItem(sword,Target("equip:Weapon"),e);
            var after=ExemplarEquipmentActions.Read(_driver,_host.MemberId);
            Check(after.Equipped.Any(r=>r.Item.def_id==chosen.Item.def_id)&&session.StateHash()!=hash,"green drop equips through real native input");
            Check(_host.Items.Dragged==null&&Target("equip:Weapon").Highlight==null,"drop clears ghost and outlines");
            var worn=Source(p=>p.Row?.OwnerId==_host.MemberId&&p.Item.slot=="Weapon");
            e=BeginItem(worn);Check(Target("inventory").Highlight==true,"worn gear can return to bag");
            int bagBefore=_host.Model.Bag.Length;
            DropItem(worn,Target("inventory"),e);
            Check(_host.Model.Bag.Length==bagBefore+1&&!ExemplarEquipmentActions.Read(_driver,_host.MemberId).Equipped.Any(r=>r.Slot=="Weapon"),"drop into bag unequips actual native gear");
            // Stale collection: removing an earlier row must never substitute a different item.
            var candidate=Source(p=>p.Row?.OwnerId==0&&p.Row.Index>0&&p.Item.kind=="Armor");
            e=BeginItem(candidate);_driver.ApplyInput("{\"SellInventoryItem\":{\"inv_idx\":0}}");_host.RefreshNow();
            hash=session.StateHash();
            ExecuteEvents.Execute(Target("equip:Shield").gameObject,e,ExecuteEvents.dropHandler);
            ExecuteEvents.Execute(candidate.gameObject,e,ExecuteEvents.endDragHandler);
            Check(session.StateHash()==hash&&_host.Items.Dragged==null,"inventory index shift cancels drag without moving another item");
            _host.Side(1);_host.Model.Mode(false);_host.Model.Clear();yield return new WaitForSecondsRealtime(.6f);
            sword=Source(p=>p.Row?.OwnerId==0&&p.Item.weapon_type=="Sword1H");
            e=BeginItem(sword);Check(Target("forge:0").Highlight==true,"eligible gear highlights next Forge input green");
            DropItem(sword,Target("forge:0"),e);Check(_host.Model.Gear.Count==1,"gear drop selects exactly one Forge material");
            e=BeginItem(sword);Check(Target("forge:1").Highlight==false,"already-selected gear cannot be added twice");_host.Items.Cancel();
            _host.Model.Mode(true);_host.Model.Clear();
            var gem=Source(p=>p.Row==null&&p.Item.def_id=="gem_strife_g0");e=BeginItem(gem);
            Check(Target("equip:Weapon").Highlight==false&&Target("forge:0").Highlight==true,"gem can go in gem forge but not character equipment slot");
            DropItem(gem,Target("forge:0"),e);Check(_host.Model.Gems.Count==1,"gem drop selects one held unit");
            _host.Model.Mode(false);_host.Model.Clear();_host.Model.AutoFill();
            Click(Find(_host.transform,"UI_FORGE_COMBINE"));_host.Back();_host.Side(2);
            yield return new WaitForSecondsRealtime(.6f);
            int socketIndex=Array.FindIndex(_host.Model.Bag,it=>it.gem_slots>0);
            Check(socketIndex>=0,"native fused gear supplies a socket target");
            var bag=_host.GetComponentsInChildren<ScrollRect>().First(r=>r.name=="BagScroll").content;
            Click(bag.GetComponentsInChildren<Button>().Where(b=>b.name=="Item").ElementAt(socketIndex));
            yield return null;
            var socketTarget=Source(p=>p.Row?.OwnerId==0&&p.Row.Index==socketIndex);Context(socketTarget);yield return null;
            Check(_host.InspectedIndex==socketIndex,"right-click selects equipment in Socket");
            var emptySocket=Source(p=>p.Row?.OwnerId==0&&p.Row.Index==socketIndex);
            _host.Items.Hover(emptySocket);yield return new WaitForSecondsRealtime(.3f);yield return Shot("16-empty-socket-hover");_host.Items.Cancel();
            var socketPanel=_host.transform.Find("WindowGroup/SocketGems");
            var contextSocketGem=socketPanel.GetComponentsInChildren<FloatingItemPointer>().First(p=>p.Payload?.Invoke()?.Item.def_id=="gem_strife_g0");
            Context(contextSocketGem);yield return null;
            Check(Find(socketPanel,"UI_FLOAT_SOCKET_PICK_GEM").GetComponentInChildren<TMP_Text>().text==Loc.Get("UI_FLOAT_SOCKET_PICK_EQUIPMENT"),"right-click selects Gem first in Socket");
            Context(socketTarget);yield return null;
            gem=socketPanel.GetComponentsInChildren<FloatingItemPointer>().First(p=>p.Payload?.Invoke()?.Item.def_id=="gem_strife_g0");
            e=BeginItem(gem);Check(Target("socket:0").Highlight==true,"fitting gem socket is green");
            hash=session.StateHash();DropItem(gem,Target("socket:0"),e);
            Check(session.StateHash()==hash&&_host.Items.Dragged==null,"socket drop requests confirmation before spending gem");
            yield return null;Click(Find(_host.transform,"UI_FLOAT_CONFIRM"));
            Check(_host.Model.Bag[socketIndex].gems?.Length==1,"confirmed drop inserts real held gem");
            var socketed=Source(p=>p.Row?.OwnerId==0&&p.Row.Index==socketIndex);
            var gearItem=socketed.Payload().Item;string nativeTip=socketed.Description();
            Check(nativeTip.Contains(Loc.GetOr("ITEM_gem_strife_g0","gem_strife_g0"))&&gearItem.gem_line_groups.All(g=>g.lines.All(nativeTip.Contains)),"socketed gear hover contains every actual native gem roll");
            _host.Items.Hover(socketed);yield return new WaitForSecondsRealtime(.3f);yield return Shot("15-socketed-hover");_host.Items.Cancel();
            gem=socketPanel.GetComponentsInChildren<FloatingItemPointer>().First(p=>p.Payload?.Invoke()?.Item.def_id=="gem_life_g0");
            e=BeginItem(gem);Check(Target("socket:0").Highlight==false,"wrong gem group is red");
            hash=session.StateHash();DropItem(gem,Target("socket:0"),e);Check(session.StateHash()==hash,"wrong gem drop consumes nothing");
            _host.Side(1);_host.Model.Mode(true);yield return new WaitForSecondsRealtime(.3f);
            // Long full-detail content is retained and scrollable, including the final line.
            var item=new ItemView{def_id="sword_s1",kind="Weapon",slot="Weapon",grade_step=2,graded_atk=55,graded_matk=12,graded_def=3,
                level_req=1,quantity=1,bound=true,unique=true,socket_count=2,gem_slots=3,
                authored_lines=new[]{"AUTHORED"},bonus_lines=Enumerable.Range(0,45).Select(i=>"BONUS "+i).ToArray(),
                gems=new[]{new SocketedGemView{def_id="gem_strife_g0"}},gem_line_groups=new[]{new GemLineGroup{lines=new[]{"ROLLED GEM +7 ATK"}}},
                card_lines=new[]{"CARD EFFECT"},card_foil_lines=new[]{"FOIL EFFECT"},bind_lines=new[]{"BIND EFFECT"},overlay_lines=new[]{"OVERLAY EFFECT"}};
            string description=FloatingItemDescription.Format(item);
            foreach(string part in new[]{"AUTHORED","BONUS 44","ROLLED GEM +7 ATK","CARD EFFECT","FOIL EFFECT","BIND EFFECT","OVERLAY EFFECT",Loc.Get("UI_ITEM_BOUND")})
                Check(description.Contains(part),"complete tooltip retains "+part);
            _host.Items.Tooltip.Show(description,new Vector2(Screen.width-4,Screen.height-4));
            Check(_host.Items.Tooltip.CanScroll,"long tooltip is scrollable without shrinking or truncating content");
            var rect=(RectTransform)_host.Items.Tooltip.transform;var corners=new Vector3[4];rect.GetWorldCorners(corners);
            Check(corners[0].x>=0&&corners[0].y>=0&&corners[2].x<=Screen.width&&corners[2].y<=Screen.height,"tooltip stays inside scaled native window");
            _host.Items.Tooltip.Scroll(-100);yield return Shot("14-full-tooltip");_host.Items.Tooltip.Hide();
            // Modal and mode changes cannot leave an active drag or invisible blockers.
            gem=Source(p=>p.Row==null&&p.Item.def_id=="gem_strife_g0");e=BeginItem(gem);_host.Back();
            Check(_host.Items.Dragged==null&&_host.ActiveSide==1,"Escape/back cancels drag before closing Forge");
            e=BeginItem(gem);_companion.Toggle();yield return new WaitForSecondsRealtime(.5f);
            Check(_host.Items.Dragged==null&&!_host.CompanionContext,"leaving Companion cancels current drag");
            _host.Open();_host.Side(0);yield return new WaitForSecondsRealtime(.3f);
            sword=Source(p=>p.Row?.OwnerId==0&&p.Item.weapon_type=="Sword1H");
            e=Pointer(sword);ExecuteEvents.Execute(sword.gameObject,e,ExecuteEvents.pointerEnterHandler);
            yield return new WaitForSecondsRealtime(.25f);
            Check(_host.Items.Tooltip.Visible,"same full tooltip works in Battle");
            e=BeginItem(sword);Check(Target("equip:Weapon").Highlight==true,"same native slot highlight works in Battle");
            DropItem(sword,Target("equip:Weapon"),e);
            Check(ExemplarEquipmentActions.Read(_driver,_host.MemberId).Equipped.Any(r=>r.Slot=="Weapon"),"same native equipment drop works in Battle");
            _driver.Paused=false;ulong tick=_driver.Tick;yield return new WaitForSecondsRealtime(.6f);
            Check(_driver.Tick>tick,"combat continues after drag operations");
            Check(_runtimeErrors==0,"drag and hover produce no runtime errors");
            yield return CaptureGradeSurfaces();
            File.WriteAllLines(Path.Combine(Output,"drag-verification.txt"),_checks);
            Debug.Log("DRAG NATIVE COMPLETE: "+_checks.Count+" checks");Application.Quit(0);
        }
    }
}
#endif
