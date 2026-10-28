using System;
using System.IO;
using Roi;
using Roi.Arena;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;

// A scheduled demonstration hit, not a collision or damage decision.
public sealed class SeraphineImpactPreview : IDisposable
{
    public const float HitTime=.55f;
    const float RedFlashStrength=.65f;
    readonly Texture2D _texture;
    readonly Sprite _sprite;
    readonly GameObject _dummy;
    readonly SwordHitImpact _impact;
    readonly SpriteRenderer _body;
    MaterialPropertyBlock _flashProperties;
    readonly Camera _camera;
    readonly bool _redFlash;
    readonly Renderer[] _outlines;
    readonly Vector3 _contact,_tangent,_targetPosition;

    public SeraphineImpactPreview(Transform actor,SpriteRenderer body,Camera camera,float height,
        Material actorMaterial,ArenaView.Tables tables,bool redFlash=false)
    {
        _camera=camera;
        _redFlash=redFlash;
        var config=tables.GeomFor("mob_b10_a");
        _texture=new Texture2D(2,2,TextureFormat.RGBA32,false);
        _texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath,"art",tables.SheetFor("mob_b10_a",null))));
        _texture.filterMode=FilterMode.Point;
        int frame=config.idle_frames[0],fw=config.fw;
        var rect=new Rect(frame%config.cols*fw,_texture.height-(frame/config.cols+1)*fw,fw,fw);
        var px=_texture.GetPixels((int)rect.x,(int)rect.y,fw,fw);
        int bottom=fw,top=0,left=fw,right=0;
        for (int y=0;y<fw;y++) for (int x=0;x<fw;x++) if (px[y*fw+x].a>.05f)
        { bottom=Mathf.Min(bottom,y); top=Mathf.Max(top,y+1); left=Mathf.Min(left,x); right=Mathf.Max(right,x+1); }
        Require(top>bottom,"target sprite is empty");
        _sprite=Sprite.Create(_texture,rect,new Vector2((left+right)*.5f/fw,bottom/(float)fw),fw);
        _dummy=new GameObject("Stationary impact target"); _dummy.transform.SetParent(actor,false);
        var yaw=Quaternion.Euler(0,body.transform.eulerAngles.y,0);
        _dummy.transform.position=actor.position+yaw*Vector3.right*(height*1.35f);
        var renderer=new GameObject("Target body").AddComponent<SpriteRenderer>();
        _body=renderer;
        renderer.transform.SetParent(_dummy.transform,false); renderer.transform.rotation=camera.transform.rotation;
        renderer.sharedMaterial=actorMaterial; renderer.sprite=_sprite;
        renderer.flipX=!config.faces_left;
        renderer.transform.localScale=Vector3.one*(height*.95f*fw/(top-bottom));
        ActorContactShadow.Create(_dummy.transform).Bind(height*.75f,0,1);
        // The clockwise screen-space sweep reaches the target side at strike phase 0.5.
        _contact=renderer.bounds.center; _tangent=yaw*Vector3.forward;
        _targetPosition=_dummy.transform.position;
        _impact=SwordHitImpact.Create(actor,height);
        Require(_impact!=null,"impact materials missing");
        _outlines=Array.FindAll(_impact.GetComponentsInChildren<Renderer>(),r=>r.name.EndsWith(" outline"));
        Sample(0);
    }

    public void Sample(float time,bool visible=true,bool flashVisible=true)
    {
        float age=time-HitTime;
        _impact.Sample(age,_contact,_tangent,_camera,visible);
        if (_redFlash) foreach (var r in _outlines) r.enabled=false;
        ApplyFlash(flashVisible && age>=0 ? ArenaView.FlashDur-age : 0);
        Require(_dummy.transform.position.Equals(_targetPosition),"impact moved its target");
    }

    void ApplyFlash(float remaining)
    {
        ActorHitFlash.Apply(_body,ref _flashProperties,remaining);
        if (!_redFlash) return;
        // Preview-only colour study; the live monster flash is unchanged.
        _flashProperties.SetColor("_FlashColor",new Color(1.3f,.42f,.36f,1));
        _flashProperties.SetFloat("_Flash",_flashProperties.GetFloat("_Flash")*RedFlashStrength);
        _body.SetPropertyBlock(_flashProperties);
    }

    public void Check(Texture2D pixels,Action<string> read,string name,int isolatedLayer)
    {
        var renderers=Array.FindAll(_impact.GetComponentsInChildren<Renderer>(true),
            r=>!_redFlash || !r.name.EndsWith(" outline"));
        var layers=new int[renderers.Length];
        var data=_camera.GetUniversalAdditionalCameraData(); bool post=data.renderPostProcessing;
        int culling=_camera.cullingMask; var clear=_camera.clearFlags; var background=_camera.backgroundColor;
        try
        {
            for (int i=0;i<renderers.Length;i++)
            { layers[i]=renderers[i].gameObject.layer; renderers[i].gameObject.layer=isolatedLayer; }
            _camera.cullingMask=1<<isolatedLayer; _camera.clearFlags=CameraClearFlags.SolidColor;
            _camera.backgroundColor=new Color(.025f,.03f,.04f); data.renderPostProcessing=false;
            CheckFlash(pixels,read,name,isolatedLayer);
            Sample(0); read("impact-empty-"+name); var empty=pixels.GetPixels32();
            Sample(HitTime+.045f);
            foreach (var r in renderers) r.enabled=false;
            foreach (var r in renderers)
            {
                r.enabled=true; read(r.name.Replace(' ','-')+"-"+name);
                long energy=Energy(empty,pixels.GetPixels32());
                Require(energy>100,"impact layer invisible: "+r.name);
                Require(!ShaderUtil.ShaderHasError(r.sharedMaterial.shader),"impact shader compile error");
                r.enabled=false; read("impact-layer-hidden-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())>100,"hidden layer"),"layer guard survived hidden renderer");
                Debug.Log("SWORD IMPACT LAYER: "+name+" "+r.name+" energy="+energy);
            }
            Sample(HitTime+.045f); read("impact-early-"+name); var original=pixels.GetPixels32();
            long early=Energy(empty,original);
            _impact.Sample(.045f,_contact,Quaternion.AngleAxis(90,_camera.transform.forward)*_tangent,_camera);
            read("impact-turned-"+name);
            Require(Energy(original,pixels.GetPixels32())>1000,"impact ignored swing direction");
            Sample(HitTime+.045f); read("impact-direction-sabotage-"+name);
            Reject(()=>Require(Energy(original,pixels.GetPixels32())>1000,"frozen direction"),"direction guard survived frozen input");
            var ps=_impact.GetComponentInChildren<ParticleSystem>();
            var particles=new ParticleSystem.Particle[10]; int count=ps.GetParticles(particles);
            Require(count>0,"impact emitted no particles");
            float forward=0;
            for (int i=0;i<count;i++) forward+=particles[i].velocity.x;
            Require(forward/count>.01f,"impact sparks did not travel along the strike");
            var shape=ps.shape; var rotation=shape.rotation;
            try
            {
                shape.rotation=new Vector3(0,-90,0);
                Sample(HitTime+.045f);
                int reversedCount=ps.GetParticles(particles); float reversedForward=0;
                for (int i=0;i<reversedCount;i++) reversedForward+=particles[i].velocity.x;
                Reject(()=>Require(reversedCount>0 && reversedForward/reversedCount>.01f,"reversed sparks"),
                    "spark direction guard survived reversed emission");
            }
            finally { shape.rotation=rotation; }
            Sample(HitTime+.14f); read("impact-middle-"+name); long middle=Energy(empty,pixels.GetPixels32());
            Sample(HitTime+.25f); read("impact-late-"+name); long late=Energy(empty,pixels.GetPixels32());
            Require(middle<early*.8 && late<middle*.5,"impact tail did not fade");
            Sample(HitTime+.045f); read("impact-fade-sabotage-"+name);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())<early*.8,"frozen age"),"fade guard survived frozen age");
            Sample(HitTime+.045f,false); read("impact-cancelled-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"cancelled impact left pixels");
            Sample(HitTime+.045f); read("impact-restarted-"+name);
            Require(Energy(empty,pixels.GetPixels32())>early*.9,"restarted impact stayed faded");
            Sample(HitTime+SwordHitImpact.Duration+.01f); read("impact-finished-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"expired impact left pixels");
            foreach (var r in renderers) Require(!r.enabled,"expired impact left a renderer enabled");
            Require(ps.particleCount==0,"expired impact left particles");
            Debug.Log("SWORD IMPACT: "+name+" fade energy="+early+","+middle+","+late
                +" particles="+count+" mean forward velocity="+(forward/count)
                +"; layer/direction/fade sabotage caught; cancellation, restart and completion passed");
        }
        finally
        {
            _impact.Hide();
            ApplyFlash(0);
            for (int i=0;i<renderers.Length;i++) renderers[i].gameObject.layer=layers[i];
            _camera.cullingMask=culling; _camera.clearFlags=clear; _camera.backgroundColor=background;
            data.renderPostProcessing=post;
        }
        CheckHaloWithFlash(pixels,read,name);
    }
    void CheckHaloWithFlash(Texture2D pixels,Action<string> read,string name)
    {
        var halo=Array.Find(_impact.GetComponentsInChildren<Renderer>(),r=>r.name=="Impact blue halo");
        try
        {
            Sample(HitTime+.033f);
            halo.enabled=false; read("flash-without-halo-"+name); var baseline=pixels.GetPixels32();
            halo.enabled=true; read("flash-with-halo-"+name);
            int blue=BlueContribution(baseline,pixels.GetPixels32());
            Require(blue>100,"blue impact halo unreadable against hit flash");
            halo.enabled=false; read("flash-halo-sabotage-"+name);
            Reject(()=>Require(BlueContribution(baseline,pixels.GetPixels32())>100,"hidden halo"),
                "blue contrast guard survived hidden halo");
            Debug.Log("SWORD IMPACT HALO: "+name+" blue pixels with flash="+blue+"; hidden-halo sabotage caught");
        }
        finally { Sample(0,false); }
    }
    static int BlueContribution(Color32[] baseline,Color32[] pixels)
    {
        int count=0;
        for (int i=0;i<pixels.Length;i++)
        {
            var p=pixels[i]; var b=baseline[i];
            int change=Math.Abs(p.r-b.r)+Math.Abs(p.g-b.g)+Math.Abs(p.b-b.b);
            if (p.b>p.r+30 && p.b>p.g+10 && change>45) count++;
        }
        return count;
    }
    public void CheckOutline(Texture2D pixels,Action<string> read,string name)
    {
        var outlines=Array.FindAll(_impact.GetComponentsInChildren<Renderer>(),r=>r.name.EndsWith(" outline"));
        var enabled=Array.ConvertAll(outlines,r=>r.enabled);
        try
        {
            foreach (var r in outlines) r.enabled=false;
            read("without-outline-"+name); var baseline=pixels.GetPixels32();
            for (int i=0;i<outlines.Length;i++) outlines[i].enabled=enabled[i];
            read("with-outline-"+name);
            int blue=BlueContribution(baseline,pixels.GetPixels32());
            Require(blue>30,"impact outline lost against full slash and flash");
            foreach (var r in outlines) r.enabled=false;
            read("hidden-outline-"+name);
            Reject(()=>Require(BlueContribution(baseline,pixels.GetPixels32())>30,"hidden outline"),
                "outline contrast guard survived hidden outline");
            Debug.Log("SWORD IMPACT OUTLINE: "+name+" blue pixels in full effect="+blue+"; hidden-outline sabotage caught");
        }
        finally { for (int i=0;i<outlines.Length;i++) outlines[i].enabled=enabled[i]; }
    }
    void CheckFlash(Texture2D pixels,Action<string> read,string name,int layer)
    {
        int oldLayer=_body.gameObject.layer;
        try
        {
            _body.gameObject.layer=layer;
            Sample(0,false); read("flash-base-"+name); var baseline=pixels.GetPixels32();
            Sample(HitTime+.045f,false); read("flash-early-"+name);
            long early=Energy(baseline,pixels.GetPixels32());
            Require(early>1000,"target flash invisible");
            if (_redFlash)
            {
                int red=RedContribution(baseline,pixels.GetPixels32());
                Require(red>200,"red target flash invisible");
                _flashProperties.SetColor("_FlashColor",new Color(ArenaView.FlashTint,ArenaView.FlashTint,ArenaView.FlashTint,1));
                _body.SetPropertyBlock(_flashProperties); read("flash-white-sabotage-"+name);
                Reject(()=>Require(RedContribution(baseline,pixels.GetPixels32())>200,"white instead of red"),
                    "red flash guard survived white tint");
                Debug.Log("SWORD IMPACT RED: "+name+" red pixels="+red+"; white-tint sabotage caught");
                Sample(HitTime+.045f,false);
            }
            Sample(HitTime+.045f,false,false); read("flash-disabled-"+name);
            Reject(()=>Require(Energy(baseline,pixels.GetPixels32())>1000,"disabled flash"),
                "flash guard survived disabled flash");
            Sample(HitTime+ArenaView.FlashDur*.5f,false);
            _body.GetPropertyBlock(_flashProperties);
            float halfFlash=.0625f*(_redFlash ? RedFlashStrength : 1);
            Require(Mathf.Abs(_flashProperties.GetFloat("_Flash")-halfFlash)<.00001f,"flash curve changed");
            Sample(HitTime+.045f,false);
            _body.GetPropertyBlock(_flashProperties);
            Reject(()=>Require(Mathf.Abs(_flashProperties.GetFloat("_Flash")-halfFlash)<.00001f,"frozen flash curve"),
                "flash curve guard survived frozen age");
            Sample(HitTime+.23f,false); read("flash-middle-"+name);
            long middle=Energy(baseline,pixels.GetPixels32());
            Require(middle>0 && middle<early*.8,"target flash did not fade");
            Sample(HitTime+.045f,false); read("flash-frozen-"+name);
            Reject(()=>Require(Energy(baseline,pixels.GetPixels32())<early*.8,"frozen flash"),
                "flash fade guard survived frozen age");
            Sample(HitTime+ArenaView.FlashDur+.01f,false); read("flash-finished-"+name);
            Require(Energy(baseline,pixels.GetPixels32())==0,"target flash did not restore sprite");
            _body.GetPropertyBlock(_flashProperties);
            Require(_flashProperties.GetFloat("_Flash")==0,"expired flash property not reset");
            Debug.Log("SWORD IMPACT FLASH: "+name+" energy="+early+","+middle
                +"; disabled/frozen sabotage caught; sprite restored");
        }
        finally
        {
            _body.gameObject.layer=oldLayer;
            Sample(0,false);
        }
    }
    static long Energy(Color32[] a,Color32[] b)
    {
        long sum=0;
        for (int i=0;i<a.Length;i++) sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
        return sum;
    }
    static int RedContribution(Color32[] baseline,Color32[] pixels)
    {
        int count=0;
        for (int i=0;i<pixels.Length;i++)
        {
            var p=pixels[i]; var b=baseline[i];
            if (p.r>p.g+45 && p.r>p.b+45 && p.r-b.r>30) count++;
        }
        return count;
    }
    static void Reject(Action guard,string message)
    {
        bool caught=false;
        try { guard(); } catch (InvalidOperationException) { caught=true; }
        Require(caught,message);
    }
    static void Require(bool condition,string message)
    { if (!condition) throw new InvalidOperationException("SWORD IMPACT: "+message); }
    public void Dispose()
    {
        if (_impact!=null) Object.DestroyImmediate(_impact.gameObject);
        if (_dummy!=null) Object.DestroyImmediate(_dummy);
        Object.DestroyImmediate(_sprite); Object.DestroyImmediate(_texture);
    }
}
