// The acolyte cut-in's direction, copied value for value from godot/views/cutin/cutin_acolyte.tscn
// (the "cutin" AnimationPlayer clip + the node/material params). Frame is 1600x900, art 3000x2863,
// times in seconds, Godot pixel space (y down, origin top-left).
using UnityEngine;

namespace Roi.Cutin
{
    public static class CutinAcolyteClip
    {
        public const float Length = 6f;
        public const float FrameW = 1600f, FrameH = 900f;
        public const float ArtW = 3000f, ArtH = 2863f;
        // Opaque content box of acolyte.png (alpha > 8), top-down px: the rig's mesh bounds cover
        // the drawn figure, not the PNG's transparent margin, so this is what maps onto them.
        public const float ContentX0 = 48f, ContentY0 = 83f, ContentX1 = 2932f, ContentY1 = 2802f;

        // Art:offset_left/top/right/bottom — cubic interp, held between beats (legs / chest / whole).
        // Offsets from a full-rect anchor: rect = (left, top, FrameW + right, FrameH + bottom).
        public static readonly float[] RectT = { 0f, 1.2f, 2.4f, 3.2f, 3.9f };
        public static readonly float[] Left = { -4368f, -4368f, -4994f, -4994f, -718f };
        public static readonly float[] Top = { -2912f, -2912f, -2324f, -2324f, -480f };
        public static readonly float[] Right = { 3588f, 3588f, 3801f, 3801f, 702f };
        public static readonly float[] Bottom = { 2957f, 2957f, 4019f, 4019f, 955f };

        // Settle after the last framing key (Unity-side, user 2026-09-06: the pull-back must not
        // stop dead, and must not snap). A damped oscillation about the final frame rather than an
        // extra keyframe: an added key inherits the huge incoming velocity of the 3.2 -> 3.9 move
        // and whips, while this starts at zero offset, drifts out, and eases back over seconds.
        // Multiplies Art:scale, so it zooms about the frame centre exactly as that track does.
        public const float SettleFrom = 3.9f, SettleAmp = 0.05f, SettleFreq = 0.3f, SettleDecay = 0.55f;

        public static float Settle(float t)
        {
            var u = t - SettleFrom;
            if (u <= 0f) return 1f;
            return 1f + SettleAmp * Mathf.Exp(-SettleDecay * u) * Mathf.Sin(2f * Mathf.PI * SettleFreq * u);
        }
        // Art:scale about pivot (800,450) — the slow continued drift.
        public static readonly float[] ArtScaleT = { 0f, 5.5f };
        public static readonly float[] ArtScale = { 1f, 1.05f };
        // Art:modulate alpha — its last key is what ends the cut-in.
        public static readonly float[] ArtAlphaT = { 0f, 0.4f, 5.5f, 6f };
        public static readonly float[] ArtAlpha = { 0f, 1f, 1f, 0f };

        public static readonly float[] StarsAlphaT = { 0f, 0.6f, 5.6f, 6f };
        public static readonly float[] StarsAlpha = { 0f, 1f, 1f, 0f };
        public const float StarBurstAt = 3.6f;
        public const float ShimmerAt = 3.65f;
        public static readonly float[] ShimmerAlphaT = { 3.6f, 4f, 5.4f, 6f };
        public static readonly float[] ShimmerAlpha = { 0f, 1f, 1f, 0f };

        public static readonly float[] RaysAlphaT = { 0f, 0.8f, 4.6f, 6f };
        public static readonly float[] RaysAlpha = { 0f, 1f, 1f, 0f };
        public static readonly float[] RaysAngleT = { 0f, 6f };
        public static readonly float[] RaysAngle = { -0.5f, -0.1f };
        public static readonly float[] RaysColorT = { 0f, 3.4f, 3.75f, 4.6f };
        public static readonly Color[] RaysColor =
        {
            new Color(1f, 0.9f, 0.62f, 0.3f), new Color(1f, 0.9f, 0.62f, 0.3f),
            new Color(1f, 0.95f, 0.72f, 0.65f), new Color(1f, 0.9f, 0.64f, 0.18f),
        };
        // Mat_gr statics.
        public const float RayPosition = -0.2f, Spread = 0.55f, Cutoff = 0.1f, Falloff = 0.3f, EdgeFade = 0.15f;
        public const float RaySpeed = 1f, Ray1Density = 8f, Ray2Density = 30f, Ray2Intensity = 0.3f, RaySeed = 5f;

