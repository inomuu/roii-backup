"""Manual native-plugin smoke check and isolated Unity preview fixture.

Run after tools/unity_dylib.sh. Writes only /tmp/potential-pair-preview.json;
never load this deliberately boosted fixture as an online/account save.
"""
import ctypes as c,json,pathlib
root=pathlib.Path(__file__).resolve().parents[1]
l=c.CDLL(str(root/'unity/ROI/Assets/Plugins/macOS/libsim_ffi.dylib'))
for n,args,ret in [('roi_new_as',[c.c_uint64,c.c_char_p,c.c_char_p,c.c_char_p,c.c_char_p],c.c_void_p),('roi_load_json',[c.c_char_p,c.c_char_p],c.c_void_p),('roi_query_json',[c.c_void_p,c.c_char_p],c.c_void_p),('roi_save_json',[c.c_void_p],c.c_void_p),('roi_apply_input',[c.c_void_p,c.c_char_p],c.c_int),('roi_tick',[c.c_void_p,c.c_uint64],None),('roi_free',[c.c_void_p],None),('roi_string_free',[c.c_void_p],None)]:
 f=getattr(l,n); f.argtypes=args;f.restype=ret
cat=(root/'unity/ROI/Assets/StreamingAssets/game_data.json').read_bytes()
def read(n,*args):
 p=getattr(l,n)(*args); assert p,n
 try:return json.loads(c.string_at(p))
 finally:l.roi_string_free(p)
def new(name):
 h=l.roi_new_as(42,cat,b'zone_b1_1',name.encode(),b'Potential QA');assert h;return h
def load(s):
 h=l.roi_load_json(json.dumps(s).encode(),cat); assert h; return h
def board(h,i):return read('roi_query_json',h,('potential:'+str(i)).encode())
def send(h,n,**fields):assert l.roi_apply_input(h,json.dumps({n:fields}).encode())==1
a=new('seraphine');b=new('lapis')
s=read('roi_save_json',a);r=read('roi_save_json',b);l.roi_free(a);l.roi_free(b)
p=next(e for e in s['grid']['entities'] if e['team']=='Player');q=next(e for e in r['grid']['entities'] if e['team']=='Player')
q['id']=2;q['pos']['x']+=1
s['grid']['entities'].append(q)
for e in (p,q):e['stats']['skill_points']=50;e['stats']['job_level']=50
pathlib.Path('/tmp/potential-pair-preview.json').write_text(json.dumps(s))
h=load(s)
for i,dest in [(1,'A'),(2,'B')]:
 v=board(h,i); assert v['supported'] and len(v['nodes'])==18
 other_before=board(h,3-i)
 node=next(n for n in v['nodes'] if n['id']==dest); cost=node['max_skill_point_cost'];assert cost>0
 send(h,'MaxPotential',entity_id=i,target_id=dest)
 after=board(h,i);assert after['skill_points_owned']==50-cost
 assert next(n for n in after['nodes'] if n['id']==dest)['rank']==1
 assert board(h,3-i)==other_before
 saved=read('roi_save_json',h);l.roi_free(h);h=load(saved)
 assert board(h,i)==after
 send(h,'ResetPotential',entity_id=i)
 assert board(h,i)['skill_points_owned']==50
 assert board(h,i)['reset_refund_points']==0
 assert board(h,3-i)==other_before
 print(i,v['unit_def'],'route cost',cost,'save/load + independent refund PASS')
# An unauthored node and insufficient budget must not change the save.
before=read('roi_save_json',h)
send(h,'UpgradePotential',entity_id=2,node_id='C')
assert read('roi_save_json',h)==before
l.roi_free(h)
print('Pair checks PASS; preview fixture has explicit test points and never writes the live save.')
# Combat fixture: one hit from a Job-level threshold, using a real catalog monster reward.
a=new('seraphine');s=read('roi_save_json',a);l.roi_free(a)
p=next(e for e in s['grid']['entities'] if e['team']=='Player')
m=next(e for e in s['grid']['entities'] if e['team']=='Monster')
p['stats']['job_xp']=102 # Live Job-1 cost is 103; no points are injected.
p['atk']=100000;p['atk_range']=99;p['atk_acc']=1000
m['hp']=1;m['pos']={'x':p['pos']['x']+1,'y':p['pos']['y']}
s['grid']['entities']=[p,m]
s['zone_spawn']=None
h=load(s);before=board(h,1)['skill_points_owned']
l.roi_tick(h,100)
after=read('roi_save_json',h)
p=next(e for e in after['grid']['entities'] if e['team']=='Player')
assert p['stats']['job_level']>1, p['stats']
assert board(h,1)['skill_points_owned']-before==p['stats']['job_level']-1
print('Combat earned Job Level',p['stats']['job_level'],'points',board(h,1)['skill_points_owned'],'PASS')
l.roi_free(h)
