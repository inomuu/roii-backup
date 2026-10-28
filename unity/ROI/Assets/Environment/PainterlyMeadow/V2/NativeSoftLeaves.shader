Shader "ROI/Painterly/NativeSoftLeaves"
{
    Properties
    {
        [MainTexture] _BaseMap("Leaf atlas (RGBA)", 2D) = "white" {}
        [MainColor] _BaseColor("Sage colour", Color) = (.5,.62,.24,1)
        _Cutoff("Alpha cutoff", Range(0,1)) = .45
        _TextureStrength("Atlas shading contrast", Range(0,1)) = .32
        _NormalUp("Upward normal blend", Range(0,1)) = .66
        _CelBlend("Broad soft light bands", Range(0,1)) = .25
        _AmbientStrength("Ambient light", Range(0,2)) = .8
        _WindStrength("Sway (local metres)", Range(0,.2)) = .045
        _WindSpeed("Breeze speed", Range(0,3)) = .8
        _WindBase("Fixed base height (local)", Float) = 0
        _WindHeight("Full sway height (local)", Float) = 2
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="TransparentCutout" "Queue"="AlphaTest" }
        Cull Off
        ZWrite On
        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
        TEXTURE2D(_BaseMap); SAMPLER(sampler_BaseMap);
        CBUFFER_START(UnityPerMaterial)
        float4 _BaseMap_ST, _BaseColor;
        float _Cutoff, _TextureStrength, _NormalUp, _CelBlend, _AmbientStrength;
        float _WindStrength, _WindSpeed, _WindBase, _WindHeight;
        CBUFFER_END
        struct Attributes
        {
            float4 positionOS : POSITION;
            float3 normalOS : NORMAL;
            float2 uv : TEXCOORD0;
            UNITY_VERTEX_INPUT_INSTANCE_ID
        };
        struct Varyings
        {
            float4 positionCS : SV_POSITION;
            float2 uv : TEXCOORD0;
            float3 positionWS : TEXCOORD1;
            half3 normalWS : TEXCOORD2;
            half fog : TEXCOORD3;
            UNITY_VERTEX_INPUT_INSTANCE_ID
            UNITY_VERTEX_OUTPUT_STEREO
        };
        float3 Sway(float3 positionOS)
        {
            float3 origin = TransformObjectToWorld(float3(0,0,0));
            float phase = dot(origin.xz, float2(.71,1.13));
            float t = _Time.y * _WindSpeed;
            // Local height fixes roots; atlas UVs restart independently on every branch.
            float weight = smoothstep(0,1,saturate((positionOS.y-_WindBase)/max(_WindHeight,.01)));
            float gust = .75 + .25*sin(t*.37+phase*.21);
            float sway = sin(t+phase) + .22*sin(t*2.3+positionOS.x*.8+positionOS.z*.4);
            positionOS.x += sway * weight * gust * _WindStrength;
            positionOS.z += sin(t*.83+phase+.9) * weight * _WindStrength * .36;
            return positionOS;
        }
        Varyings LeafVertex(Attributes input)
        {
            Varyings output = (Varyings)0;
            UNITY_SETUP_INSTANCE_ID(input);
            UNITY_TRANSFER_INSTANCE_ID(input, output);
            UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(output);
            output.positionWS = TransformObjectToWorld(Sway(input.positionOS.xyz));
            output.positionCS = TransformWorldToHClip(output.positionWS);
            output.normalWS = TransformObjectToWorldNormal(input.normalOS);
            output.uv = TRANSFORM_TEX(input.uv, _BaseMap);
            output.fog = ComputeFogFactor(output.positionCS.z);
            return output;
        }
        half4 Atlas(float2 uv)
        {
            half4 texel = SAMPLE_TEXTURE2D(_BaseMap, sampler_BaseMap, uv);
            clip(texel.a * _BaseColor.a - _Cutoff);
            return texel;
        }
        half SoftDiffuse(half3 normal, half3 direction)
        {
            half wrapped = saturate((dot(normal,direction)+.28h)/1.28h);
            half bands = .18h + .43h*smoothstep(.18h,.43h,wrapped) + .39h*smoothstep(.58h,.84h,wrapped);
            return lerp(wrapped,bands,_CelBlend);
        }
        ENDHLSL

        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma target 3.0
            #pragma vertex LeafVertex
            #pragma fragment LeafFragment
            #pragma multi_compile_instancing
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile _ _ADDITIONAL_LIGHTS
            #pragma multi_compile_fog
            half4 LeafFragment(Varyings input, FRONT_FACE_TYPE facing : FRONT_FACE_SEMANTIC) : SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(input);
                UNITY_SETUP_STEREO_EYE_INDEX_POST_VERTEX(input);
                half4 texel = Atlas(input.uv);
                half3 leafNormal = IS_FRONT_VFACE(facing, normalize(input.normalWS), -normalize(input.normalWS));
                half3 normal = normalize(lerp(leafNormal,half3(0,1,0),_NormalUp) + half3(0,.001,0));
                #if defined(_MAIN_LIGHT_SHADOWS_SCREEN)
                float4 shadowCoord = ComputeScreenPos(TransformWorldToHClip(input.positionWS));
                #else
                float4 shadowCoord = TransformWorldToShadowCoord(input.positionWS);
                #endif
                Light main = GetMainLight(shadowCoord, input.positionWS, half4(1,1,1,1));
                // Compress the atlas's baked dark veins; silhouette still uses its original alpha.
                half3 albedo = _BaseColor.rgb * lerp(half3(1,1,1),texel.rgb,_TextureStrength);
                half3 ambient = max(SampleSH(normal),half3(.20,.25,.23)) * _AmbientStrength;
                half3 lighting = ambient + main.color * SoftDiffuse(normal,main.direction)
                    * main.distanceAttenuation * main.shadowAttenuation * .85h;
                #if defined(_ADDITIONAL_LIGHTS)
                uint count = GetAdditionalLightsCount();
                for (uint i = 0; i < count; i++)
                {
                    Light extra = GetAdditionalLight(i,input.positionWS);
                    lighting += extra.color * SoftDiffuse(normal,extra.direction) * extra.distanceAttenuation * extra.shadowAttenuation;
                }
                #endif
                return half4(MixFog(albedo*lighting,input.fog),1);
            }
            ENDHLSL
        }
        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode"="ShadowCaster" }
            ZTest LEqual
            ColorMask 0
            HLSLPROGRAM
            #pragma target 3.0
            #pragma vertex ShadowVertex
            #pragma fragment ShadowFragment
            #pragma multi_compile_instancing
            #pragma multi_compile_vertex _ _CASTING_PUNCTUAL_LIGHT_SHADOW
            float3 _LightDirection, _LightPosition;
            Varyings ShadowVertex(Attributes input)
            {
                Varyings output = LeafVertex(input);
                #if defined(_CASTING_PUNCTUAL_LIGHT_SHADOW)
                float3 direction = normalize(_LightPosition-output.positionWS);
                #else
                float3 direction = _LightDirection;
                #endif
                output.positionCS = ApplyShadowClamping(TransformWorldToHClip(
                    ApplyShadowBias(output.positionWS,normalize(output.normalWS),direction)));
                return output;
            }
            half4 ShadowFragment(Varyings input) : SV_Target { UNITY_SETUP_INSTANCE_ID(input); Atlas(input.uv); return 0; }
            ENDHLSL
        }
        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ColorMask R
            HLSLPROGRAM
            #pragma target 3.0
            #pragma vertex LeafVertex
            #pragma fragment DepthFragment
            #pragma multi_compile_instancing
            half4 DepthFragment(Varyings input) : SV_Target { UNITY_SETUP_INSTANCE_ID(input); Atlas(input.uv); return input.positionCS.z; }
            ENDHLSL
        }
    }
    FallBack Off
}
