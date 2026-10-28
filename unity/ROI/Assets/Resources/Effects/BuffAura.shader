Shader "ROI/BuffAura"
{
    Properties
    {
        _MainTex ("Flow mask", 2D) = "black" {}
        [HDR] _Color ("Aura color", Color) = (.12,.75,1.5,1)
        _Opacity ("Opacity", Float) = 0
        _Phase ("Normalized age", Float) = 0
        _Mode ("Curtain / ribbon", Float) = 0
        _CoreWhiteness ("Core whiteness", Range(0, 1)) = .7
    }
    SubShader
    {
        Tags { "Queue"="Transparent+15" "RenderType"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        Cull Off
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
                float4 _Color;
                float _Opacity, _Phase, _Mode, _CoreWhiteness;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o; o.positionCS=TransformObjectToHClip(i.positionOS.xyz); o.uv=i.uv; return o;
            }
            half4 frag(V i) : SV_Target
            {
                float body,core;
                if (_Mode<.5)
                {
                    // Circular coordinates close the cylinder without a vertical texture seam.
                    float angle=i.uv.x*6.2831853;
                    float2 flow=float2(cos(angle)*.42+.5,sin(angle)*.24+i.uv.y*.65-_Phase*.55);
                    float mask=SAMPLE_TEXTURE2D(_MainTex,sampler_MainTex,flow).r;
                    float detail=SAMPLE_TEXTURE2D(_MainTex,sampler_MainTex,flow*1.7+float2(.3,-_Phase*.17)).r;
                    float top=.28+_Phase*1.15+(mask-.25)*.28;
                    float envelope=smoothstep(0,.08,i.uv.y)*(1-smoothstep(.57,1,i.uv.y))
                        *(1-smoothstep(top-.22,top,i.uv.y));
                    body=smoothstep(.09,.55,mask)*(.28+detail*.6)*envelope*.8;
                    core=pow(saturate(mask),3)*envelope*.35;
                }
                else
                {
                    float across=abs(i.uv.y-.5)*2;
                    float ends=smoothstep(0,.12,i.uv.x)*(1-smoothstep(.85,1,i.uv.x));
                    float mask=SAMPLE_TEXTURE2D(_MainTex,sampler_MainTex,
                        float2(i.uv.x*.8-_Phase*.24,i.uv.y*.3+i.uv.x*.2)).r;
                    float reveal=1-smoothstep(_Phase*2.5,_Phase*2.5+.15,i.uv.x);
                    float envelope=ends*reveal;
                    body=pow(saturate(1-across),1.6)*(.55+mask*.8)*envelope;
                    core=exp(-pow((i.uv.y-.58)/.095,2))*envelope*(.65+mask*.6);
                }
                float3 rgb=_Color.rgb*body+lerp(_Color.rgb,float3(1.6,1.9,2),_CoreWhiteness)*core;
                return half4(rgb*_Opacity*_Color.a,0);
            }
            ENDHLSL
        }
    }
}
