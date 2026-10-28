using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    // One controller on the persistent floating canvas. No global item or scene-wide polling.
    public sealed class FloatingItemInteraction : MonoBehaviour
    {
        public sealed class Payload
        {
            public ItemView Item;
            public ExemplarEquipmentActions.Row Row;
            public Roi.Sim.SimSession Session;
            public long Member;
            public int Side;
            public bool GemMode;
            public string EquipSlot,Loadout,Inventory;
        }
        public static readonly Color Allowed=new(.22f,1f,.43f,1), Denied=new(1f,.24f,.25f,1);
        public Payload Dragged { get; private set; }
        public FloatingItemTooltip Tooltip { get; private set; }
        FloatingInventoryHost _host;
        FloatingItemPointer _source,_hover;
        ForgeCell _ghost;
        float _hoverAt;
        bool _hoverDirty;string _hoverText;ItemView _hoverItem;
        readonly List<FloatingItemTarget> _targets=new();
        public void Bind(FloatingInventoryHost host)
        {
            _host=host; Tooltip=FloatingItemTooltip.Create(transform);
            _ghost=new ForgeCell(transform);_ghost.Root.name="DraggedItem";
            foreach(var g in _ghost.Root.GetComponentsInChildren<Graphic>(true))g.raycastTarget=false;
            _ghost.Root.gameObject.SetActive(false);
        }
        public FloatingItemPointer Source(RectTransform root,Func<Payload> payload,Func<string> description,Func<ItemView> preview=null)
        {
            var p=root.GetComponent<FloatingItemPointer>()??root.gameObject.AddComponent<FloatingItemPointer>();
            p.Controller=this;p.Payload=payload;p.Description=description;p.Preview=preview??(()=>payload?.Invoke()?.Item);return p;
        }
        public void Target(RectTransform root,string key,Func<Payload,bool> accepts,Action<Payload> drop)
        {
            var t=root.GetComponent<FloatingItemTarget>();
            if(t==null){t=root.gameObject.AddComponent<FloatingItemTarget>();t.Build(this);_targets.Add(t);}
            t.Key=key;t.Accepts=accepts;t.Drop=drop;
        }
        public void Hover(FloatingItemPointer source)
        {if(Dragged!=null)return;_hover=source;_hoverAt=Time.unscaledTime+.18f;_hoverDirty=true;Tooltip.Hide();}
        public void Unhover(FloatingItemPointer source)
        {if(_hover!=source)return;_hover=null;Tooltip.Hide();}
        public void Begin(FloatingItemPointer source,PointerEventData e)
        {
            Cancel();if(e.button!=PointerEventData.InputButton.Left||!_host.ItemsAvailable)return;
            var p=source.Payload?.Invoke();if(p?.Item==null||!_host.DragStillValid(p))return;
            if(p.Row?.OwnerId==0)_host.MarkBagRead(p.Row.Index);
            _host.PrepareDrag(p);Dragged=p;_source=source;
            _ghost.Bind(p.Item,null,false,_host.Model.IsGem(p.Item));_ghost.Root.gameObject.SetActive(true);
            _ghost.Root.SetAsLastSibling();Move(e.position);RefreshTargets();
        }
        public void Move(Vector2 screen)
        {
            if(Dragged==null)return;
            RectTransformUtility.ScreenPointToLocalPointInRectangle((RectTransform)transform,screen,null,out var local);
            _ghost.Root.anchorMin=_ghost.Root.anchorMax=new Vector2(.5f,.5f);
            _ghost.Root.pivot=new Vector2(.5f,.5f);_ghost.Root.anchoredPosition=local+new Vector2(22,-22);
        }
        public void DropOn(FloatingItemTarget target)
        {
            var p=Dragged;if(p==null)return;
            // Read once more at release. Indices, counts, character and session can change mid-drag.
            _host.RefreshNow();
            bool valid=Dragged==p&&_host.DragStillValid(p)&&target.Accepts?.Invoke(p)==true;
            Cancel();if(valid)target.Drop?.Invoke(p);
        }
        public void SourceDisabled(FloatingItemPointer source)
        {Unhover(source);if(source==_source)Cancel();}
        public void Refreshed()
        {
            _hoverDirty=true;
            if(Dragged==null)return;
            if(!_host.DragStillValid(Dragged)){Cancel();return;}
            RefreshTargets();
        }
        void RefreshTargets()
        {foreach(var t in _targets)if(t!=null)t.Paint(Dragged!=null&&t.gameObject.activeInHierarchy? t.Accepts?.Invoke(Dragged)==true:(bool?)null);}
        public void Cancel()
        {
            Dragged=null;_source=null;_hover=null;
            if(Tooltip!=null)Tooltip.Hide();
            if(_ghost!=null)_ghost.Root.gameObject.SetActive(false);
            foreach(var t in _targets)if(t!=null)t.Paint(null);
        }
        public void Scroll(PointerEventData e,FloatingItemPointer source)
        {
            if(_hover==source&&Tooltip.Visible&&Tooltip.CanScroll){Tooltip.Scroll(e.scrollDelta.y);e.Use();return;}
            var scroll=source.GetComponentInParent<ScrollRect>();if(scroll!=null)scroll.OnScroll(e);
        }
        public void ContextClick(FloatingItemPointer source,PointerEventData e)
        {
            if(e.button!=PointerEventData.InputButton.Right||!_host.ItemsAvailable)return;
            var p=source.Payload?.Invoke();
            if(p?.Item==null||!_host.DragStillValid(p))return;
            _host.ContextClick(p);e.Use();
        }
        void Update()
        {
            if(!_host.ItemsAvailable){if(Dragged!=null||_hover!=null)Cancel();return;}
            if(Dragged!=null)
            {
                if(!Application.isFocused||Keyboard.current?.escapeKey.wasPressedThisFrame==true){Cancel();return;}
                if(Mouse.current!=null)Move(Mouse.current.position.ReadValue());
                return;
            }
            if(_hover==null||!_hover.gameObject.activeInHierarchy||Time.unscaledTime<_hoverAt)return;
            if(_hoverDirty){_hoverText=_hover.Description?.Invoke();_hoverItem=_hover.Preview?.Invoke();_hoverDirty=false;}
            if(Mouse.current!=null)Tooltip.Show(_hoverText,Mouse.current.position.ReadValue(),_hoverItem,_host.Model.IsGem(_hoverItem));
        }
        void OnDisable()=>Cancel();
    }

    public sealed class FloatingItemPointer : MonoBehaviour,IPointerEnterHandler,IPointerExitHandler,
        IBeginDragHandler,IDragHandler,IEndDragHandler,IScrollHandler,IPointerClickHandler
    {
        public FloatingItemInteraction Controller;
        public Func<FloatingItemInteraction.Payload> Payload;
        public Func<string> Description;
        public Func<ItemView> Preview;
        public void OnPointerEnter(PointerEventData e)=>Controller.Hover(this);
        public void OnPointerExit(PointerEventData e)=>Controller.Unhover(this);
        public void OnBeginDrag(PointerEventData e){e.eligibleForClick=false;Controller.Begin(this,e);}
        public void OnDrag(PointerEventData e){e.eligibleForClick=false;Controller.Move(e.position);}
        public void OnEndDrag(PointerEventData e){e.eligibleForClick=false;Controller.Cancel();}
        public void OnScroll(PointerEventData e)=>Controller.Scroll(e,this);
        public void OnPointerClick(PointerEventData e)=>Controller.ContextClick(this,e);
        void OnDisable(){if(Controller!=null)Controller.SourceDisabled(this);}
    }
    public sealed class FloatingItemTarget : MonoBehaviour,IDropHandler
    {
        public string Key;
        public Func<FloatingItemInteraction.Payload,bool> Accepts;
        public Action<FloatingItemInteraction.Payload> Drop;
        FloatingItemInteraction _controller;
        Image _ring;
        public bool? Highlight { get; private set; }
        public void Build(FloatingItemInteraction controller)
        {
            _controller=controller;_ring=UiTheme.Border((RectTransform)transform,Color.clear,4);
            _ring.name="DropOutline";_ring.raycastTarget=false;_ring.enabled=false;
        }
        public void Paint(bool? allowed)
        {Highlight=allowed;_ring.enabled=allowed.HasValue;if(allowed.HasValue)_ring.color=allowed.Value?FloatingItemInteraction.Allowed:FloatingItemInteraction.Denied;if(allowed.HasValue)_ring.transform.SetAsLastSibling();}
        public void OnDrop(PointerEventData e)=>_controller.DropOn(this);
        void OnDisable(){if(_ring!=null)Paint(null);}
    }
}
