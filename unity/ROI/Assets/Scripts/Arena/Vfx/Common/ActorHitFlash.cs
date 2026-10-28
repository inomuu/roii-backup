using UnityEngine;

namespace Roi.Arena
{
    // Shared by live combat and editor captures; the caller owns the hit clock.
    public static class ActorHitFlash
    {
        public static void Apply(SpriteRenderer body,ref MaterialPropertyBlock properties,float remaining)
        {
            if (body==null) return;
            properties ??= new MaterialPropertyBlock();
            body.GetPropertyBlock(properties);
            float k=Mathf.Clamp01(remaining/ArenaView.FlashDur);
            properties.SetFloat("_Flash",Mathf.Pow(k,ArenaView.FlashDecay));
            float tint=ArenaView.FlashTint;
            properties.SetColor("_FlashColor",new Color(tint,tint,tint,1));
            body.SetPropertyBlock(properties);
        }
    }
}
