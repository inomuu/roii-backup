//! GOLDEN ROSTER B — the 0.0.451-456 additions at S6: Nemu (Sweet Poison, Heartstring leech,
//! the black hole), Lumi (Frost Mark, Frost Aura, Hoarfrost Crown's ult heal) beside Seraphine,
//! Lucielle, Nyx. Harness + why: `roster_common.rs`.
//! Re-pin with `tools/bless_goldens.sh golden_roster_b` and say why in `lib.rs`'s changelog.

use super::roster_common::{game_data, report, stand_up};
use sim_core::replay::advance;

#[test]
fn golden_roster_nemu_lumi_s6() {
    let d = game_data();
    let mut s = stand_up(7, &d, &["nemu", "lumi", "lucielle", "raplus"], 6);
    advance(&mut s, &d, 3_000);
    let h = report("GOLDEN_ROSTER_B", &s);
    assert_eq!(h, "70176e4f17818f2f0ec504ed0b902e1ca8c79ec5c0fdf6172e5090f74e90024f");
}
