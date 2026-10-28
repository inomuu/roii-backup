Shader "ROI/UI/CelestialMotion"
{
    Properties
    {
        [PerRendererData] _MainTex ("Background", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _MotionTime ("Motion Time", Float) = 0
        _Aspect ("Aspect", Float) = 1.77778
        _StarStrength ("Star Strength", Range(0,1)) = .5
        _HazeStrength ("Haze Strength", Range(0,1)) = .4
        _StencilComp ("Stencil Comparison", Float) = 8
        _Stencil ("Stencil ID", Float) = 0
        _StencilOp ("Stencil Operation", Float) = 0
        _StencilWriteMask ("Stencil Write Mask", Float) = 255
        _StencilReadMask ("Stencil Read Mask", Float) = 255
        _ColorMask ("Color Mask", Float) = 15
        [Toggle(UNITY_UI_ALPHACLIP)] _UseUIAlphaClip ("Use Alpha Clip", Float) = 0
    }
    SubShader
    {
        Tags { "Queue"="Transparent" "IgnoreProjector"="True" "RenderType"="Transparent" "PreviewType"="Plane" "CanUseSpriteAtlas"="True" }
        Stencil { Ref [_Stencil] Comp [_StencilComp] Pass [_StencilOp] ReadMask [_StencilReadMask] WriteMask [_StencilWriteMask] }
        Cull Off Lighting Off ZWrite Off ZTest [unity_GUIZTestMode]
        Blend SrcAlpha OneMinusSrcAlpha
        ColorMask [_ColorMask]
        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 3.0
            #pragma multi_compile_local _ UNITY_UI_CLIP_RECT
            #pragma multi_compile_local _ UNITY_UI_ALPHACLIP
            #include "UnityCG.cginc"
            #include "UnityUI.cginc"
            struct appdata { float4 vertex:POSITION; float4 color:COLOR; float2 uv:TEXCOORD0; };
            struct v2f { float4 vertex:SV_POSITION; float4 color:COLOR; float2 uv:TEXCOORD0; float4 local:TEXCOORD1; };
            sampler2D _MainTex;
            fixed4 _Color, _TextureSampleAdd;
            float4 _ClipRect;
            float _MotionTime, _Aspect, _StarStrength, _HazeStrength;
            v2f vert(appdata v)
            {
                v2f o; o.local = v.vertex; o.vertex = UnityObjectToClipPos(v.vertex);
                o.uv = v.uv; o.color = v.color * _Color; return o;
            }
            float hash21(float2 p)
            {
                p = frac(p * float2(123.34, 456.21));
                p += dot(p, p + 45.32); return frac(p.x * p.y);
            }
            float stars(float2 uv, float density, float seed)
            {
                float2 grid = uv * density;
                float2 cell = floor(grid);
                float random = hash21(cell + seed);
                float2 center = .23 + .54 * float2(hash21(cell + 4.1 + seed), hash21(cell + 19.7 + seed));
                float2 delta = frac(grid) - center;
                float radius = length(delta);
                float core = 1 - smoothstep(.012, .050, radius);
                float halo = .13 * (1 - smoothstep(.025, .23, radius));
                float rays = exp(-abs(delta.x)*160 - abs(delta.y)*14) + exp(-abs(delta.y)*160 - abs(delta.x)*14);
                float pulse = .18 + .82 * pow(.5 + .5 * sin(_MotionTime * (.65 + random*.5) + random*47), 3);
                return step(.955,random) * pulse * (core + halo + rays * .16);
            }
            fixed4 frag(v2f i):SV_Target
            {
                fixed4 c = (tex2D(_MainTex,i.uv) + _TextureSampleAdd) * i.color;
                float2 uv = i.uv * float2(_Aspect,1);
                // Two slow independent layers give depth without moving the UI or the artwork.
                float a = stars(uv + float2(_MotionTime*.00065, _MotionTime*.00018), 23, 1.3);
                float b = stars(uv + float2(-_MotionTime*.00032, _MotionTime*.0001), 37, 23.7);
                float sky = smoothstep(.29,.46,i.uv.y);
                c.rgb += (a + b*.4) * _StarStrength * sky * float3(.79,.86,1);
                float mist = sin(i.uv.x*5.2 + i.uv.y*3.8 + _MotionTime*.09) * sin(i.uv.y*4.1 - _MotionTime*.075);
                c.rgb += mist * _HazeStrength * sky * float3(.035,.047,.066);
                #ifdef UNITY_UI_CLIP_RECT
                c.a *= UnityGet2DClipping(i.local.xy,_ClipRect);
                #endif
                #ifdef UNITY_UI_ALPHACLIP
                clip(c.a-.001);
                #endif
                return c;
            }
            ENDCG
        }
    }
}
