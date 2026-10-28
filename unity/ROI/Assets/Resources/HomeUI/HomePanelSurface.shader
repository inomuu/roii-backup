// Use on a plain Image: UVs span 0..1, tint comes from Image.color.
// The restrained pattern is concentrated behind the watermark, away from the title.
Shader "ROI/UI/HomePanelSurface"
{
    Properties
    {
        [PerRendererData] _MainTex ("Unused UI texture", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _PatternStrength ("Pattern Strength", Range(0,2)) = 1
        _Aspect ("Panel Width / Height", Float) = 3
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
        Stencil
        {
            Ref [_Stencil]
            Comp [_StencilComp]
            Pass [_StencilOp]
            ReadMask [_StencilReadMask]
            WriteMask [_StencilWriteMask]
        }
        Cull Off
        Lighting Off
        ZWrite Off
        ZTest [unity_GUIZTestMode]
        // RGB is premultiplied below; preserve coverage in the transparent UI target.
        Blend One OneMinusSrcAlpha
        ColorMask [_ColorMask]
        Pass
        {
            CGPROGRAM
            #pragma target 3.0
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_local _ UNITY_UI_CLIP_RECT
            #pragma multi_compile_local _ UNITY_UI_ALPHACLIP
            #include "UnityCG.cginc"
            #include "UnityUI.cginc"

            struct appdata
            {
                float4 vertex : POSITION;
                float2 uv : TEXCOORD0;
                fixed4 color : COLOR;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };
            struct v2f
            {
                float4 vertex : SV_POSITION;
                float2 uv : TEXCOORD0;
                fixed4 color : COLOR;
                float4 local : TEXCOORD1;
                UNITY_VERTEX_OUTPUT_STEREO
            };
            fixed4 _Color;
            float _PatternStrength;
            float _Aspect;
            float4 _ClipRect;

            v2f vert(appdata v)
            {
                v2f o;
                UNITY_SETUP_INSTANCE_ID(v);
                UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
                o.local = v.vertex;
                o.vertex = UnityObjectToClipPos(v.vertex);
                o.uv = v.uv;
                o.color = v.color * _Color;
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                float2 uv = saturate(i.uv);
                float aspect = max(_Aspect, .25);
                // A few broad flat facets, never a texture under the left title column.
                float right = smoothstep(.55, .88, uv.x);
                float diagonal = uv.x + uv.y / aspect;
                float edgeAA = max(fwidth(diagonal), .0001);
                float facetA = smoothstep(.88 - edgeAA, .88 + edgeAA, diagonal);
                float facetB = smoothstep(1.03 - edgeAA, 1.03 + edgeAA, diagonal);
                float facetC = smoothstep(1.20 - edgeAA, 1.20 + edgeAA, diagonal);
                float tone = (-.036 * facetA + .064 * facetB - .050 * facetC) * right;

                // Dot spacing is square in panel space and follows perspective naturally.
                // Filter the dot edges; fade the pattern away when its grid is subpixel.
                float2 grid = uv * float2(aspect, 1) * 18;
                float footprint = max(length(ddx(grid)), length(ddy(grid)));
                float2 cell = frac(grid) - .5;
                float dist = length(cell);
                float dotAA = max(fwidth(dist), .001);
                float dots = 1 - smoothstep(.18 - dotAA, .18 + dotAA, dist);
                float lowerRight = smoothstep(.55, .88, uv.x)
                    * (1 - smoothstep(.08, .72, uv.y));
                float visibility = 1 - smoothstep(.30, .75, footprint);
                tone -= dots * lowerRight * visibility * .22;

                fixed4 color = i.color;
                color.rgb = saturate(color.rgb * (1 + tone * _PatternStrength));
                #ifdef UNITY_UI_CLIP_RECT
                color.a *= UnityGet2DClipping(i.local.xy, _ClipRect);
                #endif
                #ifdef UNITY_UI_ALPHACLIP
                clip(color.a - .001);
                #endif
                color.rgb *= color.a;
                return color;
            }
            ENDCG
        }
    }
}
