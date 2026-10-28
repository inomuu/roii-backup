// BinbunGrass's `grass.gdshader`, ported. The pack's own vegetation shader gets the shape right
// and the BEHAVIOUR wrong for this game: one push position for the whole world where Godot gives
// every clump its own, a bend where Godot shivers, and a gradient that ignores the root.
//
// What is reproduced, and why each part is the way it is (the reasons are the Godot file's):
//   WIND     noise scrolled through world space, NOT a sine. Two sine waves read as diagonal
//            bands marching across the field; noise gives each patch its own irregular sway.
//            Weighted by height^2 so the base stays planted and only the tips travel.
//   PUSH     grass SHIVERS where something stands in it rather than leaning aside — a steady lean
//            is what a shove looks like, walking through wobbles. Amplitude falls with distance
//            SQUARED so blades under the foot move hard and blades an arm's length away barely
//            notice, which is what makes it read as local disturbance instead of the whole clump
//            reacting. Phase comes from world position so neighbours are out of step; in step it
//            reads as one object shaking, and two unrelated rates keep it from resolving into a
//            beat. Several centres, not one: the party walks as a group.
//   ROOT     the base darkens toward root_shade AND re-saturates. Darkening alone went
//            muddy-grey; pushing saturation back up as it darkens keeps it a deep, rich green.
Shader "ROI/GrassBlade"
{
    Properties
    {
        _MainTex ("Blade", 2D) = "white" {}
        _TopColor ("Tip Colour", Color) = (0.53,0.67,0.27,1)
        _BottomColor ("Root Colour", Color) = (0.44,0.55,0.24,1)
        _Stop0 ("Gradient Stop 0", Color) = (0.53,0.67,0.27,1)
        _Stop1 ("Gradient Stop 1", Color) = (0.23,0.47,0.20,1)
        _Stop2 ("Gradient Stop 2", Color) = (0.44,0.55,0.24,1)
        _Off0 ("Offset 0", Float) = 0.167
        _Off1 ("Offset 1", Float) = 0.5
        _Off2 ("Offset 2", Float) = 0.833
        _RootShade ("Root Shade", Range(0,1)) = 0.28
        _RootSat ("Root Saturation", Range(0,2)) = 1.4
        _BladeHeight ("Blade Height", Float) = 1
        _WindDir ("Wind Direction", Vector) = (1.2,0.36,0,0)
        _WindSpeed ("Wind Speed", Float) = 3
        _WindScale ("Wind Scale", Float) = 0.15
        _PushStrength ("Push Strength", Range(0,6)) = 0.7
        _Cutoff ("Alpha Cut Start", Range(0,1)) = 0.4
        _CutoffEnd ("Alpha Cut End", Range(0,1)) = 0.6
        _Exposure ("Blade Exposure", Range(0.5,2)) = 1
    }
    SubShader
    {
        Tags { "RenderType"="TransparentCutout" "Queue"="AlphaTest" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_instancing
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE
            #pragma multi_compile_fragment _ _SHADOWS_SOFT
            #pragma multi_compile_fragment _ _LIGHT_COOKIES
            #pragma multi_compile _ _ADDITIONAL_LIGHTS_VERTEX _ADDITIONAL_LIGHTS
            #pragma multi_compile _ _FORWARD_PLUS
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "Packages/com.unity.render-pipelines.core/ShaderLibrary/Color.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST, _TopColor, _BottomColor, _WindDir, _Stop0, _Stop1, _Stop2;
                float _Off0, _Off1, _Off2;
                float _RootShade, _RootSat, _BladeHeight, _WindSpeed, _WindScale, _PushStrength, _Cutoff, _CutoffEnd, _Exposure;
            CBUFFER_END

            // xz = walker, y = energy, w = radius — the same packing Godot's per-instance `push`
            // uses, except there is a list of them here instead of one.
            #define MAX_PUSH 8
            float4 _PushPoints[MAX_PUSH];
            int _PushCount;

            float hash21b(float2 p)
            {
                p = frac(p * float2(123.34, 456.21));
                p += dot(p, p + 45.32);
                return frac(p.x * p.y);
            }

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
            struct A { float4 positionOS : POSITION; float3 normalOS : NORMAL; float2 uv : TEXCOORD0;
                       UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; float3 positionWS : TEXCOORD1;
                       float3 normalWS : TEXCOORD2; float height : TEXCOORD3; float4 screenPos : TEXCOORD4; };

            #include "GrassBladeCommon.hlsl"

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 wp = TransformObjectToWorld(i.positionOS.xyz);
                float h;
                wp = GrassDisplace(wp, i.positionOS.xyz, h);
                o.positionWS = wp;
                o.positionCS = TransformWorldToHClip(wp);
                // Camera-facing, as a Godot billboard blade is: N.L against this sun is 0.85
                // facing the lens against 0.56 facing up, and pinning it up left every tuft at
                // half the brightness of the Godot frame it was measured against.
                // The camera's DIRECTION, one normal for the whole field — not the vector to the
                // camera's position. Godot's billboard aligns every quad to the camera's
                // orientation, so every blade meets the sun at the same angle; pointing each at
                // the lens instead tilts the left of the field away from the sun and the right
                // toward it, and the blades measured 120 on the left against 186 on the right.
                // Row 2 of the view matrix is already the camera's BACKWARD axis in world space
                // (view space looks down -Z), so it is the normal as-is. Negating it — the first
                // attempt — turned every blade to face away from the lens and the field went black.
                o.normalWS = normalize(UNITY_MATRIX_V[2].xyz);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                o.height = h;
                o.screenPos = ComputeScreenPos(o.positionCS);
                return o;
            }

            static const half3 GRASS_LUMA = half3(0.299, 0.587, 0.114);

            half4 frag(V i) : SV_Target
            {
                half4 tex = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv);
                // The blade's SHAPE lives in the R channel, not in alpha — BinbunGrass samples
                // `.r` off its atlas. Clipping on alpha instead passes the whole quad, which is
                // why every blade rendered as a solid rectangle.
                // Back to the space the number was authored in. The mask is imported as sRGB, so
                // the sampler hands back LINEAR values — Godot's 0.4 cutoff then bites at roughly
                // 0.13 and takes the whole blade with it. Convert, then compare.
                GrassClip(tex, i.positionCS.xy);

                // The gradient is picked per BLADE by noise, not by height — that is what gives a
                // field its patchiness instead of one flat colour with a lighter top. Godot samples
                // its colour gradient at a noise value keyed to world position; two stops here, so
                // the noise chooses between them.
                // 0.4, not 0.1: Godot samples its noise TEXTURE at world*0.1, and that texture
                // carries ~4 cycles across its width, so a colour patch is ~2.5 world units. At
                // 0.1 with an analytic noise a patch was 10 units, and a whole corner of the field
                // could sit in one dark pick.
                half v = vnoise(i.positionWS.xz * 0.4);
                // Godot's gradient has THREE stops (light, dark, mid) at authored offsets and is
                // sampled at the noise. Two stops gave every patch either the light or the dark
                // end; the mid stop is what most of a field wears, and what makes the clumps
                // read as different from one another rather than as one of two colours.
                half3 albedo = v < _Off1
                    ? lerp(_Stop0.rgb, _Stop1.rgb, saturate((v - _Off0) / max(_Off1 - _Off0, 1e-3)))
                    : lerp(_Stop1.rgb, _Stop2.rgb, saturate((v - _Off1) / max(_Off2 - _Off1, 1e-3)));
                // THEN the root: darken AND re-saturate, blended in toward the base. Darkening
                // alone goes muddy-grey; re-saturating as it darkens keeps it a deep rich green.
                half3 root = albedo * _RootShade;
                half rl = dot(root, GRASS_LUMA);
                root = saturate(lerp(rl.xxx, root, _RootSat));
                albedo = lerp(root, albedo, i.height);
                // The texture supplies SHADING, not colour. Godot's blade atlas is a mask and its
                // gradient carries everything; this pack's texture is a painted bush, so its hue
                // would tint an already-tinted blade — but its light and shade is the only thing
                // giving each blade form. Take the luminance, centred on 1, and keep the colour.
                // The mask is ALPHA ONLY, as Godot uses it. A shading term off the same mask was
                // added for the pack's bush mesh and stayed; on these blades it took the body to
                // two thirds of the Godot frame's brightness, measured on isolated blade pixels.

                // CALIBRATED, and the only number in this shader that is. With the layout, the
                // gradient, the billboard normal and the lights all Godot's, the blades still came
                // out darker than the Godot frame — measured on blade pixels isolated by a
                // with/without pair: tips x0.79, body x0.65, roots x0.82. Whatever the engines
                // disagree on underneath is not in this file, so the shortfall is closed here,
                // named, next to the evidence.
                albedo *= _Exposure;
                float4 sc = TransformWorldToShadowCoord(i.positionWS);
                Light L = GetMainLight(sc);
                #if defined(_LIGHT_COOKIES)
                L.color *= SampleMainLightCookie(i.positionWS);   // the cloud shadow, see ActorSprite
                #endif
                half3 lit = albedo * (SampleSH(i.normalWS) + L.color * saturate(dot(i.normalWS, L.direction)));

                lit += GodotFill(i.positionWS, i.normalWS, albedo);
                return half4(lit, 1);
            }
            ENDHLSL
        }

        // See GroundArt's DepthOnly pass: with MSAA, URP builds `_CameraDepthTexture` from a
        // prepass that draws this pass only, so a shader without one is absent from depth and
        // depth of field blurs it at full radius. Wind, push and the alpha cut come from
        // GrassBladeCommon.hlsl so the depth silhouette cannot drift from the drawn one.
        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ZWrite On
            ColorMask R
            HLSLPROGRAM
            #pragma vertex vertD
            #pragma fragment fragD
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.core/ShaderLibrary/Color.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST, _TopColor, _BottomColor, _WindDir, _Stop0, _Stop1, _Stop2;
                float _Off0, _Off1, _Off2;
                float _RootShade, _RootSat, _BladeHeight, _WindSpeed, _WindScale, _PushStrength, _Cutoff, _CutoffEnd, _Exposure;
            CBUFFER_END

            #define MAX_PUSH 8
            float4 _PushPoints[MAX_PUSH];
            int _PushCount;

            #include "GrassBladeCommon.hlsl"

            struct AD { float4 positionOS : POSITION; float2 uv : TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct VD { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };

            VD vertD(AD i)
            {
                VD o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 wp = TransformObjectToWorld(i.positionOS.xyz);
                float h;
                wp = GrassDisplace(wp, i.positionOS.xyz, h);
                o.positionCS = TransformWorldToHClip(wp);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                return o;
            }

            half fragD(VD i) : SV_Target
            {
                GrassClip(SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv), i.positionCS.xy);
                return 0;
            }
            ENDHLSL
        }

        // See GroundArt's DepthNormals pass: with SSAO on, this prepass is what fills the camera
        // depth texture. The normal is the lit pass's billboard normal, not the mesh's.
        Pass
        {
            Name "DepthNormals"
            Tags { "LightMode"="DepthNormals" }
            ZWrite On
            HLSLPROGRAM
            #pragma vertex vertN
            #pragma fragment fragN
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.core/ShaderLibrary/Color.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST, _TopColor, _BottomColor, _WindDir, _Stop0, _Stop1, _Stop2;
                float _Off0, _Off1, _Off2;
                float _RootShade, _RootSat, _BladeHeight, _WindSpeed, _WindScale, _PushStrength, _Cutoff, _CutoffEnd, _Exposure;
            CBUFFER_END

            #define MAX_PUSH 8
            float4 _PushPoints[MAX_PUSH];
            int _PushCount;

            #include "GrassBladeCommon.hlsl"

            struct AN { float4 positionOS : POSITION; float2 uv : TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct VN { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };

            VN vertN(AN i)
            {
                VN o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 wp = TransformObjectToWorld(i.positionOS.xyz);
                float h;
                wp = GrassDisplace(wp, i.positionOS.xyz, h);
                o.positionCS = TransformWorldToHClip(wp);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                return o;
            }

            half4 fragN(VN i) : SV_Target
            {
                GrassClip(SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv), i.positionCS.xy);
                return half4(normalize(UNITY_MATRIX_V[2].xyz), 0);
            }
            ENDHLSL
        }
    }
    Fallback Off
}
