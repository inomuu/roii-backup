Shader "ROI/SwordEdge"
{
    Properties
    {
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("Emission Gain", Range(0, 8)) = 6
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
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
                float _Opacity;
                float _EmissionGain;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float4 color : COLOR; };
            struct V { float4 positionCS : SV_POSITION; float4 color : COLOR; };
            V vert(A i)
            {
                V o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.color = i.color;
                return o;
            }
            half4 frag(V i) : SV_Target { return half4(i.color.rgb * _EmissionGain, i.color.a * _Opacity); }
            ENDHLSL
        }
    }
}
