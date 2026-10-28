Shader "ROI/BuffGroundGlow"
{
    Properties
    {
        [HDR] _Color ("Ground color", Color) = (.08,.55,1.3,1)
        _Opacity ("Opacity", Float) = 0
        _TintStrength ("Ground tint", Range(0, 1)) = 0
        [HDR] _CoreColor ("Ground core", Color) = (0,0,0,0)
    }
    SubShader
    {
        Tags { "Queue"="Transparent+10" "RenderType"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One OneMinusSrcAlpha
        Cull Off
        ZWrite Off
        ZTest LEqual
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                float4 _Color, _CoreColor;
                float _Opacity, _TintStrength;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                float r=length((i.uv-.5)*2);
                float edge=1-smoothstep(.70,1,r);
                float pool=exp(-r*r*3.8)*1.4;
                float rim=exp(-pow((r-.59)/.075,2))*.25;
                float mask=(pool+rim)*edge*_Opacity*_Color.a;
                float core=exp(-pow(r/.31,2))*edge*_Opacity*_CoreColor.a;
                // Zero tint retains the original additive buff glow.
                return half4(_Color.rgb*mask+_CoreColor.rgb*core,saturate(mask*_TintStrength));
            }
            ENDHLSL
        }
    }
}
