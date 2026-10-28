//! Runtime construction from `GameData` — instantiate items, populate a
//! zone, build player/party entities, change zones, recruit members.

use super::*;
use crate::grid::{Cell, Entity, Team};
use crate::items::{DropEntry, Item};
use crate::spawn;
use crate::state::{GameState, MonsterSpawnEntry, MonsterTemplate, ZoneSpawnState};

/// Skill points a freshly-built character starts with (Job Level 1), to invest
/// in its class skill tree. Job Levels add +1 each on top. Was 9 (a dev value
/// for quick skill testing); a Job-1 character should start with just 1.

pub(crate) mod items;
pub use items::*;
mod party;
pub use party::*;
mod zone;
pub use zone::*;

/// Reserved id range for monster spawns. Players use 1..1000; this
/// keeps player vs monster spawn ids visually distinct in golden
/// hashes and event logs.
pub const MONSTER_ID_BASE: u32 = 1000;

/// The id the FIRST player character spawns with at new-game; subsequent party
/// members take `FIRST_PLAYER_ID + i`. This is purely the player id-allocation
/// BASE — NOT a "lead" (the party is symmetric, any member can be benched). Also
/// the fallback id when no player exists yet (an empty grid).
pub const FIRST_PLAYER_ID: u32 = 1;
