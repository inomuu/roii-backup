Shader "ROI/SuzuSlashBase"
{
    Properties
    {
        _MaskTex ("Base wedge mask", 2D) = "black" {}
        _Offset ("One-shot U offset", Float) = -1
        _Opacity ("Opacity", Range(0,1)) = 1
        _EmissionGain ("White emission", Float) = 2
        _ArcWindow ("Carrier U range", Vector) = (0,1,0,0)
        _AlignBackU ("Align mirrored underside travel", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off
            Cull Back
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_MaskTex); SAMPLER(sampler_MaskTex);
            CBUFFER_START(UnityPerMaterial)
                float _Offset, _Opacity, _EmissionGain;
                float4 _ArcWindow;
                float _AlignBackU;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; float3 normalOS : NORMAL; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv;
                if (_AlignBackU>.5 && i.normalOS.y<0) o.uv.x=1-o.uv.x;
                return o;
            }
            half4 frag(V i) : SV_Target
            {
                // A single mask crosses the UV window; out-of-range samples never wrap back in.
                float arcU=(i.uv.x-_ArcWindow.x)/max(.001,_ArcWindow.y-_ArcWindow.x);
                float u=arcU-_Offset;
                float inside=step(0,u)*step(u,1)*step(0,arcU)*step(arcU,1);
                float2 uv=float2(.02+.96*saturate(u),.02+.96*i.uv.y);
                float mask=SAMPLE_TEXTURE2D(_MaskTex,sampler_MaskTex,uv).r;
                float aa=max(fwidth(mask),.015);
                float coverage=smoothstep(.5-aa,.5+aa,mask)*inside;
                float endFade=smoothstep(0,.045,u)*(1-smoothstep(.94,1,u));
                return half4(_EmissionGain.xxx,coverage*endFade*_Opacity);
            }
            ENDHLSL
        }
    }
}
