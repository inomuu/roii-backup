Shader "ROI/SwordStarSpark"
{
    Properties
    {
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("White Core Emission", Float) = 4.5
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+5" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off
            Cull Off
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                float _Opacity, _EmissionGain;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float4 color : COLOR; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float4 color : COLOR; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz);
                o.color=i.color; o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                float2 p=sqrt(abs(i.uv*2-1));
                float star=p.x+p.y;
                float aa=max(fwidth(star),.015);
                float mask=1-smoothstep(1-aa,1+aa,star);
                float core=1-smoothstep(.75,.96,star);
                half3 color=lerp(i.color.rgb*1.5,half3(1,1,1)*_EmissionGain,core);
                return half4(color,mask*i.color.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
