// The ground, as Godot's `ground_recolour.gdshader` does it. A terrain layer alone renders the
// art flat; what makes that floor read is two things the Godot shader adds on top, and region 1
// has both switched on:
//
//   SHADE CRUSH  multiplies DARK pixels darker and leaves bright ones alone — a shadows-only
//                contrast curve applied to the ART before lighting, so the scene's own grade
//                stays free to do its job. Ground only, deliberately: characters have to stay
//                readable at every hour.
//   WET SHEEN    masked on how GREY a pixel is (low saturation = cobble, not grass) with a
//                luminance floor so the near-black joints are not all puddle. Wet pixels get a
//                darker, richer albedo and an explicit sun glint. The GRAZING term is the one
//                that reads from a pitched camera — the sun barely reflects down off flat
//                ground, but the surface brightens hard where it faces away from the lens.
//
// The ripple normal and the baked normal/occlusion maps are not ported: LookSettings has them at
// zero for this region, and porting a switched-off feature is how untested code arrives.
Shader "ROI/GroundArt"
{
    Properties
    {
        _MainTex ("Ground", 2D) = "white" {}
        _ShadeCrush ("Shade Crush", Range(0,1)) = 0.4
        _ShadeCeiling ("Shade Ceiling", Range(0.1,1)) = 0.6
        _WetAmount ("Wet", Range(0,1)) = 0.6
        _WetRough ("Wet Roughness", Range(0.02,1)) = 0.18
        _WetSat ("Wet Saturation Cutoff", Range(0.02,0.6)) = 0.35
        _WetDarken ("Wet Darken", Range(0,0.6)) = 0.22
        _WetGain ("Wet Gain", Range(0,3)) = 1
        _SunPitch ("Sun Pitch", Float) = -34
        _SunYaw ("Sun Yaw", Float) = -38
        _SunCol ("Sun Colour", Color) = (1,0.97,0.91,1)
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE
            #pragma multi_compile_fragment _ _SHADOWS_SOFT
            #pragma multi_compile_fragment _ _LIGHT_COOKIES
            #pragma multi_compile _ _ADDITIONAL_LIGHTS_VERTEX _ADDITIONAL_LIGHTS
            // The ground pool is GodotFill below, not the engine's point light (which is culled
            // off this surface) — see that function for why. Forward+ keyword kept so the main
            // light's shadow data resolves the same way as the rest of the scene.
            #pragma multi_compile _ _FORWARD_PLUS
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST, _SunCol;
                float _ShadeCrush, _ShadeCeiling, _WetAmount, _WetRough, _WetSat, _WetDarken,
                      _WetGain, _SunPitch, _SunYaw;
            CBUFFER_END

            // The ground pool with GODOT'S falloff, `1 - d/range`, in place of the engine's point
            // light for this surface. URP's light is inverse-square: matched at the pool's centre
            // it leaves the far corners almost unlit (the bottom-left blades measured 15 from the
            // pool where the centre got 64), and no calibration at one point can fix a curve.
            float4 _FillPos;      // xyz = world, w = range
            float4 _FillColor;    // colour x energy, already tinted for the hour
            half3 GodotFill(float3 wp, float3 n, half3 albedo)
            {
                float3 d = _FillPos.xyz - wp;
                float dist = length(d);
                float att = saturate(1.0 - dist / max(_FillPos.w, 0.01));
                return albedo * _FillColor.rgb * (att * saturate(dot(n, d / max(dist, 1e-4))));
            }
            struct A { float4 positionOS : POSITION; float3 normalOS : NORMAL; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0;
                       float3 positionWS : TEXCOORD1; float3 normalWS : TEXCOORD2;
                       float4 screenPos : TEXCOORD3; };

            V vert(A i)
            {
                V o;
                o.positionWS = TransformObjectToWorld(i.positionOS.xyz);
                o.positionCS = TransformWorldToHClip(o.positionWS);
                o.normalWS = TransformObjectToWorldNormal(i.normalOS);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                o.screenPos = ComputeScreenPos(o.positionCS);
                return o;
            }

            static const half3 LUMA = half3(0.2126, 0.7152, 0.0722);

            half4 frag(V i) : SV_Target
            {
                half3 c = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;

                // Shade crush — smoothstep, so the curve has no knee for the eye to find.
                if (_ShadeCrush > 0.0)
                {
                    half lum = dot(c, LUMA);
                    c *= lerp(1.0 - _ShadeCrush, 1.0, smoothstep(0.0, _ShadeCeiling, lum));
                }

                half3 emission = 0;
                half smoothness = 0;
                if (_WetAmount > 0.0)
                {
                    half mx = max(c.r, max(c.g, c.b)), mn = min(c.r, min(c.g, c.b));
                    half sat = mx > 0.0001 ? (mx - mn) / mx : 0.0;
                    half lum = dot(c, LUMA);
                    // ASCENDING edges only. smoothstep with edge0 > edge1 is undefined and Metal
                    // returns 0 for it, which silently killed this whole mask once already.
                    half mask = (1.0 - smoothstep(_WetSat * 0.4, _WetSat, sat))
                              * smoothstep(0.10, 0.30, lum) * _WetAmount;
                    if (mask > 0.001)
                    {
                        c *= lerp(1.0, 1.0 - _WetDarken, mask);
                        smoothness = lerp(0.0, 1.0 - _WetRough, mask);
                        float pr = radians(_SunPitch), yr = radians(_SunYaw);
                        float3 toSun = -float3(-cos(pr) * sin(yr), sin(pr), -cos(pr) * cos(yr));
                        // This world mirrors x against Godot's, so the glint has to come from the
                        // same side of the field as the shadows do.
                        toSun.x = -toSun.x;
                        float3 nw = float3(0, 1, 0);
                        float3 view = normalize(GetCameraPositionWS() - i.positionWS);
                        float3 h = normalize(toSun + view);
                        float shine = clamp(0.5 / max(_WetRough * _WetRough, 0.0004), 6.0, 90.0);
                        half spec = pow(max(dot(nw, h), 0.0), shine);
                        // Grazing: broad, low exponent — a wet band, not a hairline.
                        half fres = pow(1.0 - max(dot(nw, view), 0.0), 2.0);
                        emission = _SunCol.rgb * ((spec * 1.2 + fres * 1.6) * mask * _WetGain);
                    }
                }

                float4 sc = TransformWorldToShadowCoord(i.positionWS);
                Light L = GetMainLight(sc);
                #if defined(_LIGHT_COOKIES)
                L.color *= SampleMainLightCookie(i.positionWS);   // the cloud shadow, see ActorSprite
                #endif
                half ndl = saturate(dot(i.normalWS, L.direction));
                half3 lit = c * (SampleSH(i.normalWS) + L.color * (ndl * L.shadowAttenuation));
                // Forward+ clusters by screen position, so the loop needs it.
                lit += GodotFill(i.positionWS, i.normalWS, c);
                return half4(lit + emission, 1);
            }
            ENDHLSL
        }

        // WITHOUT THIS PASS THE GROUND IS ABSENT FROM `_CameraDepthTexture`. With MSAA on, URP
        // builds that texture from a DEPTH PREPASS, which draws only `DepthOnly` passes — a
        // shader that has none leaves the cleared far value there, and every depth-reading
        // effect treats this surface as infinitely distant. Depth of field then blurs the whole
        // floor at full radius (user, 2026-09-06: "เบลอผิดปกติ").
        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ZWrite On
            ColorMask R
            HLSLPROGRAM
            #pragma vertex vertD
            #pragma fragment fragD
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            struct AD { float4 positionOS : POSITION; };
            struct VD { float4 positionCS : SV_POSITION; };

            VD vertD(AD i)
            {
                VD o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                return o;
            }

            half fragD(VD i) : SV_Target { return 0; }
            ENDHLSL
        }

        // AND DepthNormals, because the SSAO renderer feature on PC_Renderer runs a DEPTH-NORMALS
        // prepass — that prepass, not DepthOnly, is where `_CameraDepthTexture` comes from while
        // SSAO is enabled. A shader with only DepthOnly is still missing from depth, which is why
        // adding that pass alone did not unblur anything.
        Pass
        {
            Name "DepthNormals"
            Tags { "LightMode"="DepthNormals" }
            ZWrite On
            HLSLPROGRAM
            #pragma vertex vertN
            #pragma fragment fragN
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            struct AN { float4 positionOS : POSITION; float3 normalOS : NORMAL; };
            struct VN { float4 positionCS : SV_POSITION; float3 normalWS : TEXCOORD0; };

            VN vertN(AN i)
            {
                VN o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.normalWS = TransformObjectToWorldNormal(i.normalOS);
                return o;
            }

            half4 fragN(VN i) : SV_Target { return half4(normalize(i.normalWS), 0); }
            ENDHLSL
        }
    }
    Fallback Off
}
