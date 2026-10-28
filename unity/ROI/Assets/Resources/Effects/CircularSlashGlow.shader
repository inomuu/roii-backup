Shader "ROI/CircularSlashGlow"
{
    Properties
    {
        _FlowTex ("Water flow", 2D) = "white" {}
        _Opacity ("Opacity", Range(0, 1)) = 1
        _EmissionGain ("Emission", Float) = 3
        _Phase ("Strike phase", Float) = 0
        _Spark ("Spark mask", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+1" "RenderPipeline"="UniversalPipeline" }
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
            TEXTURE2D(_FlowTex); SAMPLER(sampler_FlowTex);
            CBUFFER_START(UnityPerMaterial)
                float _Opacity, _EmissionGain, _Phase, _Spark;
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
                float crossSection=pow(saturate(1-abs(i.uv.y*2-1)),1.5);
                float flow=SAMPLE_TEXTURE2D(_FlowTex,sampler_FlowTex,float2(i.uv.x*2-_Phase*.3,i.uv.y)).r;
                float wake=crossSection*lerp(.25,1,flow);
                float spark=pow(saturate(1-length(i.uv*2-1)),2);
                float mask=lerp(wake,spark,_Spark);
                return half4(i.color.rgb*_EmissionGain,mask*i.color.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
