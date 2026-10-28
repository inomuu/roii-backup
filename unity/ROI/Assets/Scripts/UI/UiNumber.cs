using System;
using System.Globalization;

namespace Roi.UI
{
    /// Number formatting for UI only. Gameplay values stay integer and are never scaled here.
    public static class UiNumber
    {
        static readonly string[] Suffixes = { "K", "M", "B", "T", "Qa", "Qi" };

        public static string Exact(ulong value)
            => value.ToString("N0", CultureInfo.InvariantCulture);

        public static string Compact(ulong value)
        {
            if (value < 1000) return Exact(value);
            decimal scaled = value;
            int suffix = -1;
            while (scaled >= 1000m && suffix + 1 < Suffixes.Length)
            {
                scaled /= 1000m;
                suffix++;
            }
            if (scaled >= 1000m)
                return Exact(value);

            // Truncate rather than round. A compact affordability label must never imply
            // more currency than the exact integer contains.
            scaled = decimal.Truncate(scaled * 100m) / 100m;
            return scaled.ToString("0.##", CultureInfo.InvariantCulture) + Suffixes[suffix];
        }

        public static string Signed(long value)
        {
            if (value >= 0) return "+" + ((ulong)value).ToString("N0", CultureInfo.InvariantCulture);
            if (value == long.MinValue) return "-9,223,372,036,854,775,808";
            return "-" + ((ulong)(-value)).ToString("N0", CultureInfo.InvariantCulture);
        }

        public static bool IsWithin(ulong balance, ulong cost) => balance >= cost;
    }
}
