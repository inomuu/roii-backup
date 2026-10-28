Shader "ROI/UI/WhiteCircleGlow"
{
    Properties
    {
        [PerRendererData] _MainTex ("Circle luminance mask", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _GlowColor ("Tier glow hue", Color) = (1,1,1,1)
        _CoreStrength ("Line opacity", Range(0,1)) = .70
        _GlowStrength ("Halo opacity", Range(0,4)) = 1.1
        _GlowRadius ("Halo radius in source pixels", Range(1,40)) = 14
        _MotionTime ("Unscaled time", Float) = 0
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
        Tags { "Queue"="Transparent" "IgnoreProjector"="True" "RenderType"="Transparent" "PreviewType"="Plane" }
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
            struct v2f { float4 vertex:SV_POSITION; fixed4 color:COLOR; float2 uv:TEXCOORD0; float4 local:TEXCOORD1; };
            sampler2D _MainTex;
            float4 _MainTex_TexelSize, _ClipRect;
            fixed4 _Color,_GlowColor;
            float _CoreStrength, _GlowStrength, _GlowRadius, _MotionTime;
            v2f vert(appdata v)
            {
                v2f o;o.local=v.vertex;o.vertex=UnityObjectToClipPos(v.vertex);
                o.uv=v.uv;o.color=v.color*_Color;return o;
            }
            float mask(float2 uv)
            {
                // Remove low-level generator noise in the black field so rotating corners
                // never appear as a dim square. Preserve antialiasing on the white strokes.
                return smoothstep(.06,.70,tex2D(_MainTex,uv).r);
            }
            float taps(float2 uv,float2 d)
            {
                float a=mask(uv+float2(d.x,0))+mask(uv-float2(d.x,0));
                a+=mask(uv+float2(0,d.y))+mask(uv-float2(0,d.y));
                a+=mask(uv+d*.7071)+mask(uv-d*.7071);
                a+=mask(uv+float2(d.x,-d.y)*.7071)+mask(uv+float2(-d.x,d.y)*.7071);
                return a*.125;
            }
            fixed4 frag(v2f i):SV_Target
            {
                // Self-contained tier-colored halo: no scene bloom/HDR dependence and
                // no opaque black quad in the transparent companion window. 17 texture taps.
                float core=mask(i.uv);
                float2 radius=_MainTex_TexelSize.xy*_GlowRadius;
                float halo=taps(i.uv,radius*.32)*.64+taps(i.uv,radius)*.36;
                float pulse=.93+.07*sin(_MotionTime*.65);
                float opacity=saturate(core*_CoreStrength+halo*_GlowStrength)*pulse*i.color.a;
                float3 ink=lerp(_GlowColor.rgb,float3(1,1,1),.48);
                float3 hue=lerp(_GlowColor.rgb,ink,saturate(core*1.4));
                fixed4 c=fixed4(hue*i.color.rgb,opacity);
                #ifdef UNITY_UI_CLIP_RECT
                c.a*=UnityGet2DClipping(i.local.xy,_ClipRect);
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
