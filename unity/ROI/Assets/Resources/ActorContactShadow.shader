Shader "ROI/ActorContactShadow"
{
    Properties { _Opacity ("Opacity", Range(0,1)) = .30 }
    SubShader
    {
        Tags { "Queue"="Transparent-20" "RenderType"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off Cull Off
        Offset -1,-1
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            struct A { float4 position:POSITION;float2 uv:TEXCOORD0; };
            struct V { float4 position:SV_POSITION;float2 uv:TEXCOORD0; };
            float _Opacity;
            V vert(A v){V o;o.position=TransformObjectToHClip(v.position.xyz);o.uv=v.uv;return o;}
            half4 frag(V i):SV_Target
            {
                float r=length((i.uv-.5)*2);
                float a=(1-smoothstep(.10,1,r));a*=a;
                return half4(.015,.021,.032,a*_Opacity);
            }
            ENDHLSL
        }
    }
}
