//! GOLDEN ROSTER A — the launch five at S3: Seraphine (Iron Chains DEF, Aegis), Rapis, Lucielle
//! (Benedictio, Communio), Aveline (Umbral Mark), Nyx. Harness + why: `roster_common.rs`.
//! Re-pin with `tools/bless_goldens.sh golden_roster_a` and say why in `lib.rs`'s changelog.

use super::roster_common::{game_data, report, stand_up};
use sim_core::replay::advance;

#[test]
fn golden_roster_launch_five_s3() {
    let d = game_data();
    let mut s = stand_up(42, &d, &["lapis", "lucielle", "aveline", "raplus"], 3);
    advance(&mut s, &d, 3_000);
    let h = report("GOLDEN_ROSTER_A", &s);
    assert_eq!(h, "fb2c2e99053fd8f84a4d9433960ec107f4b346c47ce65a0273315d3896d46e97");
}
