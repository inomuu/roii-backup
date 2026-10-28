// Lit billboard sprite — the port of proto/arena3d/sprite_flash.gdshader.
//
// Why a shader at all, in either engine: the stock sprite path is UNLIT, so a character stands in
// a scene it is not part of — same reason Godot could not use a plain Sprite3D and wrote its own.
// What is reproduced here, and why each line exists:
//   * lit, not unlit          — `shaded = true`; the sun and the ambient reach the actor, the
//                               ground fill does not (that is the light's culling mask, not this).
//   * _ActorExposure          — a GLOBAL, set once per frame like Godot's global uniform. Pixel art
//                               carries its own painted shading, so the sun tuned for the ground
//                               over-lights the sprites; this dims the actors ALONE.
//   * _Flash REPLACES colour  — it does not multiply. An overbright tint cannot survive the 8-bit
//                               vertex colour path, and Godot hit exactly that; replacing the
//                               colour is a bigger, cheaper jump that needs no HDR.
//   * alpha clip + ShadowCaster — a clipped (not blended) sprite is what lets the billboard cast a
//                               real shadow; Godot needs `depth_prepass_alpha` for the same reason.
Shader "ROI/ActorSprite"
{
    Properties
    {
        [PerRendererData] _MainTex ("Sprite", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _Flash ("Flash", Range(0,1)) = 0
        _FlashColor ("Flash Colour", Color) = (1,1,1,1)
        _Cutoff ("Alpha Cutoff", Range(0,1)) = 0.5
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
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE
            #pragma multi_compile_fragment _ _SHADOWS_SOFT
            #pragma multi_compile_fragment _ _LIGHT_COOKIES
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST; float4 _Color; float4 _FlashColor; float _Flash; float _Cutoff;
            CBUFFER_END
            float _ActorExposure;
            float4 _BreakKeyPosition;
            float4 _BreakKeyColor;

            struct A { float4 positionOS : POSITION; float3 normalOS : NORMAL; float2 uv : TEXCOORD0; float4 color : COLOR; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; float3 positionWS : TEXCOORD1;
                       float3 normalWS : TEXCOORD2; float4 color : COLOR; };

            V vert(A i)
            {
                V o;
                float3 wp = TransformObjectToWorld(i.positionOS.xyz);
                o.positionWS = wp;
                o.positionCS = TransformWorldToHClip(wp);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                // The quad's OWN normal — which, being billboarded, faces the camera. That is
                // what Godot's Sprite3D does, and it is why the sprites sat visibly darker than
                // the ground with the normal pinned up: against this camera and sun, N.L is 0.85
                // facing the lens versus 0.56 facing straight up, a third of the light missing.
                // The trade is Godot's too — turn the camera and every sprite changes brightness.
                o.normalWS = TransformObjectToWorldNormal(i.normalOS);
                o.color = i.color;
                return o;
            }

            half4 frag(V i) : SV_Target
            {
                half4 tex = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv);
                half4 c = tex * i.color * _Color;
                clip(c.a - _Cutoff);

                // The billboard CASTS a shadow but does not RECEIVE one. A camera-facing quad
                // standing in its own shadow map shadows ITSELF — half the slimes came out black
                // — and a pixel-art sprite has its shading painted in anyway, so what a received
                // shadow would add is mostly artefact. The character is anchored to the ground by
                // its contact-shadow blob instead, which is how the Godot scene does it too.
                Light L = GetMainLight();
                // The sun's cookie is the drifting cloud shadow (ArenaView.BuildCloudCookie): the
                // same shade on the sprite as on the ground it stands on, which is the whole
                // point of Godot's shared cloud uniform. Applied by hand rather than through the
                // positionWS overload of GetMainLight, which would also sample a shadow this
                // sprite deliberately does not receive.
                #if defined(_LIGHT_COOKIES)
                L.color *= SampleMainLightCookie(i.positionWS);
                #endif
                half ndl = saturate(dot(i.normalWS, L.direction));
                half3 lit = c.rgb * (SampleSH(i.normalWS) + L.color * ndl);
                // Isolated presentation key; zero outside BREAK, so ordinary actor lighting stays unchanged.
                float3 toKey = _BreakKeyPosition.xyz - i.positionWS;
                float distanceSq = max(dot(toKey, toKey), .1);
                float keyRange = max(_BreakKeyPosition.w, .01);
                float falloff = saturate(1.0 - distanceSq / (keyRange * keyRange));
                lit += c.rgb * _BreakKeyColor.rgb * (falloff * falloff / distanceSq)
                     * saturate(dot(normalize(i.normalWS), toKey * rsqrt(distanceSq)));
                lit *= _ActorExposure;

                // Replace, never multiply — see the header.
                lit = lerp(lit, _FlashColor.rgb, _Flash);
                return half4(lit, 1);
            }
            ENDHLSL
        }

        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode"="ShadowCaster" }
            ZWrite On ColorMask 0
            HLSLPROGRAM
            #pragma vertex vertS
            #pragma fragment fragS
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Shadows.hlsl"
            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST; float4 _Color; float4 _FlashColor; float _Flash; float _Cutoff;
            CBUFFER_END
            struct AS { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct VS { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            VS vertS(AS i)
            {
                VS o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                return o;
            }
            half4 fragS(VS i) : SV_Target
            {
                clip(SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).a - _Cutoff);
                return 0;
            }
            ENDHLSL
        }

        // See GroundArt's DepthOnly pass: with MSAA, URP's depth texture comes from a prepass
        // that draws this pass only. Without it the sprite is missing from depth and every
        // depth-reading effect (depth of field first) treats it as infinitely far.
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

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST; float4 _Color; float4 _FlashColor; float _Flash; float _Cutoff;
            CBUFFER_END

            struct AD { float4 positionOS : POSITION; float2 uv : TEXCOORD0; float4 color : COLOR; };
            struct VD { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; float4 color : COLOR; };

            VD vertD(AD i)
            {
                VD o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                o.color = i.color;
                return o;
            }

            // The same cut as ForwardLit, or the depth silhouette does not match what is drawn.
            half fragD(VD i) : SV_Target
            {
                half4 c = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv) * i.color * _Color;
                clip(c.a - _Cutoff);
                return 0;
            }
            ENDHLSL
        }

        // See GroundArt's DepthNormals pass: the SSAO feature's depth-normals prepass is where
        // the camera depth texture comes from, so a shader needs THIS pass to be in depth at all.
        Pass
        {
            Name "DepthNormals"
            Tags { "LightMode"="DepthNormals" }
            ZWrite On
            HLSLPROGRAM
            #pragma vertex vertN
            #pragma fragment fragN
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST; float4 _Color; float4 _FlashColor; float _Flash; float _Cutoff;
            CBUFFER_END

            struct AN { float4 positionOS : POSITION; float3 normalOS : NORMAL; float2 uv : TEXCOORD0; float4 color : COLOR; };
            struct VN { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; float3 normalWS : TEXCOORD1; float4 color : COLOR; };

            VN vertN(AN i)
            {
                VN o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.uv = TRANSFORM_TEX(i.uv, _MainTex);
                o.normalWS = TransformObjectToWorldNormal(i.normalOS);
                o.color = i.color;
                return o;
            }

            half4 fragN(VN i) : SV_Target
            {
                half4 c = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv) * i.color * _Color;
                clip(c.a - _Cutoff);
                return half4(normalize(i.normalWS), 0);
            }
            ENDHLSL
        }
    }
    Fallback Off
}
