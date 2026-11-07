//! One funnel for every runtime `Mob.finalizeSpawn`.
//!
//! Each spawn path asks [`Game::finalize_spawn_event`] first, with the reason
//! vanilla gives the spawn, and gets back the local difficulty the finalize
//! uses. A `reason` of `None` marks a path where vanilla does not call
//! `finalizeSpawn`: the game still finalizes the mob there as it did before,
//! and no mod is asked.
//!
//! The funnel also says whether a mod skipped vanilla's `finalizeSpawn` and
//! whether it refused the spawn: a skipped spawn still adds the mob, without
//! the steps `finalizeSpawn` runs, and a cancelled one does not add it at all.
use super::Game;
use horizon_client::mobs::{
    Mob,
    finalize::Reason,
    spawning::{self, LocalDifficulty},
};

/// Where a runtime spawn's `finalizeSpawn` starts.
pub(super) struct SpawnFinalize {
    /// The local difficulty at the spawn position, 0 peaceful to 3 hard.
    pub difficulty: LocalDifficulty,
    /// A mod skipped vanilla's `finalizeSpawn`: the mob still spawns.
    pub skip: bool,
    /// A mod refused the spawn: the mob is not added to the level.
    pub cancelled: bool,
}

impl Game<'_> {
    /// The local difficulty at `at`, and from the hook commit the mods' say on
    /// the spawn (`native_finalize_spawn`). `spawner` is the spawner block for
    /// a `SPAWNER` or `TRIAL_SPAWNER` spawn. `reason` is `None` where vanilla
    /// does not call `finalizeSpawn`: no mod is asked there.
    pub(super) fn finalize_spawn_event(
        &self,
        kind: &str,
        at: [f64; 3],
        reason: Option<Reason>,
        spawner: Option<[i32; 3]>,
    ) -> SpawnFinalize {
        let difficulty = self.local_difficulty(at);
        let _ = (kind, reason, spawner);
        SpawnFinalize {
            difficulty,
            skip: false,
            cancelled: false,
        }
    }

    /// The common case: the event, then the biome facts and vanilla's
    /// `Mob.finalizeSpawn` unless a mod skipped it. False when a mod refused
    /// the spawn: the mob must not be added.
    pub(super) fn finalize_mob(
        &self,
        mob: &mut Mob,
        reason: Option<Reason>,
        spawner: Option<[i32; 3]>,
    ) -> bool {
        let at = [mob.position.x, mob.position.y, mob.position.z];
        let kind = super::native_hooks::mob_kind(mob);
        let event = self.finalize_spawn_event(&kind, at, reason, spawner);
        if event.cancelled {
            return false;
        }
        if event.skip {
            return true;
        }
        let facts = self.biome_facts(
            at[0].floor() as i32,
            at[1].floor() as i32,
            at[2].floor() as i32,
        );
        spawning::finalize(mob, &facts, &event.difficulty);
        true
    }
}
