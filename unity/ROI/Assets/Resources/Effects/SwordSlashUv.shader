Shader "ROI/SwordSlashUv"
{
    Properties
    {
        _WidthTex ("Approved width profile", 2D) = "white" {}
        _MaskTex ("Sharp streak mask", 2D) = "black" {}
        _FlowTex ("Flow", 2D) = "white" {}
        _InnerTint ("Inner tint", Vector) = (1,1,1,1)
        _OuterTint ("Outer tint", Vector) = (1,1,1,1)
        _EmissionGain ("Emission", Float) = 1
        _Head ("Head position in turns", Float) = 0
        _Span ("Texture length in turns", Float) = 0.916667
        _Reveal ("Revealed length", Range(0,1)) = 1
        _Phase ("Strike phase", Float) = 0
        _Erosion ("Erosion", Float) = 0
        _Opacity ("Opacity", Range(0,1)) = 1
        _Mode ("Layer: blade, glow, detail", Float) = 0
        [Enum(UnityEngine.Rendering.BlendMode)] _DstBlend ("Destination blend", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha [_DstBlend]
            ZWrite Off
            Cull Off
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_WidthTex); SAMPLER(sampler_WidthTex);
            TEXTURE2D(_MaskTex); SAMPLER(sampler_MaskTex);
            TEXTURE2D(_FlowTex); SAMPLER(sampler_FlowTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _InnerTint, _OuterTint;
                float _EmissionGain, _Head, _Span, _Reveal, _Phase, _Erosion, _Opacity, _Mode, _DstBlend;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                // Periodic distance keeps the travelling mask continuous across the ring's UV seam.
                float u=1-frac(_Head-i.uv.x)/max(_Span,.001);
                float reveal=smoothstep(1-_Reveal,1-_Reveal+max(.001,min(.08,_Reveal*.3)),u);
                float inside=step(0,u);
                float v=i.uv.y,coverage;
                if (_Mode<1.5)
                {
                    float width=SAMPLE_TEXTURE2D_LOD(_WidthTex,sampler_WidthTex,float2(saturate(u),.5),0).r*reveal;
                    float radial=(v*2-1)/max(width,.0001);
                    v=saturate(radial*.5+.5);
                    float distance=abs(i.uv.y*2-1)-width;
                    float aa=max(fwidth(distance),.0005);
                    coverage=(1-smoothstep(-aa,aa,distance))*inside*step(.0001,width);
                    if (_Mode>.5)
                    {
                        float flow=SAMPLE_TEXTURE2D(_FlowTex,sampler_FlowTex,float2(u*2-_Phase*.3,v)).r;
                        coverage*=pow(saturate(1-abs(radial)),1.5)*lerp(.25,1,flow);
                    }
                }
                else
                {
                    float mask=SAMPLE_TEXTURE2D(_MaskTex,sampler_MaskTex,float2(saturate(u),v)).r;
                    float aa=max(fwidth(mask),.01);
                    coverage=smoothstep(.5-aa,.5+aa,mask)*inside*reveal;
                    float flow=SAMPLE_TEXTURE2D(_FlowTex,sampler_FlowTex,float2(u*1.7-_Phase*.12,v)).r;
                    float field=.12+u*.73+flow*.15;
                    float edge=max(fwidth(field),.015);
                    coverage*=smoothstep(_Erosion-edge,_Erosion+edge,field);
                }
                float4 tint=lerp(_InnerTint,_OuterTint,v);
                return half4(tint.rgb*_EmissionGain,coverage*tint.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
