Shader "ROI/IronChainMetal"
{
    Properties
    {
        _Opacity ("Opacity", Range(0,1))=1
        _Charge ("Charge", Range(0,1))=0
        _Outline ("Outline Pass", Float)=0
        _OutlineWidth ("Outline Width", Float)=0
        [Enum(UnityEngine.Rendering.CullMode)] _Cull ("Cull", Float)=2
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+2" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            Tags { "LightMode"="SRPDefaultUnlit" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off
            Cull [_Cull]
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                float _Opacity, _Charge, _Outline, _OutlineWidth;
            CBUFFER_END
            struct A { float4 positionOS:POSITION; float3 normalOS:NORMAL; };
            struct V { float4 positionCS:SV_POSITION; float3 normal:TEXCOORD0; float3 positionWS:TEXCOORD1; };
            V vert(A i)
            {
                V o; o.positionWS=TransformObjectToWorld(i.positionOS.xyz+i.normalOS*_OutlineWidth);
                o.positionCS=TransformWorldToHClip(o.positionWS);
                o.normal=TransformObjectToWorldNormal(i.normalOS); return o;
            }
            half4 frag(V i):SV_Target
            {
                if (_Outline>.5) return half4(.0003,.0005,.001,_Opacity);
                float3 n=normalize(i.normal), view=normalize(GetWorldSpaceViewDir(i.positionWS));
                float diffuse=saturate(dot(n,normalize(float3(-.4,.85,-.3))));
                float facing=saturate(dot(n,view));
                float lit=smoothstep(.46,.5,diffuse);
                float highlight=smoothstep(.82,.87,diffuse);
                float edge=smoothstep(.57,.64,facing);
                float3 color=lerp(float3(.06,.32,.72),float3(.65,.9,1.2),lit);
                color=lerp(color,float3(1.25,1.4,1.5),highlight);
                color+=float3(.01,.08,.18)*_Charge;
                color=lerp(float3(.001,.003,.009),color,edge);
                return half4(color,_Opacity);
            }
            ENDHLSL
        }
    }
}
