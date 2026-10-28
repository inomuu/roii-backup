// THE AUDIO LAYER, ported from godot/views/sound_fx.gd. A pool of one-shot players, a key -> clip
// cache, and one looping music player. Presentation only: the deterministic sim is never gated on
// a sound, and nothing here can change a tick.
//
// LOADED FROM StreamingAssets, as the art sheets are, and for the same reason — a file there is
// bytes the client opens, with no import settings between the source and what ships, so the two
// engines cannot end up playing differently processed audio. It also keeps the authored copies in
// `godot/assets/` the single source (`tools/unity_data.sh` syncs them).
//
// NO AudioMixer. A mixer is Unity's native answer and would normally win under the
// native-before-porting rule, but it buys nothing here: Godot's two buses exist to hold two
// volumes, this holds the same two as scalars, and a .mixer asset is an Editor-authored binary
// that no probe can build or verify. If ducking or an effect chain is ever wanted, that is when a
// mixer earns its place.
//
// dB, not linear, at the call sites — the Godot ones are all written in dB and copying the numbers
// unchanged is the point of a port.
using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Networking;

namespace Roi.Audio
{
    public sealed class SoundFx : MonoBehaviour
    {
        const int PoolSize = 12;

        static SoundFx _instance;
        public static SoundFx Instance => _instance;

        readonly List<AudioSource> _pool = new();
        readonly Dictionary<string, AudioClip> _clips = new();
        readonly Dictionary<string, float> _gain = new();
        AudioSource _bgm;
        string _bgmKey = "";
        /// Set BEFORE the clips finish loading. A track asked for during the load is remembered and
        /// started when its file arrives — otherwise the zone's music is silently skipped, because
        /// the only call that would start it is the one made on the frame the zone opened.
        string _pendingBgm = "";
        float _pendingBgmDb;

        public static SoundFx Create(Transform parent)
        {
            if (_instance != null) return _instance;
            var go = new GameObject("SoundFx");
            go.transform.SetParent(parent, false);
            _instance = go.AddComponent<SoundFx>();
            _instance.Build();
            return _instance;
        }

        void Build()
        {
            for (int i = 0; i < PoolSize; i++)
            {
                var s = gameObject.AddComponent<AudioSource>();
                s.playOnAwake = false;
                s.spatialBlend = 0f;      // 2D: the camera is fixed and the arena is one screen
                _pool.Add(s);
            }
            EnsureListener();
            _bgm = gameObject.AddComponent<AudioSource>();
            _bgm.playOnAwake = false;
            _bgm.loop = true;
            _bgm.spatialBlend = 0f;
            ApplyVolumes();
        }

        void OnDestroy() { if (_instance == this) _instance = null; }

        /// WITHOUT ONE, NOTHING IS AUDIBLE — and nothing says so except a console warning that
        /// scrolls past. Every source still reports `isPlaying`, so a probe that checks the voices
        /// passes while the game is silent; that is exactly what happened here.
        ///
        /// It goes on the CAMERA when there is one, which is where Unity expects it, and on this
        /// object otherwise so a scene with no camera still makes noise.
        static void EnsureListener()
        {
            if (Object.FindFirstObjectByType<AudioListener>() != null) return;
            var cam = Camera.main;
            var host = cam != null ? cam.gameObject : Instance != null ? Instance.gameObject : null;
            if (host != null) host.AddComponent<AudioListener>();
        }

        // --- volumes ---------------------------------------------------------------------------

        /// 0..1 each, the two the options screen shows. Kept as scalars rather than bus levels:
        /// there are exactly two, and the sources multiply them straight in.
        public static float MusicVolume { get; private set; } = 0.7f;
        public static float SfxVolume { get; private set; } = 0.8f;

        public static void SetVolumes(float music, float sfx)
        {
            MusicVolume = Mathf.Clamp01(music);
            SfxVolume = Mathf.Clamp01(sfx);
            if (_instance != null) _instance.ApplyVolumes();
        }

        void ApplyVolumes()
        {
            if (_bgm != null) _bgm.volume = Db(_bgmDb) * MusicVolume;
        }

        float _bgmDb = -8f;

        /// dB -> the linear gain Unity wants. The Godot call sites are all in dB and stay that way.
        static float Db(float db) => Mathf.Pow(10f, db / 20f);

        // --- registration ----------------------------------------------------------------------

        /// Register `key` from a file in StreamingAssets/audio. `gainDb` is a per-sound trim baked
        /// in at load, so a quiet or loud source is normalised without touching every call site.
        public void Register(string key, string file, float gainDb = 0f)
        {
            _gain[key] = gainDb;
            StartCoroutine(LoadClip(key, file));
        }

