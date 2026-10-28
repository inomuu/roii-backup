Shader "ROI/MeadowAtmosphere"
{
    Properties
    {
        [HDR] _Tint("Light colour", Color) = (1,.83,.5,1)
        _Intensity("Intensity", Float) = .1
        _Ray("Ray instead of mote", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent" "RenderType"="Transparent" }
        Blend SrcAlpha One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
            float4 _Tint;
            float _Intensity, _Ray;
            CBUFFER_END
            struct A { float4 p:POSITION; float2 uv:TEXCOORD0; half4 colour:COLOR; };
            struct V { float4 p:SV_POSITION; float2 uv:TEXCOORD0; half4 colour:COLOR; };
            V vert(A i) { V o; o.p=TransformObjectToHClip(i.p.xyz);o.uv=i.uv;o.colour=i.colour;return o; }
            half4 frag(V i):SV_Target
            {
                float2 d=(i.uv-.5)*2;
                float mote=pow(saturate(1-dot(d,d)),3);
                float ray=pow(saturate(1-abs(d.x)),2)*smoothstep(0,.2,i.uv.y)*(1-smoothstep(.7,1,i.uv.y));
                return half4(_Tint.rgb*i.colour.rgb,lerp(mote,ray,_Ray)*_Intensity*_Tint.a*i.colour.a);
            }
            ENDHLSL
        }
    }
}
