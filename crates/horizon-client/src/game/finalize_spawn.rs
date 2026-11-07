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
    /// The local difficulty at `at`, then the mods' say on the spawn
    /// (`native_finalize_spawn`). `spawner` is the spawner block for a
    /// `SPAWNER` or `TRIAL_SPAWNER` spawn. `reason` is `None` where vanilla
    /// does not call `finalizeSpawn`: no mod is asked there. API
    /// @API:finalize_spawn@.
    pub(super) fn finalize_spawn_event(
        &self,
        kind: &str,
        at: [f64; 3],
        reason: Option<Reason>,
        spawner: Option<[i32; 3]>,
    ) -> SpawnFinalize {
        let computed = self.local_difficulty(at);
        let Some(reason) = reason else {
            return SpawnFinalize {
                difficulty: computed,
                skip: false,
                cancelled: false,
            };
        };
        let (mut difficulty, mut effective) = (computed.base, computed.effective);
        let (mut cancelled, mut skip) = (false, false);
        self.native_finalize_spawn(
            self.dim.id(),
            kind,
            at,
            reason.name(),
            spawner,
            &mut difficulty,
            &mut effective,
            &mut cancelled,
            &mut skip,
        );
        SpawnFinalize {
            difficulty: spawn_difficulty(computed, difficulty, effective),
            skip,
            cancelled,
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

/// The local difficulty the finalize uses: `computed`, or rebuilt from what a
/// mod left when it changed the base or the effective value (`base` and `id`
/// are the id, `normal` and `hard` follow it).
fn spawn_difficulty(computed: LocalDifficulty, difficulty: u8, effective: f32) -> LocalDifficulty {
    if difficulty == computed.base && effective == computed.effective {
        return computed;
    }
    LocalDifficulty {
        base: difficulty,
        effective,
        normal: difficulty == 2,
        hard: difficulty == 3,
        id: difficulty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mods_difficulty_rebuilds_the_local_difficulty() {
        let computed = LocalDifficulty::new(1, 0, 0, 1.0);
        let untouched = spawn_difficulty(computed, computed.base, computed.effective);
        assert_eq!(untouched.base, computed.base);
        assert_eq!(untouched.effective, computed.effective);
        // Hard, with the mod's own effective value.
        let hard = spawn_difficulty(computed, 3, 4.5);
        assert_eq!((hard.base, hard.id), (3, 3));
        assert_eq!(hard.effective, 4.5);
        assert!(hard.hard && !hard.normal);
        // Normal and easy keep the flags in step with the id.
        let normal = spawn_difficulty(computed, 2, 1.0);
        assert!(normal.normal && !normal.hard);
        let easy = spawn_difficulty(computed, 1, 0.75);
        assert!(!easy.normal && !easy.hard);
        assert_eq!((easy.base, easy.id, easy.effective), (1, 1, 0.75));
    }
}
