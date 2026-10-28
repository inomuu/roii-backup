using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Only the selected dossier animates. Union cropping keeps every frame at one scale/anchor.
    public sealed class MonsterPortraitIdle : MonoBehaviour
    {
        Image _image;
        Sprite[] _frames;
        readonly List<Sprite> _owned=new();
        int _frame;
        float _interval;
        public int FrameChanges { get; private set; }
        public void Initialize(Image image,Sprite[] source,int fps)
        {
            _image=image;_interval=1f/Mathf.Clamp(fps,1,12);
            if(source==null||source.Length==0)return;
            var valid=new List<Sprite>();foreach(var s in source)if(s!=null)valid.Add(s);
            if(valid.Count==0)return;
            int minX=int.MaxValue,minY=int.MaxValue,maxX=-1,maxY=-1;
            // Read only the small frame regions, once per selected creature; never in the loop.
            foreach(var s in valid)
            {
                var r=s.rect;if(!s.texture.isReadable)continue;
                var pixels=s.texture.GetPixels((int)r.x,(int)r.y,(int)r.width,(int)r.height);
                for(int y=0;y<(int)r.height;y++)for(int x=0;x<(int)r.width;x++)
                    if(pixels[y*(int)r.width+x].a>.08f){minX=Mathf.Min(minX,x);minY=Mathf.Min(minY,y);maxX=Mathf.Max(maxX,x);maxY=Mathf.Max(maxY,y);}
            }
            if(maxX>=minX&&maxY>=minY)
            {
                foreach(var s in valid)
                {
                    var r=s.rect;
                    var cropped=Sprite.Create(s.texture,new Rect(r.x+minX,r.y+minY,maxX-minX+1,maxY-minY+1),new Vector2(.5f,0),s.pixelsPerUnit,0,SpriteMeshType.FullRect);
                    cropped.name=s.name+"_Dossier";_owned.Add(cropped);
                }
                _frames=_owned.ToArray();
            }
            else _frames=valid.ToArray();
            // Bottom alignment keeps the sprite on the fixed plinth, including wide creatures.
            var bounds=_frames[0].rect;var rect=_image.rectTransform;var area=rect.sizeDelta;
            float scale=Mathf.Min(area.x/bounds.width,area.y/bounds.height);
            var size=new Vector2(bounds.width*scale,bounds.height*scale);
            rect.anchoredPosition+=new Vector2((area.x-size.x)*.5f,-(area.y-size.y));rect.sizeDelta=size;
            _image.sprite=_frames[0];
            if(_frames.Length>1)StartCoroutine(Animate());
        }
        IEnumerator Animate()
        {
            var tick=new WaitForSecondsRealtime(_interval);var idle=new WaitForSecondsRealtime(.5f);
            while(true)
            {
                // No frame swaps, transforms, mesh rebuilds or texture work while motion is off.
                if(!UiEntrance.MotionAllowed||PlayerPrefs.GetInt("roi.opt.postfx",1)==0)
                {yield return idle;continue;}
                yield return tick;
                if(!UiEntrance.MotionAllowed||PlayerPrefs.GetInt("roi.opt.postfx",1)==0)continue;
                if(_image.canvasRenderer.cull)continue;
                _frame=(_frame+1)%_frames.Length;_image.sprite=_frames[_frame];FrameChanges++;
            }
        }
        void OnDestroy(){foreach(var s in _owned)if(s!=null)Destroy(s);}
    }
}
