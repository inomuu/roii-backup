Shader "ROI/SwordImpactHalo"
{
    Properties
    {
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("Emission", Float) = 5
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+101" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha One
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
                float radius=length(i.uv*2-1);
                // Push colour outside the white hit flash, with a feathered outer edge.
                float mask=smoothstep(.16,.5,radius)*(1-smoothstep(.5,1,radius));
                return half4(i.color.rgb*_EmissionGain,mask*i.color.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
