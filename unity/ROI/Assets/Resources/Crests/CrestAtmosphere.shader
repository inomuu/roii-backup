Shader "ROI/UI/CrestAtmosphere"
{
    Properties
    {
        [PerRendererData] _MainTex ("UI texture", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _Accent ("Crest atmosphere tint", Color) = (.35,.55,.65,1)
        _MotionTime ("Unscaled motion time", Float) = 0
        _Aspect ("Aspect", Float) = 1.2931
        _FogStrength ("Mist strength", Range(0,1)) = .23
        _PatternStrength ("Etched pattern", Range(0,1)) = 0
        _NebulaStrength ("Nebula strength", Range(0,1)) = .5
        _NebulaTex ("Structured nebula artwork", 2D) = "black" {}
        _UseNebulaTexture ("Use nebula artwork", Float) = 0
        _ArtworkStrength ("Nebula artwork strength", Range(0,1)) = .55
        _SparkleTime ("Unscaled sparkle time", Float) = 0
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
            struct appdata { float4 vertex:POSITION; fixed4 color:COLOR; float2 uv:TEXCOORD0; };
            struct v2f { float4 vertex:SV_POSITION; fixed4 color:COLOR; float2 uv:TEXCOORD0; float4 local:TEXCOORD1; };
            float4 _ClipRect,_Color,_Accent;
            float _MotionTime,_Aspect,_FogStrength,_PatternStrength,_NebulaStrength;
            sampler2D _NebulaTex;
            float _UseNebulaTexture,_ArtworkStrength,_SparkleTime;
            v2f vert(appdata v)
            { v2f o;o.local=v.vertex;o.vertex=UnityObjectToClipPos(v.vertex);o.color=v.color*_Color;o.uv=v.uv;return o; }
            float hash(float2 p)
            { p=frac(p*float2(123.34,456.21));p+=dot(p,p+45.32);return frac(p.x*p.y); }
            float noise(float2 p)
            {
                float2 cell=floor(p),f=frac(p);f=f*f*(3-2*f);
                return lerp(lerp(hash(cell),hash(cell+float2(1,0)),f.x),
                    lerp(hash(cell+float2(0,1)),hash(cell+1),f.x),f.y);
            }
            float cloud(float2 p)
            { return noise(p)*.58+noise(p*2.03+7.8)*.28+noise(p*4.07+17.1)*.14; }
            fixed4 frag(v2f i):SV_Target
            {
                float2 uv=i.uv,p=(uv-.5)*float2(_Aspect,1);
                float t=_MotionTime;
                float3 base;
                float edgeFade=smoothstep(0,.12,uv.x)*smoothstep(0,.12,1-uv.x)
                    *smoothstep(0,.12,uv.y)*smoothstep(0,.12,1-uv.y);
                if(_UseNebulaTexture>.5)
                {
                    // Preserve the square painting's proportions and leave overscan for drift.
                    float2 crop=_Aspect>=1?float2(1,1/_Aspect):float2(_Aspect,1);
                    float2 artUV=(uv-.5)*crop*.94+.5;
                    artUV+=float2(sin(t*.035),cos(t*.027))*float2(.012,.009);
                    float3 art=tex2D(_NebulaTex,artUV).rgb*.82;
                    base=lerp(float3(.010,.014,.024),art,_ArtworkStrength);
                }
                else
                {
                float vignette=saturate(1-dot(p,p)*1.25);
                base=lerp(float3(.013,.018,.031),float3(.039,.052,.080),vignette);
                // Quiet etched-diamond textile underneath the moving mist. Derivative AA
                // keeps this pattern stable at companion 1x, 1.5x and 2x scales.
                float2 tile=abs(frac(uv*float2(8,6))-.5);
                float edge=abs(tile.x+tile.y-.44);
                float aa=max(fwidth(edge),.008);
                float pattern=1-smoothstep(.008,.008+aa,edge);
                base+=pattern*_PatternStrength*.055*vignette;
                base+=(hash(floor(uv*float2(600,464)))-.5)*.006;
                // Broken, warped ribbons leave real dark gaps between banks of mist.
                // A narrow density ridge gives each bank a visible curled silhouette.
                float2 drift=float2(t*.022,t*.006);
                float warp=cloud(p*float2(3.1,2.4)-drift)-.5;
                float curl=.105*sin(p.x*7.2+t*.045)+warp*.24;
                float ridgeA=abs(p.y-(-.21+p.x*.25+curl));
                float ridgeB=abs(p.y-(.23-p.x*.30-curl*.75));
                float ribbonA=1-smoothstep(.014,.11,ridgeA);
                float ribbonB=1-smoothstep(.012,.085,ridgeB);
                float breakup=cloud(p*float2(4.2,6.4)+drift*float2(-1,1)+8.4);
                float density=smoothstep(.32,.66,breakup);
                float wisps=saturate((ribbonA+ribbonB*.65)*density);
                // Blend colored pigment instead of adding neutral white light, so overlap
                // cannot wash the background out. Keep the center quiet behind the Crest.
                float centerClear=lerp(.45,1,smoothstep(.10,.38,length(p)));
                float3 fogColor=float3(.022,.033,.057)+_Accent.rgb*.10;
                base=lerp(base,fogColor,saturate(wisps*_FogStrength*3)*edgeFade*centerClear);
                // Colored cloud banks with a dark dust lane, rather than white haze.
                float mass=cloud(p*float2(2.5,3.4)+float2(warp*.8,t*.008));
                float band=1-smoothstep(.10,.48,abs(p.y+p.x*.43+warp*.55));
                float dust=smoothstep(.025,.12,abs(p.y+p.x*.43+warp*.32+.03));
                float nebula=smoothstep(.30,.74,mass)*band*dust*edgeFade;
                float3 nebulaColor=lerp(float3(.027,.075,.135),float3(.105,.035,.13),smoothstep(-.5,.5,p.x+warp));
                base=lerp(base,nebulaColor,nebula*_NebulaStrength);
                }
                // Larger soft stars; long rays only on the rare brightest ones.
                float2 grid=(p+float2(t*.0008,0))*26,cell=floor(grid);
                float rnd=hash(cell+31);float2 st=abs(frac(grid)-.5);
                float radius=length(st),large=step(.992,rnd);
                // Staggered 4-8 second pulses: a small core stays while its halo expands
                // then fades completely. Each cell has a different phase and duration.
                float phase=frac(_SparkleTime/(4+rnd*4)+rnd*13);
                float pulse=sin(phase*3.14159265);pulse*=pulse;
                float scale=lerp(.65,1.45,phase);
                float star=1-smoothstep(.012,lerp(.10,.17,large)*scale,radius);
                float rays=exp(-st.x*95-st.y*10)+exp(-st.y*95-st.x*10);
                float halo=(1-smoothstep(0,lerp(.07,.30,phase),radius))*pulse;
                star=star*(.035+pulse*.20)+rays*large*pulse*.10+halo*large*.055;
                base+=star*step(.96,rnd)*float3(.7,.81,1)*edgeFade;
                fixed4 c=fixed4(base*i.color.rgb,i.color.a);
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
