Shader "ROI/SeraphineAegisCutin"
{
    Properties
    {
        [PerRendererData] _MainTex ("Portrait", 2D) = "white" {}
        _AuraTex ("Aura Flow Mask", 2D) = "gray" {}
        _AuraDetail ("Aura Texture Strength", Range(0,1)) = 1
        _Age ("Age", Float) = 0
        _Duration ("Duration", Float) = 1.8
        _Aspect ("Frame Aspect", Float) = 1.777778
        _PortraitAspect ("Portrait Aspect", Float) = 1.162341
    }
    SubShader
    {
        Tags { "Queue"="Overlay" "RenderType"="Transparent" "RenderPipeline"="UniversalPipeline" "IgnoreProjector"="True" }
        Blend SrcAlpha OneMinusSrcAlpha
        Cull Off
        ZWrite Off
        ZTest Always
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            TEXTURE2D(_MainTex);
            SAMPLER(sampler_MainTex);
            TEXTURE2D(_AuraTex);
            SAMPLER(sampler_AuraTex);
            CBUFFER_START(UnityPerMaterial)
                float _Age, _Duration, _Aspect, _PortraitAspect;
                float _AuraDetail;
            CBUFFER_END
            struct A { float4 positionOS : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 positionCS : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i)
            {
                V o;
                o.positionCS = TransformObjectToHClip(i.positionOS.xyz);
                o.uv = i.uv;
                return o;
            }
            float ramp(float a, float b, float t) { return smoothstep(a, b, t); }
            float segment(float2 p, float2 a, float2 b)
            {
                float2 v = b - a;
                return length(p - a - v * saturate(dot(p - a,v) / dot(v,v)));
            }
            float shield(float2 p)
            {
                p.x = abs(p.x);
                float d = segment(p,float2(0,.31),float2(.25,.31));
                d = min(d,segment(p,float2(.25,.31),float2(.30,.12)));
                d = min(d,segment(p,float2(.30,.12),float2(.22,-.19)));
                return min(d,segment(p,float2(.22,-.19),float2(0,-.37)));
            }
            float neon(float d, float width)
            {
                return exp(-d / width) + .16 * exp(-d / (width * 10));
            }
            float burstExpansion(float t)
            {
                // One outward release after the portrait settles; never wraps or restarts.
                float progress = saturate((t - .60) / .86);
                return 1 - pow(1 - progress,3);
            }
            float3 orbitLines(float2 p, float t)
            {
                float3 light = 0;
                float motion = .95 + .16 * burstExpansion(t);
                for (int k = 0; k < 2; k++)
                {
                    float n = k;
                    float2 q = float2(p.x + p.y * .16,p.y + .06)
                             / float2(.67 + n * .09,.43 + n * .055);
                    float angle = atan2(q.y,q.x);
                    float radius = 1 + .055 * sin(angle * 3 + motion * 2 + n)
                                     + .025 * sin(angle * 7 - motion * 3);
                    float tail = frac((motion * (2.7 + n * .23) + n * 3.141593 - angle)
                                      / 6.283185) * 6.283185;
                    float taper = ramp(0,.12,tail) * (1-ramp(.45,3.4,tail));
                    float d = abs(length(q) - radius);
                    light += (float3(.60,.91,1) * neon(d,.0035) * 2.2
                            + float3(.06,.40,1) * exp(-d / .045) * .32) * taper;
                }
                return light;
            }
            float3 auraWaves(float2 p, float t)
            {
                float2 q = float2(p.x + p.y * .12, p.y + .055) / float2(.68,.46);
                float angle = atan2(q.y,q.x);
                float radius = length(q);
                float expansion = burstExpansion(t);
                float envelope = ramp(.60,.72,t) * (1-ramp(1.08,1.92,t));
                // The mask stretches with the single burst instead of continuously scrolling.
                float2 flow = q / max(.2,radius);
                float2 uv = q * .38 / (.78 + expansion * .32) + .5 - flow * expansion * .055;
                uv += .03 * float2(sin(q.y * 5 + expansion * .2),sin(q.x * 4 - expansion * .2));
                float primary = SAMPLE_TEXTURE2D(_AuraTex,sampler_AuraTex,uv).r;
                float secondary = SAMPLE_TEXTURE2D(_AuraTex,sampler_AuraTex,
                    float2(-uv.y,uv.x) * .83 + float2(.31,.045)).r;
                float maskValue = saturate(primary * .78 + secondary * .22);
                float material = lerp(1,.18 + 1.65 * ramp(.045,.65,maskValue),_AuraDetail);
                float crestDetail = lerp(1,.18 + 1.8 * ramp(.18,.72,maskValue),_AuraDetail);
                float fold = .085 * sin(angle * 3 - expansion * .20)
                           + .04 * sin(angle * 5 + expansion * .15)
                           + (maskValue - .35) * .12 * _AuraDetail;
                float d = radius - (.55 + expansion * .72 + fold);
                float width = lerp(.34,.10,ramp(-.035,.06,d));
                float sheet = exp(-pow(d / width,2));
                float crest = exp(-pow((d + .045) / .11,2));
                float haze = exp(-pow(d / .42,2));
                float folds = .58 + .42 * pow(.5 + .5 * sin(angle * 4 - expansion * .20),2);
                float gaps = .55 + .45 * ramp(-.65,.8,sin(angle * 2 + .8));
                return (float3(.025,.30,.75) * sheet * 1.4 * material
                      + float3(.28,.72,1) * crest * 1.05 * folds * crestDetail
                      + float3(.015,.10,.26) * haze * lerp(1,.35 + maskValue,_AuraDetail))
                      * envelope * gaps;
            }
            half4 portrait(float2 uv)
            {
                half4 c = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, saturate(uv));
                c.a *= step(0, uv.x) * step(uv.x, 1) * step(0, uv.y) * step(uv.y, 1);
                return c;
            }
            half4 frag(V i) : SV_Target
            {
                if (_Age <= 0 || _Age >= _Duration) return 0;
                float t = _Age / _Duration * 2.05;
                float2 p = (i.uv - .5) * float2(_Aspect, 1);
                float reveal = 1 - pow(1 - saturate(t / .60), 3);
                float settle = sin(saturate((t - .88) / .55) * 3.141593) * .022;
                float artH = lerp(1.28, 1.35, ramp(0,1.73,t)) + settle;
                // Start the whole illustration outside the left edge at every aspect ratio.
                float startX = -_Aspect * .5 - artH * _PortraitAspect * .5 - .03;
                float2 centre = float2(lerp(startX, 0, reveal), -.075);
                float2 artUV = (p - centre) / float2(artH * _PortraitAspect, artH) + .5;

                // Dark diagonal panels behind the illustration; no scene lights are changed.
                float stripe = p.y - p.x * .22;
                float3 color = float3(.004,.010,.017);
                color += float3(.008,.016,.024) * exp(-abs(stripe) * 5);
                color *= .72 + .28 * saturate(1 - length(p * float2(.5, 1)));
                float coverage = .96;
                color *= coverage;
                float3 water = float3(.16,.64,1);
                float pulse = exp(-pow((t - .72) / .10, 2));
                // One expanding light wave stays mostly behind the opaque illustration.
                float auraEnvelope = ramp(.30,.66,t) * (1-ramp(1.66,2.02,t));
                float3 waves = auraWaves(p,t) * auraEnvelope;
                float3 orbits = orbitLines(p,t) * auraEnvelope;
                float2 shieldUV = p / (1.38 + .09 * ramp(.88,1.65,t));
                float sigil = neon(shield(shieldUV),.0018)
                            + .45 * neon(shield(shieldUV / .86) * .86,.0012);
                float3 shieldLight = float3(.40,.80,1) * sigil
                                   * ramp(.58,.90,t) * (1-ramp(1.66,2.02,t))
                                   * (.85 + pulse * .55);
                color = 1 - (1 - color) * exp(-waves - orbits - shieldLight);
                half4 art = portrait(artUV);
                float artIn = ramp(.065,.19,t);
                color = lerp(color, art.rgb, art.a * artIn);
                coverage = lerp(coverage, 1, art.a * artIn);

                // Alpha-edge light traces the existing art, without drawing over its interior.
                float2 texel = float2(.0015,.0015);
                float rimA = max(max(portrait(artUV + float2(texel.x,0)).a,
                                    portrait(artUV - float2(texel.x,0)).a),
                                 max(portrait(artUV + float2(0,texel.y)).a,
                                     portrait(artUV - float2(0,texel.y)).a));
                float rim = max(0,rimA - art.a) * reveal * auraEnvelope;
                // A translucent lower wash crosses the art, leaving face and chest untouched.
                float3 light = waves * .32 * (1-ramp(-.25,-.055,p.y))
                             + orbits * .70 * (1-ramp(-.18,-.035,p.y))
                             + float3(.38,.80,1) * rim * (1.1 + pulse * .5)
                             + float3(.45,.8,1) * pulse * .035;
                color = 1 - (1 - saturate(color)) * exp(-light);

                // Diagonal entry and exit have a narrow luminous leading edge, not a whiteout.
                float u = i.uv.x + i.uv.y * .18;
                float front = lerp(-.08,1.3,ramp(0,.16,t));
                float exitFront = lerp(-.08,1.3,ramp(1.73,2.05,t));
                float alpha = (1-ramp(front-.018,front+.018,u)) * ramp(exitFront-.02,exitFront+.02,u);
                float edge = exp(-abs(u-front)*180) * (1-ramp(.14,.17,t));
                edge += exp(-abs(u-exitFront)*150) * ramp(1.73,1.80,t);
                color = 1 - (1 - color) * exp(-water * edge * 3);
                return half4(color / coverage, alpha * coverage);
            }
            ENDHLSL
        }
    }
}
