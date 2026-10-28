using System;
using System.Collections.Generic;
using System.IO;
using Roi.Arena;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Only display artwork belonging to the inspected character; missing art gets a named emblem.
    public sealed class ExemplarPortrait : MonoBehaviour
    {
        RectTransform _fallback;
        TextMeshProUGUI _fallbackName;
        Image _sprite;
        CombatHudPortrait _illustration;
        ExemplarPortraitGround _ground;
        bool _reservePortraitLane, _preserveSourceAspect;
        string _classId;

        public static ExemplarPortrait Create(Transform parent)
        {
            var go = new GameObject("ExemplarPortrait", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            Stretch((RectTransform)go.transform);
            var portrait = go.AddComponent<ExemplarPortrait>();
            portrait.Build();
            return portrait;
        }

        void Build()
        {
            var ground=ExemplarUi.Rect(transform,"PortraitGround",.21f,.06f,.655f,1.02f);
            _ground=ground.gameObject.AddComponent<ExemplarPortraitGround>();_ground.raycastTarget=false;
            ground.gameObject.SetActive(false);
            _fallback=ExemplarUi.Rect(transform,"PortraitUnavailable",.25f,.06f,.595f,.94f);
            ExemplarUi.Icon(_fallback,"Exemplars",.34f,.45f,.66f,.67f,new Color(.65f,.61f,.48f,.38f));
            _fallbackName=ExemplarUi.Text(_fallback,"Name","",.04f,.33f,.96f,.43f,34,ExemplarUi.White,TextAlignmentOptions.Center);
            ExemplarUi.Text(_fallback,"Status",Loc.Get("UI_EXEMPLAR_PORTRAIT_UNAVAILABLE"),.04f,.26f,.96f,.32f,16,ExemplarUi.Muted,TextAlignmentOptions.Center);
            _fallback.gameObject.SetActive(false);

            var go = new GameObject("CharacterSprite", typeof(RectTransform), typeof(Image));
            go.transform.SetParent(transform, false);
            _sprite = go.GetComponent<Image>();
            _sprite.raycastTarget = false;
            _sprite.enabled = false;
            _sprite.preserveAspect = true;
            var rt = _sprite.rectTransform;
            rt.anchorMin = new Vector2(.34f, .04f);
            rt.anchorMax = new Vector2(.86f, .90f);
            rt.offsetMin = rt.offsetMax = Vector2.zero;
            go.SetActive(false);
            var art = new GameObject("CharacterIllustration", typeof(RectTransform));
            art.transform.SetParent(transform, false);
            var ar = (RectTransform)art.transform; Stretch(ar);
            ar.anchorMin = new Vector2(.10f, -.04f); ar.anchorMax = new Vector2(.60f, .96f);
            _illustration = art.AddComponent<CombatHudPortrait>();
            _illustration.Crop = new Rect(.06f, .02f, .86f, .95f);
            _illustration.PreserveAspect = true; _illustration.FeatherAllEdges = false; _illustration.raycastTarget = false;
            art.SetActive(false);
        }

        public void SetFraming(float framing, float anchor, bool reservePortraitLane = false, bool centerManagementPortrait = false, bool fitBetweenPanels = false, bool overview = false)
        {
            _reservePortraitLane=reservePortraitLane;
            _preserveSourceAspect=reservePortraitLane||fitBetweenPanels||overview;
            // All art states share the same reserved lane, including characters without an illustration.
            ExemplarUi.Place(_fallback,reservePortraitLane ? .25f : .10f,reservePortraitLane ? .06f : .04f,
                reservePortraitLane ? .595f : .60f,reservePortraitLane ? .94f : .96f);
            ExemplarUi.Place(_illustration.rectTransform, reservePortraitLane ? .21f : .10f,
                reservePortraitLane ? .06f : -.04f, reservePortraitLane ? .655f : .60f,
                reservePortraitLane ? 1.02f : .96f);
            // Keep the authored boot intact: a broad bottom fade was erasing its contact edge.
            _illustration.Crop=reservePortraitLane?new Rect(.01f,.005f,.98f,.99f):new Rect(.06f,.02f,.86f,.95f);
            _illustration.SideFeather=reservePortraitLane?.012f:.07f;
            _illustration.BottomFeather=reservePortraitLane?.008f:.09f;
            _illustration.SourceAspect=_preserveSourceAspect&&_classId=="seraphine"?5151f/5816f:0;
            _illustration.SetVerticesDirty();UpdateGround();
            ExemplarUi.Place(_sprite.rectTransform, reservePortraitLane ? .25f : .34f,
                reservePortraitLane ? .06f : .04f, reservePortraitLane ? .595f : .86f,
                reservePortraitLane ? .94f : .90f);
            if(centerManagementPortrait)
            {
                // Leave the bottom lane for the shared portrait dock. Centre all art states
                // between the two panels, including characters whose illustration is unavailable.
                ExemplarUi.Place(_illustration.rectTransform,.26f,.14f,.76f,.99f);
                ExemplarUi.Place(_fallback,.28f,.16f,.78f,.96f);
                ExemplarUi.Place(_sprite.rectTransform,.27f,.16f,.79f,.90f);
            }
            if(fitBetweenPanels||overview)
            {
                // Regalia and Resonance panels end at x=.325 and start at x=.763.
                // Attributes uses the space left of its stats panel. Both leave the dock clear.
                float left=overview?.395f:.335f, right=overview?.90f:.752f;
                ExemplarUi.Place(_illustration.rectTransform,left,.155f,right,.965f);
                ExemplarUi.Place(_fallback,left,.155f,right,.965f);
                ExemplarUi.Place(_sprite.rectTransform,left,.155f,right,.965f);
                _illustration.Crop=new Rect(0,0,1,1);
                _illustration.SideFeather=.006f;
                _illustration.BottomFeather=.006f;
                _illustration.SetVerticesDirty();
            }
        }

        public void SetCharacter(string classId)
        {
            if (_classId == classId) return;
            _classId = classId;
            _sprite.sprite = Avatar(classId);
            _sprite.enabled = _sprite.sprite != null;
            var authored = Illustration(classId);
            _illustration.SetTexture(authored); _illustration.gameObject.SetActive(authored != null);
            _illustration.SourceAspect=_preserveSourceAspect&&classId=="seraphine"?5151f/5816f:0;
            _fallbackName.text=GameText.UnitName(classId);
            _fallback.gameObject.SetActive(authored == null && _sprite.sprite == null);
            _sprite.gameObject.SetActive(authored == null && _sprite.sprite != null);
            UpdateGround();
            if(authored != null) UiEntrance.Play(_illustration.rectTransform, 0, .35f, 10);
        }

        void UpdateGround()
        {
            // These contact positions belong to Seraphine's illustration, never to another pose.
            bool visible=_reservePortraitLane&&_classId=="seraphine"&&_illustration.gameObject.activeSelf;
            _ground.gameObject.SetActive(visible);
            if(!visible)return;
            var texture=_illustration.mainTexture;
            _ground.Aspect=(_illustration.SourceAspect>0?_illustration.SourceAspect:(float)texture.width/texture.height)*_illustration.Crop.width/_illustration.Crop.height;
            _ground.SetVerticesDirty();
        }

        public static Texture2D Illustration(string classId)
            => string.IsNullOrEmpty(classId) ? null : Resources.Load<Texture2D>("CombatPortraits/"+char.ToUpperInvariant(classId[0])+classId.Substring(1));

        // Shared first idle frame, including the arena's temporary Seraphine/Rapis aliases.
        // Callers borrow the sprite; the cache owns its lifetime.
        public static Sprite Avatar(string classId)
            => ExemplarPortraitArtCache.Avatar(classId);

        static void Stretch(RectTransform rt)
        {
            rt.anchorMin = Vector2.zero;
            rt.anchorMax = Vector2.one;
            rt.offsetMin = rt.offsetMax = Vector2.zero;
        }
    }

    // Soft, retained contact shading; no animation loop, shader, render texture or extra camera.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExemplarPortraitGround : MaskableGraphic
    {
        public float Aspect=.876f;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var rect=GetPixelAdjustedRect();var center=rect.center;
            if(rect.width>rect.height*Aspect)rect.width=rect.height*Aspect;
            else rect.height=rect.width/Aspect;
            rect.center=center;
            Oval(vh,rect,new Vector2(.76f,.022f),new Vector2(.31f,.045f),new Color(.41f,.48f,.53f,.075f));
            Oval(vh,rect,new Vector2(.83f,.013f),new Vector2(.24f,.037f),new Color(.005f,.009f,.012f,.50f));
            Oval(vh,rect,new Vector2(.90f,.018f),new Vector2(.105f,.012f),new Color(.002f,.004f,.006f,.78f));
        }
        static void Oval(VertexHelper vh,Rect rect,Vector2 at,Vector2 radius,Color ink)
        {
            const int rings=12,segments=64;
            int start=vh.currentVertCount;
            var center=new Vector2(rect.x+at.x*rect.width,rect.y+at.y*rect.height);
            for(int ring=0;ring<=rings;ring++){
                float t=ring/(float)rings;var tint=ink;tint.a*=Mathf.Pow(1-t*t,2);
                for(int step=0;step<=segments;step++){
                    float angle=step*Mathf.PI*2/segments;
                    vh.AddVert(center+new Vector2(Mathf.Cos(angle)*radius.x*rect.width,Mathf.Sin(angle)*radius.y*rect.height)*t,tint,Vector2.zero);
                }
            }
            for(int ring=0;ring<rings;ring++)for(int step=0;step<segments;step++){
                int a=start+ring*(segments+1)+step,b=a+segments+1;
                vh.AddTriangle(a,b,a+1);vh.AddTriangle(a+1,b,b+1);
            }
        }
    }

    // One owner survives screen/scene changes, then destroys the native allocations at shutdown.
    // This also clears the cache when leaving Play Mode with domain reload disabled.
    internal sealed class ExemplarPortraitArtCache : MonoBehaviour
    {
        static ExemplarPortraitArtCache _instance;
        readonly Dictionary<string, Texture2D> _sheets = new();
        readonly Dictionary<string, Sprite> _frames = new();
        readonly Dictionary<string, Sprite> _avatars = new();
        ArenaView.Tables _tables;
        bool _tablesLoaded;

        internal static Sprite Avatar(string classId)
        {
            if (string.IsNullOrEmpty(classId)) return null;
            if (_instance == null)
            {
                var go = new GameObject("ExemplarPortraitArtCache");
                go.hideFlags = HideFlags.HideInHierarchy;
                _instance = go.AddComponent<ExemplarPortraitArtCache>();
                if (Application.isPlaying) DontDestroyOnLoad(go);
            }
            return _instance.Resolve(classId);
        }

        Sprite Resolve(string classId)
        {
            if (_avatars.TryGetValue(classId, out var cached)) return cached;
            if (!_tablesLoaded)
            {
                _tablesLoaded = true;
                try { _tables = ArenaView.Tables.Load(); }
                catch (Exception e) when (e is IOException || e is UnauthorizedAccessException || e is ArgumentException)
                { Debug.LogWarning("Exemplar art table unavailable: " + e.Message); }
            }
            var geometry = _tables?.Class(classId);
            Sprite sprite = null;
            if (geometry != null && geometry.fw > 0 && geometry.fh > 0 && geometry.idle_row >= 0
                && !string.IsNullOrEmpty(geometry.sheet))
            {
                string key = geometry.sheet + "#" + geometry.fw + "x" + geometry.fh + "@" + geometry.idle_row;
                if (!_frames.TryGetValue(key, out sprite))
                {
                    var sheet = Sheet(geometry.sheet);
                    int y = sheet == null ? -1 : sheet.height - (geometry.idle_row + 1) * geometry.fh;
                    if (sheet != null && y >= 0 && geometry.fw <= sheet.width)
                    {
                        sprite = Sprite.Create(sheet, new Rect(0, y, geometry.fw, geometry.fh),
                            new Vector2(.5f, .5f), geometry.fh, 0, SpriteMeshType.FullRect);
                        sprite.name = "ExemplarIdle_" + geometry.id;
                    }
                    _frames[key] = sprite;
                }
            }
            _avatars[classId] = sprite; // Cache misses as well, avoiding repeated file reads.
            return sprite;
        }

        Texture2D Sheet(string file)
        {
            if (_sheets.TryGetValue(file, out var cached)) return cached;
            Texture2D texture = null;
            try
            {
                var path = Path.Combine(Application.streamingAssetsPath, "art", file);
                if (File.Exists(path))
                {
                    texture = new Texture2D(2, 2, TextureFormat.RGBA32, false)
                    {
                        name = "ExemplarSheet_" + file,
                        filterMode = FilterMode.Point,
                        wrapMode = TextureWrapMode.Clamp
                    };
                    if (!texture.LoadImage(File.ReadAllBytes(path), true))
                    { Release(texture); texture = null; }
                }
            }
            catch (Exception e) when (e is IOException || e is UnauthorizedAccessException || e is ArgumentException)
            {
                if (texture != null) Release(texture);
                texture = null;
                Debug.LogWarning("Exemplar sprite unavailable: " + e.Message);
            }
            _sheets[file] = texture;
            return texture;
        }

        void OnDestroy()
        {
            foreach (var sprite in _frames.Values) if (sprite != null) Release(sprite);
            foreach (var texture in _sheets.Values) if (texture != null) Release(texture);
            _frames.Clear();
            _sheets.Clear();
            _avatars.Clear();
            if (_instance == this) _instance = null;
        }

        static void Release(UnityEngine.Object asset)
        {
            if (Application.isPlaying) Destroy(asset);
            else DestroyImmediate(asset);
        }
    }
}
