using UnityEngine;

namespace Roi.Arena
{
    [CreateAssetMenu(menuName="ROI/VFX/Buff Effect Profile")]
    public sealed class BuffEffectProfile : ScriptableObject
    {
        public GameObject Prefab;
        public Color Tint=Color.white;
        public bool Recolor;
        [Min(.01f)] public float ScalePerHeight=.6f;
        [Min(.1f)] public float Duration=.9f;
        [Min(.01f)] public float PlaybackSpeed=1;
        [Min(0)] public float Warmup;
        public bool GroundGlow;
        [ColorUsage(true,true)] public Color GroundColor=new Color(.08f,.55f,1.3f,1);
        [Min(.1f)] public float GroundDiameter=2;
        public bool RisingAura;
        public Texture2D AuraTexture;
        [ColorUsage(true,true)] public Color AuraColor=new Color(.12f,.75f,1.5f,1);
    }
}
