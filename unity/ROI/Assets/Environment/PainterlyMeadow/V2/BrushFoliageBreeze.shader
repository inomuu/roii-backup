Shader "ROI/Painterly/BrushFoliageBreeze"
{
    Properties
    {
        [MainTexture] _BaseMap("Foliage", 2D) = "white" {}
        [MainColor] _BaseColor("Colour", Color) = (1,1,1,1)
        _Cutoff("Alpha cutoff", Range(0,1)) = .35
        _ChromaCutoff("Neutral backdrop saturation cutoff", Range(0,1)) = .3
        _Smoothness("Smoothness", Range(0,1)) = .08
        _Metallic("Metallic", Range(0,1)) = 0
        _BumpMap("Normal", 2D) = "bump" {}
        _BumpScale("Normal scale", Float) = 1
        _EmissionColor("Emission", Color) = (0,0,0,0)
        _EmissionMap("Emission map", 2D) = "white" {}
        _WindStrength("Tip sway (local metres)", Range(0,.05)) = .012
        _WindSpeed("Breeze speed", Range(0,3)) = .8
        _RockMask("Fixed rock ellipse: centre XY, radius ZW", Vector) = (.65,.38,.34,.32)
        _RockProtection("Protect rock", Range(0,1)) = 1
        [HideInInspector] _Surface("Surface", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="TransparentCutout" "Queue"="AlphaTest" }
        Cull Off
        HLSLINCLUDE
        #define _ALPHATEST_ON 1
        #define _RECEIVE_SHADOWS_OFF 1
        #include "Packages/com.unity.render-pipelines.universal/Shaders/LitInput.hlsl"
        float _WindStrength, _WindSpeed, _RockProtection, _ChromaCutoff;
        float4 _RockMask;
        void ClipNeutralBackdrop(float2 uv)
        {
            // Rendering cutout only: the source PNG still contains an opaque gray checkerboard.
            // Classify raw texture RGB before any material tint, lighting, fog, or colour grade.
            half3 raw = SAMPLE_TEXTURE2D(_BaseMap, sampler_BaseMap, uv).rgb;
            half maximum = max(raw.r,max(raw.g,raw.b));
            half saturation = (maximum - min(raw.r,min(raw.g,raw.b))) / max(maximum,.001h);
            clip(saturation - _ChromaCutoff);
        }
        float3 Breeze(float3 position, float2 uv)
        {
            float3 origin = TransformObjectToWorld(float3(0,0,0));
            float phase = dot(origin.xz, float2(.71,1.13));
            float t = _Time.y * _WindSpeed;
            float rock = smoothstep(.85,1.15,length((uv-_RockMask.xy)/max(_RockMask.zw,.001)));
            float weight = smoothstep(.15,.85,uv.y) * lerp(1,rock,_RockProtection);
            float gust = .7 + .3*sin(t*.37+phase*.2);
            position.x += weight*_WindStrength*gust*(sin(t+phase)+.22*sin(t*2.7+uv.y*9+phase));
            position.y += weight*_WindStrength*.15*sin(t*1.6+uv.x*11+phase);
            return position;
        }
        ENDHLSL
        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma target 3.0
            #pragma vertex BreezeVertex
            #pragma fragment BrushLitFragment
            #define _ALPHATEST_ON 1
            #define _RECEIVE_SHADOWS_OFF 1
            #pragma multi_compile _ _ADDITIONAL_LIGHTS_VERTEX _ADDITIONAL_LIGHTS
            #pragma multi_compile _ _FORWARD_PLUS
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/Shaders/LitForwardPass.hlsl"
            Varyings BreezeVertex(Attributes input)
            {
                input.positionOS.xyz = Breeze(input.positionOS.xyz,input.texcoord);
                return LitPassVertex(input);
            }
            void BrushLitFragment(Varyings input, out half4 outColor : SV_Target0
            #ifdef _WRITE_RENDERING_LAYERS
                , out float4 outRenderingLayers : SV_Target1
            #endif
            )
            {
                UNITY_SETUP_INSTANCE_ID(input);
                UNITY_SETUP_STEREO_EYE_INDEX_POST_VERTEX(input);
                ClipNeutralBackdrop(input.uv);
                LitPassFragment(input,outColor
                #ifdef _WRITE_RENDERING_LAYERS
                    ,outRenderingLayers
                #endif
                );
            }
            ENDHLSL
        }
        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ZWrite On
            ColorMask R
            HLSLPROGRAM
            #pragma vertex BreezeDepthVertex
            #pragma fragment BrushDepthFragment
            #define _ALPHATEST_ON 1
            #include "Packages/com.unity.render-pipelines.universal/Shaders/DepthOnlyPass.hlsl"
            Varyings BreezeDepthVertex(Attributes input)
            {
                input.position.xyz = Breeze(input.position.xyz,input.texcoord);
                return DepthOnlyVertex(input);
            }
            half BrushDepthFragment(Varyings input) : SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(input);
                UNITY_SETUP_STEREO_EYE_INDEX_POST_VERTEX(input);
                ClipNeutralBackdrop(input.uv);
                return DepthOnlyFragment(input);
            }
            ENDHLSL
        }
    }
}