        IEnumerator LoadClip(string key, string file)
        {
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "audio", file);
            if (!System.IO.File.Exists(path))
            {
                Debug.LogWarning($"SOUND: {file} is not in StreamingAssets/audio — '{key}' stays silent");
                yield break;
            }
            // UnityWebRequest even for a local file: it is the only decoder path for a compressed
            // clip at runtime. `System.IO` gives bytes, and there is no AudioClip.LoadMP3.
            using var req = UnityWebRequestMultimedia.GetAudioClip("file://" + path, AudioType.MPEG);
            yield return req.SendWebRequest();
            if (req.result != UnityWebRequest.Result.Success)
            {
                Debug.LogWarning($"SOUND: {file} did not decode ({req.error})");
                yield break;
            }
            var clip = DownloadHandlerAudioClip.GetContent(req);
            if (clip == null) yield break;
            clip.name = key;
            _clips[key] = clip;
            if (_pendingBgm == key) { _pendingBgm = ""; PlayBgm(key, _pendingBgmDb); }
        }

        public bool Has(string key) => _clips.ContainsKey(key);

        // --- playing ---------------------------------------------------------------------------

        /// One-shot on the next free pool voice, round-robin. `pitch` varies the tone — call with a
        /// small random spread so repeated hits do not sound identical. No-op for an unknown key.
        public void Play(string key, float volumeDb = 0f, float pitch = 1f)
        {
            if (!_clips.TryGetValue(key, out var clip) || clip == null || _pool.Count == 0) return;
            var s = _pool[_next];
            _next = (_next + 1) % _pool.Count;
            s.clip = clip;
            s.volume = Db(volumeDb + (_gain.TryGetValue(key, out var g) ? g : 0f)) * SfxVolume;
            s.pitch = pitch;
            s.Play();
        }

        int _next;

        /// Loop `key` as music, switching only when it CHANGES — so calling this every frame with
        /// the same track is a no-op.
        public void PlayBgm(string key, float volumeDb = -8f)
        {
            if (key == _bgmKey && _bgm != null && _bgm.isPlaying) return;
            _bgmKey = key;
            _bgmDb = volumeDb;
            if (_bgm == null) return;
            if (!_clips.TryGetValue(key, out var clip) || clip == null)
            {
                _bgm.Stop();
                // Remember it: music is asked for once, on the frame the zone opens, and the file
                // may still be decoding then.
                if (!string.IsNullOrEmpty(key)) { _pendingBgm = key; _pendingBgmDb = volumeDb; }
                return;
            }
            _bgm.clip = clip;
            ApplyVolumes();
            _bgm.Play();
        }

        /// Fire `key` after `delay` seconds. Used to nudge a cast-start sound forward so it lands
        /// near the hit without waiting for the resolve event.
        public void PlayDelayed(string key, float delay, float volumeDb = 0f, float pitch = 1f)
        {
            if (delay <= 0f) { Play(key, volumeDb, pitch); return; }
            StartCoroutine(Later(key, delay, volumeDb, pitch));
        }

        IEnumerator Later(string key, float delay, float volumeDb, float pitch)
        {
            yield return new WaitForSeconds(delay);
            Play(key, volumeDb, pitch);
        }
    }

    /// The TABLES, copied from views/render_data.gd. Which sound a thing makes is a data decision
    /// and it belongs beside the other render data, not scattered through the code that fires it.
    public static class SoundTable
    {
        /// key -> file in StreamingAssets/audio.
        public static readonly Dictionary<string, string> Paths = new()
        {
            { "slime_hit", "slime_hit.mp3" },
            { "dagger_hit", "dagger_hit.mp3" },
            { "bow_shot", "bow_shot.mp3" },
            { "arrow_hit", "arrow_hit.mp3" },
            { "bash_hit", "bash_hit.mp3" },
        };

        /// Per-sound dB trim baked in at load. Absent = 0 dB.
        public static readonly Dictionary<string, float> Gain = new()
        {
            { "dagger_hit", -6f },   // rattle-hit — trimmed down (net ~-16 dB on play)
        };

        /// Skill id -> the sound its impact makes.
        public static readonly Dictionary<string, string> SkillHit = new()
        {
            { "bash", "bash_hit" },
        };

        /// Monster HIT sound by RENDER BUCKET, not by monster id: every slime-rendered monster
        /// shares one sound, which is the same grouping the sprites use.
        public static readonly Dictionary<string, string> MonsterHit = new()
        {
            { "slime", "slime_hit" },
        };

        public static readonly Dictionary<string, string> Bgm = new()
        {
            { "map1", "bgm_map1.mp3" },
        };

        /// Band -> track. Bands with no entry play nothing rather than repeating band 1's.
        public static readonly Dictionary<int, string> BandBgm = new()
        {
            { 1, "map1" },
        };
    }
}
