// The wet-road sheen for the FULL-3D ground, as an additive overlay over the terrain. The
// terrain itself is TerrainLit, and a layer's smoothness alone showed nothing from the table's
// 42-degree camera (a high sun reflects into the sky, not the lens — the Godot doc's finding, and
// the frame confirmed it). So the glint is EXPLICIT, the way ground_recolour.gdshader and our own
// GroundArt.shader build it: the region art masks the road by low saturation, a rippled normal
// scrolled through time tilts the water, and a Blinn lobe plus a grazing fresnel band paint the
// sun back onto it. This file is that block lifted out of GroundArt (same names, same maths) and
// made a pass of its own, minus the albedo darkening the terrain layer already applies.
//
// Additive, no depth write, drawn right after the opaques: the sprites and the blades still sort
// over it, the terrain shows through, and being pre-tonemap the bloom blooms it like Godot's glow.
Shader "ROI/WetSheen"
{
    Properties
    {
        _MainTex ("Region art", 2D) = "white" {}
        _WetAmount ("Wet", Range(0,1)) = 0.6
        _WetRough ("Wet Roughness", Range(0.02,1)) = 0.18
        _WetSat ("Wet Saturation Cutoff", Range(0.02,0.6)) = 0.35
        _WetGain ("Wet Gain", Range(0,3)) = 1
        _WetRipple ("Ripple", Range(0,1)) = 0.5
        _WetRippleScale ("Ripple Scale", Float) = 0.7
        _SunPitch ("Sun Pitch", Float) = -34
        _SunYaw ("Sun Yaw", Float) = -38
        _SunCol ("Sun Colour", Color) = (1,0.97,0.91,1)
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Geometry+5" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        ZTest LEqual
        Cull Off
        Pass
        {
            Name "Sheen"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST, _SunCol;
                float _WetAmount, _WetRough, _WetSat, _WetGain, _WetRipple, _WetRippleScale, _SunPitch, _SunYaw;
            CBUFFER_END

            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; float3 positionWS : TEXCOORD1; };

            static const half3 LUMA = half3(0.2126, 0.7152, 0.0722);

            // cloud_shadow.gdshaderinc's value noise — the ripple uses the same one in Godot.
            float cloudHash(float2 p)
            {
                p = frac(p * float2(123.34, 345.45));
                p += dot(p, p + 34.345);
                return frac(p.x * p.y);
            }
            float cloudNoise(float2 p)
            {
                float2 i = floor(p), f = frac(p);
                f = f * f * (3.0 - 2.0 * f);
                float a = cloudHash(i), b = cloudHash(i + float2(1, 0)), c = cloudHash(i + float2(0, 1)), d = cloudHash(i + float2(1, 1));
                return lerp(lerp(a, b, f.x), lerp(c, d, f.x), f.y);
            }

            V vert(A i)
            {
                V o;
                o.positionWS = TransformObjectToWorld(i.positionOS.xyz);
                o.positionCS = TransformWorldToHClip(o.positionWS);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                return o;
            }

            half4 frag(V i) : SV_Target
            {
                half3 c = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;
                half mx = max(c.r, max(c.g, c.b)), mn = min(c.r, min(c.g, c.b));
                half sat = mx > 0.0001 ? (mx - mn) / mx : 0.0;
                half lum = dot(c, LUMA);
                // ASCENDING edges only (Metal returns 0 for a descending smoothstep).
                half mask = (1.0 - smoothstep(_WetSat * 0.4, _WetSat, sat))
                          * smoothstep(0.10, 0.30, lum) * _WetAmount;
                if (mask <= 0.001) return 0;

                // Ripple normal: two offset noise taps give a gradient, scrolled through time.
                // Zero ripple = mirror-flat water, one clean pool instead of sparkle (battery mode).
                float3 wn = float3(0, 0, 1);
                if (_WetRipple > 0.0)
                {
                    float2 wp = i.positionWS.xz * _WetRippleScale;
                    float t = _Time.y * 0.13;
                    float e = 0.2;
                    float n0 = cloudNoise(wp + float2(t, t * 0.3));
                    float nx = cloudNoise(wp + float2(e, 0) + float2(t, t * 0.3));
                    float nz = cloudNoise(wp + float2(0, e) + float2(t, t * 0.3));
                    wn = normalize(float3((n0 - nx) * _WetRipple * 8.0, (n0 - nz) * _WetRipple * 8.0, 1.0));
                }
                float3 nw = normalize(float3(wn.x, wn.z, wn.y));
                float pr = radians(_SunPitch), yr = radians(_SunYaw);
                float3 toSun = -float3(-cos(pr) * sin(yr), sin(pr), -cos(pr) * cos(yr));
                toSun.x = -toSun.x;   // this world mirrors x against Godot's (see GroundArt)
                float3 view = normalize(GetCameraPositionWS() - i.positionWS);
                float3 h = normalize(toSun + view);
                float shine = clamp(0.5 / max(_WetRough * _WetRough, 0.0004), 6.0, 90.0);
                half spec = pow(max(dot(nw, h), 0.0), shine);
                half fres = pow(1.0 - max(dot(nw, view), 0.0), 2.0);   // grazing: broad wet band
                half3 emission = _SunCol.rgb * ((spec * 1.2 + fres * 1.6) * mask * _WetGain);
                return half4(emission, 1);
            }
            ENDHLSL
        }
    }
}
