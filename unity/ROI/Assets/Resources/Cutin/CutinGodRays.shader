// Port of godot/views/cutin/cutin_godrays.gdshader (no URP-native screen-space god ray exists; the
// nature pack's GodRays is a world-space light-shaft sheet). Additive full-frame quad. UV.y is
// flipped once at the top so the maths below reads exactly like the Godot source (y down).
Shader "ROI/CutinGodRays"
{
    Properties
    {
        _Angle ("Angle", Float) = -0.3
        _RayPosition ("Ray Position", Float) = -0.2
        _Spread ("Spread", Range(0, 1)) = 0.55
        _Cutoff ("Cutoff", Range(-1, 1)) = 0.1
        _Falloff ("Falloff", Range(0, 1)) = 0.25
        _EdgeFade ("Edge Fade", Range(0, 1)) = 0.15
        _Speed ("Speed", Float) = 1.0
        _Ray1Density ("Ray1 Density", Float) = 8.0
        _Ray2Density ("Ray2 Density", Float) = 30.0
        _Ray2Intensity ("Ray2 Intensity", Range(0, 1)) = 0.3
        _Color ("Color", Color) = (1.0, 0.9, 0.62, 0.55)
        _Seed ("Seed", Float) = 5.0
        _Modulate ("Modulate", Range(0, 1)) = 1.0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" "IgnoreProjector"="True" }
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
            float _Angle, _RayPosition, _Spread, _Cutoff, _Falloff, _EdgeFade, _Speed;
            float _Ray1Density, _Ray2Density, _Ray2Intensity, _Seed, _Modulate;
            float4 _Color;
            CBUFFER_END

            struct Attributes { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct Varyings { float4 positionHCS : SV_POSITION; float2 uv : TEXCOORD0; };

            Varyings vert(Attributes v)
            {
                Varyings o;
                o.positionHCS = TransformObjectToHClip(v.positionOS.xyz);
                o.uv = v.uv;
                return o;
            }

            float random(float2 p) { return frac(sin(dot(p, float2(12.9898, 78.233))) * 43758.5453123); }

            float noise(float2 uv)
            {
                float2 i = floor(uv);
                float2 f = frac(uv);
                float a = random(i);
                float b = random(i + float2(1.0, 0.0));
                float c = random(i + float2(0.0, 1.0));
                float d = random(i + float2(1.0, 1.0));
                float2 u = f * f * (3.0 - 2.0 * f);
                return lerp(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
            }

            float2 rotate(float a, float2 v)
            {
                // Godot's mat2(vec2(cos,-sin), vec2(sin,cos)) * v, written out.
                return float2(cos(a) * v.x + sin(a) * v.y, -sin(a) * v.x + cos(a) * v.y);
            }

            half4 frag(Varyings i) : SV_Target
            {
                float2 uv = float2(i.uv.x, 1.0 - i.uv.y);
                float2 tuv = rotate(_Angle, uv - _RayPosition) / ((uv.y + _Spread) - (uv.y * _Spread));
                float t = _Time.y;
                float2 ray1 = float2(tuv.x * _Ray1Density + sin(t * 0.1 * _Speed) * (_Ray1Density * 0.2) + _Seed, 1.0);
                float2 ray2 = float2(tuv.x * _Ray2Density + sin(t * 0.2 * _Speed) * (_Ray1Density * 0.2) + _Seed, 1.0);
                float cut = step(_Cutoff, tuv.x) * step(_Cutoff, 1.0 - tuv.x);
                ray1 *= cut;
                ray2 *= cut;
                float rays = saturate(noise(ray1) + noise(ray2) * _Ray2Intensity);
                rays *= smoothstep(0.0, _Falloff, 1.0 - uv.y);
                rays *= smoothstep(_Cutoff, _EdgeFade + _Cutoff, tuv.x);
                rays *= smoothstep(_Cutoff, _EdgeFade + _Cutoff, 1.0 - tuv.x);
                return half4(_Color.rgb * rays, rays * _Color.a * _Modulate);
            }
            ENDHLSL
        }
    }
}
