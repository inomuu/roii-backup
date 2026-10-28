Shader "ROI/SeraphineAegisField"
{
    Properties
    {
        _MainTex ("Water mask", 2D) = "gray" {}
        _Mode ("Layer", Float) = 0
        _Opacity ("Opacity", Float) = 0
        _Phase ("Phase", Float) = 0
        _Radius ("Radius", Float) = .7
        [Enum(UnityEngine.Rendering.CullMode)] _Cull ("Cull", Float) = 0
    }
    SubShader
    {
        Tags { "Queue"="Transparent+20" "RenderType"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        Cull [_Cull]
        ZWrite Off
        ZTest LEqual
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_MainTex); SAMPLER(sampler_MainTex);
            CBUFFER_START(UnityPerMaterial)
                float _Mode, _Opacity, _Phase, _Radius;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float3 normalOS : NORMAL; float2 uv : TEXCOORD0; float2 surface : TEXCOORD1; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0;
                       float3 positionWS : TEXCOORD1; float3 normalWS : TEXCOORD2; float2 surface : TEXCOORD3; };
            V vert(A i)
            {
                V o; o.positionWS=TransformObjectToWorld(i.positionOS.xyz);
                o.positionCS=TransformWorldToHClip(o.positionWS); o.uv=i.uv;
                o.normalWS=TransformObjectToWorldNormal(i.normalOS); o.surface=i.surface; return o;
            }
            half4 frag(V i) : SV_Target
            {
                float2 p=(i.uv-.5)*2;
                float r=length(p);
                float tex=SAMPLE_TEXTURE2D(_MainTex,sampler_MainTex,i.uv*1.4-float2(_Phase*.04,_Phase*.06)).r;
                float core=0,body=0;
                if (_Mode < .5)
                {
                    float d=abs(r-_Radius);
                    core=exp(-d/.012)+.4*exp(-abs(r-(_Radius-.10))/.008);
                    body=exp(-d/.085)*(.3+tex)+.16*exp(-r*r*5);
                }
                else if (_Mode < 1.5)
                {
                    float radius=i.surface.x;
                    float aa=max(fwidth(radius),.002);
                    float border=smoothstep(.94-aa,.94+aa,radius);
                    float inner=exp(-abs(radius-.84)/(.01+aa*.3));
                    float rim=pow(1-saturate(abs(dot(normalize(i.normalWS),GetWorldSpaceNormalizeViewDir(i.positionWS)))),2.5);
                    float emblem=abs(i.uv.x-.5)/.075+abs(i.uv.y-.55)/.12;
                    core=border*.80+inner*.30+exp(-abs(emblem-1)/.12)*.40;
                    core*=lerp(1,.38,i.surface.y);
                    body=.32+tex*.09+rim*.28+exp(-abs(radius-.94)/.08)*.32;
                }
                else if (_Mode < 2.5)
                {
                    float vertical=smoothstep(0,.07,i.uv.y)*(1-smoothstep(.35,1,i.uv.y));
                    body=tex*vertical*.75;
                    core=exp(-abs(i.uv.y-.045)/.018)*.22;
                }
                else
                {
                    float d=abs(r-_Radius);
                    body=exp(-d*d/.018)*(.12+tex*1.7);
                    core=exp(-d/.014)*tex*.45;
                    float edge=1-smoothstep(.86,1,r);
                    body*=edge; core*=edge;
                }
                float3 rgb=float3(.30,.88,1.4)*core*1.8+float3(.035,.38,.85)*body;
                return half4(rgb*_Opacity,0);
            }
            ENDHLSL
        }
    }
}
