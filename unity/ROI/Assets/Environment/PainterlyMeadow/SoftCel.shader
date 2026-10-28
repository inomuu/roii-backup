Shader "ROI/Painterly/SoftCel"
{
 Properties {
  [MainTexture] _BaseMap("Albedo",2D)="white"{}
  [MainColor] _BaseColor("Colour",Color)=(1,1,1,1)
  _CelBlend("Soft cel blend",Range(0,1))=.6
  _ShadowTint("Shadow colour",Color)=(.55,.72,.76,1)
  _Smoothness("Smoothness",Range(0,1))=.2
  _Cutoff("Cutoff",Range(0,1))=.3
 }
 SubShader {
  Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" }
  Pass {
   Tags { "LightMode"="UniversalForward" }
   HLSLPROGRAM
   #pragma vertex vert
   #pragma fragment frag
   #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
   #pragma multi_compile_fragment _ _SHADOWS_SOFT
   #pragma multi_compile _ _ADDITIONAL_LIGHTS
   #pragma multi_compile_fog
   #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
   #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
   TEXTURE2D(_BaseMap); SAMPLER(sampler_BaseMap);
   CBUFFER_START(UnityPerMaterial)
   float4 _BaseMap_ST, _BaseColor, _ShadowTint;
   float _CelBlend, _Smoothness, _Cutoff;
   CBUFFER_END
   struct A { float4 pos:POSITION; float3 normal:NORMAL; float2 uv:TEXCOORD0; };
   struct V { float4 pos:SV_POSITION; float3 world:TEXCOORD0; float3 normal:TEXCOORD1; float2 uv:TEXCOORD2; float fog:TEXCOORD3; };
   V vert(A a) { V o; o.world=TransformObjectToWorld(a.pos.xyz); o.pos=TransformWorldToHClip(o.world); o.normal=TransformObjectToWorldNormal(a.normal); o.uv=TRANSFORM_TEX(a.uv,_BaseMap); o.fog=ComputeFogFactor(o.pos.z); return o; }
   half4 frag(V i):SV_Target {
    float3 n=normalize(i.normal);
    Light light=GetMainLight(TransformWorldToShadowCoord(i.world));
    float nd=saturate(dot(n,light.direction));
    float bands=.18+.40*smoothstep(.18,.3,nd)+.42*smoothstep(.64,.77,nd);
    float diffuse=lerp(nd,bands,_CelBlend)*light.shadowAttenuation;
    float3 albedo=SAMPLE_TEXTURE2D(_BaseMap,sampler_BaseMap,i.uv).rgb*_BaseColor.rgb;
    albedo=lerp(albedo, floor(albedo*12+.5)/12, .25);
    float3 ambient=max(SampleSH(n),float3(.17,.20,.22));
    float3 c=albedo*(ambient*_ShadowTint.rgb+light.color*diffuse*.85);
    float spec=pow(saturate(dot(n,normalize(light.direction+GetWorldSpaceNormalizeViewDir(i.world)))),32);
    c+=light.color*smoothstep(.45,.7,spec)*_Smoothness*.25*light.shadowAttenuation;
    #if defined(_ADDITIONAL_LIGHTS)
    uint count=GetAdditionalLightsCount();
    for(uint k=0;k<count;k++) {
     Light extra=GetAdditionalLight(k,i.world);
     c+=albedo*extra.color*saturate(dot(n,extra.direction))*extra.distanceAttenuation*extra.shadowAttenuation;
    }
    #endif
    return half4(MixFog(c,i.fog),1);
   }
   ENDHLSL
  }
  UsePass "Universal Render Pipeline/Lit/ShadowCaster"
  UsePass "Universal Render Pipeline/Lit/DepthOnly"
 }
}
