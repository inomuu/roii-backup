using System.Globalization;
using System.Text.RegularExpressions;

namespace Roi.UI
{
    public static class CrestPresentation
    {
        public struct BonusView { public string Label,Current,Next; }
        static readonly Regex Value=new(@"^([+-]?\d+(?:\.\d+)?)(%?)\s+(.+)$",RegexOptions.CultureInvariant);
        // Display only: the host publishes a linear per-level line and the current total.
        // No price, eligibility or applied stat is derived here. Unrecognized formats fall back
        // to the host text instead of inventing a value. Decimal preserves 0.1% / 0.3% exactly.
        public static BonusView Bonus(CrestModel.Slot slot)
        {
            if(slot==null)return new BonusView{Label="",Current="—",Next="—"};
            var step=Value.Match(slot.text_per_level??"");
            var current=Value.Match(slot.text??"");
            if(!step.Success||!decimal.TryParse(step.Groups[1].Value,NumberStyles.AllowLeadingSign|NumberStyles.AllowDecimalPoint,CultureInfo.InvariantCulture,out var amount))
                return new BonusView{Label=slot.text_per_level??"",Current=slot.text??"—",Next="—"};
            string suffix=step.Groups[2].Value;
            return new BonusView{
                Label=step.Groups[3].Value,
                Current=current.Success?current.Groups[1].Value+current.Groups[2].Value:slot.level==0?"0"+suffix:slot.text??"—",
                Next=slot.level>=slot.max_level?"—":(amount>=0?"+":"")+(amount*(slot.level+1)).ToString("0.##",CultureInfo.InvariantCulture)+suffix
            };
        }
    }
}
