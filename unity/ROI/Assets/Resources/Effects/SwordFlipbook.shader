Shader "ROI/SwordFlipbook"
{
    Properties
    {
        _Atlas ("4x4 atlas", 2D) = "black" {}
        _Frame ("Frame", Range(0, 15)) = 0
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("Emission", Float) = 3
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend One One
            ZWrite Off
            Cull Off
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_Atlas); SAMPLER(sampler_Atlas);
            CBUFFER_START(UnityPerMaterial)
                float4 _Atlas_TexelSize;
                float _Frame, _Opacity, _EmissionGain;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                float frame=clamp(floor(_Frame),0,15);
                float2 cell=float2(fmod(frame,4),3-floor(frame/4));
                float2 inset=_Atlas_TexelSize.xy*2;
                float2 uv=(cell+clamp(i.uv,inset,1-inset))/4;
                half4 tex=SAMPLE_TEXTURE2D(_Atlas,sampler_Atlas,uv);
                // Protect cells from the faint grid artifacts in the unretouched generated atlas.
                float2 edge=min(i.uv,1-i.uv);
                float guard=smoothstep(.025,.06,min(edge.x,edge.y));
                return half4(tex.rgb*tex.a*_EmissionGain*_Opacity*guard,0);
            }
            ENDHLSL
        }
    }
}
