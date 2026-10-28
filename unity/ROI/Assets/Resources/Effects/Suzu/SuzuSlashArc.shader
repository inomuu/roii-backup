Shader "ROI/SuzuSlashArc"
{
    Properties
    {
        _MaskTex ("Silhouette", 2D) = "black" {}
        _FlowTex ("Erosion flow", 2D) = "white" {}
        _MaskRect ("Mask UV rectangle", Vector) = (0,0,1,1)
        _InnerTint ("Inner tint", Vector) = (1,1,1,1)
        _OuterTint ("Outer tint", Vector) = (1,1,1,1)
        _EmissionGain ("Emission", Float) = 1
        _Phase ("UV travel", Float) = 0
        _Erosion ("Erosion", Float) = 0
        _Opacity ("Opacity", Float) = 0
        _Width ("Mask width", Float) = 1
        _Soft ("Soft wake", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha One
            ZWrite Off
            Cull Back
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_MaskTex); SAMPLER(sampler_MaskTex);
            TEXTURE2D(_FlowTex); SAMPLER(sampler_FlowTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MaskRect, _InnerTint, _OuterTint;
                float _EmissionGain, _Phase, _Erosion, _Opacity, _Width, _Soft;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                // Both authored faces retain their mirrored U. No vertex displacement or mesh rotation.
                float u=frac(i.uv.x-_Phase+.38);
                float v=(i.uv.y-.5)/max(.01,_Width)+.5;
                float2 maskUV=_MaskRect.xy+float2(u,saturate(v))*_MaskRect.zw;
                float mask=SAMPLE_TEXTURE2D(_MaskTex,sampler_MaskTex,maskUV).r;
                float aa=max(fwidth(mask),.015);
                float coverage=lerp(smoothstep(.5-aa,.5+aa,mask),mask,_Soft);
                coverage*=step(0,v)*step(v,1);
                // Hide the square carrier ends and the repeat seam, not the painted flame tips.
                coverage*=smoothstep(0,.025,min(i.uv.x,1-i.uv.x));
                coverage*=smoothstep(0,.035,min(u,1-u));
                float flow=SAMPLE_TEXTURE2D(_FlowTex,sampler_FlowTex,float2(u*1.7-_Phase*.12,v)).r;
                float field=.12+u*.73+flow*.15;
                coverage*=smoothstep(_Erosion-.025,_Erosion+.025,field);
                float4 tint=lerp(_InnerTint,_OuterTint,saturate(v));
                return half4(tint.rgb*_EmissionGain,coverage*tint.a*_Opacity);
            }
            ENDHLSL
        }
    }
}
