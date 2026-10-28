Shader "ROI/SwordSlashDetail"
{
    Properties
    {
        _MaskTex ("Sharp streak mask", 2D) = "black" {}
        _FlowTex ("Erosion flow", 2D) = "white" {}
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("Emission", Float) = 3.8
        _Phase ("Strike phase", Float) = 0
        _Erosion ("Tail erosion", Range(0, 1.05)) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+2" "RenderPipeline"="UniversalPipeline" }
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
            TEXTURE2D(_MaskTex); SAMPLER(sampler_MaskTex);
            TEXTURE2D(_FlowTex); SAMPLER(sampler_FlowTex);
            CBUFFER_START(UnityPerMaterial)
                float _Opacity, _EmissionGain, _Phase, _Erosion;
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
                float mask=SAMPLE_TEXTURE2D(_MaskTex,sampler_MaskTex,i.uv).r;
                float aa=max(fwidth(mask),.01);
                float coverage=smoothstep(.5-aa,.5+aa,mask);
                float flow=SAMPLE_TEXTURE2D(_FlowTex,sampler_FlowTex,
                    float2(i.uv.x*1.7-_Phase*.12,i.uv.y)).r;
                // Low U is the trailing edge. Erode it first without moving the blade geometry.
                float field=.12+i.uv.x*.73+flow*.15;
                float edge=max(fwidth(field),.015);
                coverage*=smoothstep(_Erosion-edge,_Erosion+edge,field);
                return half4(i.color.rgb*_EmissionGain,coverage*i.color.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
