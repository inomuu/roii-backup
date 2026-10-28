Shader "ROI/Painterly/PaintedStoneSheen"
{
    Properties
    {
        [MainTexture] _BaseMap("Painted albedo", 2D) = "white" {}
        [MainColor] _BaseColor("Colour", Color) = (1, 1, 1, 1)
        _AlbedoSaturation("Albedo saturation", Range(0, 1.5)) = 0.75
        _CelBlend("Soft cel blend", Range(0, 1)) = 0.35
        _ShadowTint("Shadow colour", Color) = (0.66, 0.76, 0.84, 1)
        _Smoothness("Smoothness", Range(0, 1)) = 0.42
        _SpecularStrength("Stone sheen", Range(0, 2)) = 0.55
        _SpecularColor("Sheen colour", Color) = (1, 0.93, 0.78, 1)
        _Cutoff("Cutoff", Range(0, 1)) = 0.3
    }

    SubShader
    {
        Tags { "RenderPipeline" = "UniversalPipeline" "RenderType" = "Opaque" }

        Pass
        {
            Name "PaintedStoneForward"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile _ _ADDITIONAL_LIGHTS
            #pragma multi_compile_fragment _ _ADDITIONAL_LIGHT_SHADOWS
            #pragma multi_compile _ _FORWARD_PLUS
            #pragma multi_compile_fog

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            TEXTURE2D(_BaseMap);
            SAMPLER(sampler_BaseMap);

            CBUFFER_START(UnityPerMaterial)
                float4 _BaseMap_ST;
                half4 _BaseColor;
                half4 _ShadowTint;
                half4 _SpecularColor;
                half _AlbedoSaturation;
                half _CelBlend;
                half _Smoothness;
                half _SpecularStrength;
                half _Cutoff;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
                float2 uv : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                half3 normalWS : TEXCOORD1;
                float2 uv : TEXCOORD2;
                half fog : TEXCOORD3;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                output.positionWS = TransformObjectToWorld(input.positionOS.xyz);
                output.positionCS = TransformWorldToHClip(output.positionWS);
                output.normalWS = TransformObjectToWorldNormal(input.normalOS);
                output.uv = TRANSFORM_TEX(input.uv, _BaseMap);
                output.fog = ComputeFogFactor(output.positionCS.z);
                return output;
            }

            half3 PaintedLight(Light light, half3 albedo, half3 normalWS, half3 viewDirWS)
            {
                half ndl = saturate(dot(normalWS, light.direction));
                half bands = 0.18h + 0.40h * smoothstep(0.18h, 0.30h, ndl)
                    + 0.42h * smoothstep(0.64h, 0.77h, ndl);
                half diffuse = lerp(ndl, bands, _CelBlend);
                half attenuation = light.distanceAttenuation * light.shadowAttenuation;

                // Continuous broad reflection reveals painted planes; the smaller lobe
                // gives occasional glints without thresholding them into plastic dots.
                half3 halfDir = SafeNormalize(light.direction + viewDirWS);
                half ndh = saturate(dot(normalWS, halfDir));
                half broad = pow(ndh, lerp(6.0h, 18.0h, _Smoothness));
                half glint = pow(ndh, exp2(4.0h + 4.0h * _Smoothness));
                half sheen = (0.45h * broad + 0.55h * glint) * _SpecularStrength * ndl;

                return light.color * attenuation
                    * (albedo * diffuse * 0.85h + _SpecularColor.rgb * sheen);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = normalize(input.normalWS);
                half3 viewDirWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half3 albedo = SAMPLE_TEXTURE2D(_BaseMap, sampler_BaseMap, input.uv).rgb;
                half luma = dot(albedo, half3(0.2126h, 0.7152h, 0.0722h));
                albedo = max(0.0h, lerp(luma.xxx, albedo, _AlbedoSaturation)) * _BaseColor.rgb;

                float4 shadowCoord;
                #if defined(_MAIN_LIGHT_SHADOWS_SCREEN)
                    shadowCoord = ComputeScreenPos(TransformWorldToHClip(input.positionWS));
                #else
                    shadowCoord = TransformWorldToShadowCoord(input.positionWS);
                #endif
                Light mainLight = GetMainLight(shadowCoord, input.positionWS, half4(1, 1, 1, 1));
                half3 ambient = max(SampleSH(normalWS), half3(0.17h, 0.20h, 0.22h));
                half3 colour = albedo * ambient * _ShadowTint.rgb;
                colour += PaintedLight(mainLight, albedo, normalWS, viewDirWS);

                #if defined(_ADDITIONAL_LIGHTS)
                    // URP's Forward+ loop reads these exact InputData fields.
                    InputData inputData = (InputData)0;
                    inputData.positionWS = input.positionWS;
                    inputData.normalizedScreenSpaceUV = GetNormalizedScreenSpaceUV(input.positionCS);

                    #if USE_FORWARD_PLUS
                        [loop] for (uint lightIndex = 0;
                            lightIndex < min(URP_FP_DIRECTIONAL_LIGHTS_COUNT, MAX_VISIBLE_LIGHTS);
                            ++lightIndex)
                        {
                            FORWARD_PLUS_SUBTRACTIVE_LIGHT_CHECK
                            Light extra = GetAdditionalLight(lightIndex, input.positionWS, half4(1, 1, 1, 1));
                            colour += PaintedLight(extra, albedo, normalWS, viewDirWS);
                        }
                    #endif

                    uint lightCount = GetAdditionalLightsCount();
                    LIGHT_LOOP_BEGIN(lightCount)
                        Light extra = GetAdditionalLight(lightIndex, input.positionWS, half4(1, 1, 1, 1));
                        colour += PaintedLight(extra, albedo, normalWS, viewDirWS);
                    LIGHT_LOOP_END
                #endif

                return half4(MixFog(colour, input.fog), 1);
            }
            ENDHLSL
        }

        UsePass "Universal Render Pipeline/Lit/ShadowCaster"
        UsePass "Universal Render Pipeline/Lit/DepthOnly"
    }
}
