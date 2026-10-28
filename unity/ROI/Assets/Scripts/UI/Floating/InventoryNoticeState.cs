using System;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    // Session-local read receipts. Compare owned counts, including equipped items, so an
    // unequip or reorder is not mistaken for loot. Legacy items have no unique instance ID.
    internal sealed class InventoryNoticeState
    {
        SimSession _session;
        Dictionary<string,int> _owned = new();
        readonly Dictionary<string,int> _unread = new();
        ItemView[] _bag = Array.Empty<ItemView>();
        public int Capacity { get; private set; }
        static string Key(ItemView i) => !string.IsNullOrEmpty(i.instance_id) ? i.instance_id
            : i.def_id+"|"+(i.grade??"Common")+"|"+i.star+"|"+i.foil;
        static int Count(Dictionary<string,int> map,string key) => map.TryGetValue(key,out var n)?n:0;
        public void Observe(SimSession session, ItemView[] bag, string view)
        {
            if(session==null||!session.IsAlive)return;
            bool baseline=!ReferenceEquals(_session,session);
            if(baseline){_session=session;_owned.Clear();_unread.Clear();}
            var root=JObject.Parse(view);
            Capacity=(int?)root["inventory_cap"]??0;
            var next=new Dictionary<string,int>();
            foreach(var item in bag) {var k=Key(item);next[k]=Count(next,k)+1;}
            foreach(var token in root.SelectTokens("$..equipped[*]"))
            {
                var item=JsonUtility.FromJson<ItemView>(token.ToString());
                if(string.IsNullOrEmpty(item?.def_id))continue;
                var k=Key(item);next[k]=Count(next,k)+1;
            }
            if(!baseline)foreach(var pair in next)
            {
                int gained=Math.Max(0,pair.Value-Count(_owned,pair.Key));
                _unread[pair.Key]=Math.Min(pair.Value,Count(_unread,pair.Key)+gained);
            }
            foreach(var key in new List<string>(_unread.Keys))
                if(!next.ContainsKey(key))_unread.Remove(key);
            _owned=next;_bag=bag;
        }
        public bool IsNew(int index)
        {
            if(index<0||index>=_bag.Length)return false;
            var key=Key(_bag[index]);int later=0;
            for(int i=index+1;i<_bag.Length;i++)if(Key(_bag[i])==key)later++;
            return Count(_unread,key)>later;
        }
        public void Read(int index)
        {
            if(!IsNew(index))return;
            var key=Key(_bag[index]);_unread[key]=Math.Max(0,Count(_unread,key)-1);
        }
    }
}
