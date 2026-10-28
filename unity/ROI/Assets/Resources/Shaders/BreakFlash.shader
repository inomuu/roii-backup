Shader "ROI/Break Flash"
{
    Properties { [HDR] _Color ("Radiance", Color) = (4.4,4.8,5.2,1) }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent-10" "RenderType"="Transparent" "IgnoreProjector"="True" }
        Pass
        {
            Name "BREAKFlash"
            Blend SrcAlpha One
            ZWrite Off
            ZTest LEqual
            Cull Off
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                float4 _Color;
            CBUFFER_END
            struct Attributes { float4 positionOS : POSITION; float4 color : COLOR; float2 uv : TEXCOORD0; };
            struct Varyings { float4 positionCS : SV_POSITION; float4 color : COLOR; float2 uv : TEXCOORD0; };
            Varyings Vert(Attributes v)
            {
                Varyings o;
                o.positionCS = TransformObjectToHClip(v.positionOS.xyz);
                o.color = v.color;
                o.uv = v.uv;
                return o;
            }
            half4 Frag(Varyings i) : SV_Target
            {
                float2 p = i.uv * 2.0 - 1.0;
                float radius = length(p);
                // Broad overlapping cloud lobes; no texture noise, ring or hard outer disk.
                float warp = .045 * sin(p.x * 9.0 + p.y * 5.0) + .035 * cos(p.y * 11.0 - p.x * 3.0);
                float r = max(0.0, radius + warp * saturate(radius * 2.0));
                float cloud = exp(-r * r * 2.7) * (1.0 - smoothstep(.52, 1.0, r));
                float alpha = saturate(cloud * i.color.a * _Color.a);
                return half4(_Color.rgb * i.color.rgb, alpha);
            }
            ENDHLSL
        }
    }
}
