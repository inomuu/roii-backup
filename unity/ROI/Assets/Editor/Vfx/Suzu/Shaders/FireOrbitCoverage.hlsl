#ifndef ROI_FIRE_ORBIT_COVERAGE_INCLUDED
#define ROI_FIRE_ORBIT_COVERAGE_INCLUDED
// Adapt the stationary strip to the vendor UV island before its existing
// texture, noise and dissolve operations. The orbit path is an exact passthrough.
void FireSlashUV_float(float4 UV, float BellStrip, float Head, float Tail, out float4 Out)
{
    Out=UV;
    if (BellStrip>.5)
    {
        float along=saturate((UV.x-Head)/max(Tail-Head,.001));
        Out.xy=float2(lerp(.16,1,along),lerp(.0048,.7985,UV.y));
    }
}
void FireSlashUV_half(half4 UV, half BellStrip, half Head, half Tail, out half4 Out)
{
    float4 value;
    FireSlashUV_float(UV,BellStrip,Head,Tail,value);
    Out=value;
}
void FireSlashWindow_float(float Alpha, float4 UV, float BellStrip, float Head, float Tail, float Fade, out float Out)
{
    Out=Alpha;
    if (BellStrip>.5)
    {
        float aa=max(fwidth(UV.x),.001);
        float window=smoothstep(Head-aa,Head+aa,UV.x)*(1-smoothstep(Tail-aa,Tail+aa,UV.x));
        Out=Alpha*window*Fade;
    }
}
void FireSlashWindow_half(half Alpha, half4 UV, half BellStrip, half Head, half Tail, half Fade, out half Out)
{
    float value;
    FireSlashWindow_float(Alpha,UV,BellStrip,Head,Tail,Fade,value);
    Out=value;
}
// Coverage in the vendor's UV islands, never vertex displacement.
void FireOrbitCoverage_float(float Alpha, float4 UV, float WhiteBand, float Taper, float BellStrip, out float Out)
{
    float u=saturate(UV.x);
    float radial=saturate((UV.y-.0048)/.7937);
    // The raised surface is packed into a second, narrow V island.
    if (UV.y>.815) radial=.78+.13*saturate(1-abs(UV.y-.912)/.087);
    if (BellStrip>.5)
    {
        // The vendor mask erodes the far end before U=1. Start narrowing in
        // the visible body, keeping the outer cutting edge continuous.
        float span=lerp(1,1-smoothstep(.20,.82,u),saturate(Taper));
        float aa=max(fwidth(radial)*.8,.003);
        float inside=radial-(1-span);
        float coverage=smoothstep(-aa,aa,inside);
        float across=inside/max(span,.001);
        float white=smoothstep(.57,.60,across)*(1-smoothstep(.93,.96,across));
        Out=Alpha*coverage*lerp(1,white,saturate(WhiteBand));
        return;
    }
    // Keep the beginning and middle broad; only the final 42% narrows to a tip.
    float width=smoothstep(0,.42,1-u);
    width=lerp(1,width,saturate(Taper));
    float inside=radial-(1-width);
    float aa=max(fwidth(radial)*.8,.003);
    float coverage=smoothstep(-aa,aa,inside);
    float band=inside/max(width,.001);
    float white=smoothstep(.57,.60,band)*(1-smoothstep(.93,.96,band));
    Out=Alpha*coverage*lerp(1,white,saturate(WhiteBand));
}
void FireOrbitCoverage_half(half Alpha, half4 UV, half WhiteBand, half Taper, half BellStrip, out half Out)
{
    float value;
    FireOrbitCoverage_float(Alpha,UV,WhiteBand,Taper,BellStrip,value);
    Out=value;
}
#endif