        // Bloom:material strength (Mat_bloom threshold 0.5, warm tint), toned DOWN for Unity (user,
        // 2026-09-06: "ฟุ้งน้อยลง จะได้เห็นสี sat เต็มๆ"). Godot's bloom was a screen-blur ColorRect
        // over an already-flat SubViewport; URP's runs on the real HDR-ish frame and the white robes
        // sat far above a 0.5 threshold, so the whole centre washed out. Threshold up to just under
        // white, strength halved, scatter tight so the glow hugs the highlight instead of veiling.
        public static readonly float[] BloomT = { 0f, 3.2f, 3.8f };
        public static readonly float[] Bloom = { 0.3f, 0.3f, 0.06f };
        public const float BloomThreshold = 0.95f;
        public const float BloomScatter = 0.5f;
        public static readonly Color BloomTint = new Color(1f, 0.95f, 0.82f);
        // Mat_vig: strength 0.55, clear until inner 0.42 of the half-diagonal.
        public const float VignetteStrength = 0.55f, VignetteInner = 0.42f;

        // cutin_view.gd: black SubViewport under a (0.02, 0.02, 0.04, 0.72) dim veil.
        public static readonly Color Backdrop = new Color(0.02f * 0.72f, 0.02f * 0.72f, 0.04f * 0.72f, 1f);

        // `smooth` = Godot INTERPOLATION_CUBIC (interp=2 in the tscn): Math::cubic_interpolate_in_time,
        // a Catmull-Rom through the neighbouring keys, time-aware. That is what gives the held
        // beats their ease-out/ease-in instead of a hard stop at every key. Otherwise linear.
        public static float Eval(float t, float[] times, float[] values, bool smooth)
        {
            if (t <= times[0]) return values[0];
            var last = times.Length - 1;
            if (t >= times[last]) return values[last];
            var i = 0;
            while (t >= times[i + 1]) i++;
            var u = Mathf.InverseLerp(times[i], times[i + 1], t);
            if (!smooth) return Mathf.Lerp(values[i], values[i + 1], u);
            var pre = Mathf.Max(i - 1, 0);
            var post = Mathf.Min(i + 2, last);
            return CubicInTime(values[i], values[i + 1], values[pre], values[post], u,
                times[i + 1] - times[i], times[pre] - times[i], times[post] - times[i]);
        }

        public static Color Eval(float t, float[] times, Color[] values, bool smooth)
        {
            return new Color(
                Eval(t, times, Channel(values, 0), smooth), Eval(t, times, Channel(values, 1), smooth),
                Eval(t, times, Channel(values, 2), smooth), Eval(t, times, Channel(values, 3), smooth));
        }

        static float[] Channel(Color[] c, int ch)
        {
            var r = new float[c.Length];
            for (var i = 0; i < c.Length; i++) r[i] = c[i][ch];
            return r;
        }

        // Godot core/math/math_funcs.h cubic_interpolate_in_time (Barry-Goldman), times relative
        // to `from`: toT > 0, preT <= 0, postT >= toT. UNCLAMPED lerps: the a1/a3 terms extrapolate
        // past the neighbours, which is the anticipation/overshoot around a held key. Mathf.Lerp
        // clamps and flattened every hold.
        static float CubicInTime(float from, float to, float pre, float post, float weight, float toT, float preT, float postT)
        {
            var t = Mathf.LerpUnclamped(0f, toT, weight);
            var a1 = Mathf.LerpUnclamped(pre, from, preT == 0f ? 0f : (t - preT) / (0f - preT));
            var a2 = Mathf.LerpUnclamped(from, to, toT == 0f ? 0.5f : t / toT);
            var a3 = Mathf.LerpUnclamped(to, post, postT - toT == 0f ? 1f : (t - toT) / (postT - toT));
            var b1 = Mathf.LerpUnclamped(a1, a2, toT - preT == 0f ? 0f : (t - preT) / (toT - preT));
            var b2 = Mathf.LerpUnclamped(a2, a3, postT == 0f ? 1f : t / postT);
            return Mathf.LerpUnclamped(b1, b2, toT == 0f ? 0.5f : t / toT);
        }
    }
}
