//! A mod's say before a mob's `finalizeSpawn`, through a real game and the
//! spawn paths that carry a reason. Installs process-wide trait mods: run this
//! test alone.
use super::*;
use crate::mod_api::{self, FinalizeSpawnEvent, Flow, Hooks, Host};
use horizon_client::{
    chunk::Chunk,
    mobs::finalize::{Attr, Reason, has_modifier},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

#[derive(Default)]
struct Probe {
    /// 0: hear only, 1: skip the finalize, 2: refuse the spawn.
    mode: AtomicU8,
    calls: Mutex<Vec<(String, String)>>,
}

impl Hooks for Arc<Probe> {
    fn finalize_spawn(&self, _: &dyn Host, event: &mut FinalizeSpawnEvent<'_>) -> Flow {
        self.calls
            .lock()
            .unwrap()
            .push((event.kind.into(), event.reason.into()));
        match self.mode.load(Ordering::Relaxed) {
            1 => Flow::Cancel,
            2 => {
                event.spawn_cancelled = true;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }
}

#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES and an isolated mod registry"]
fn a_mod_skips_or_refuses_a_spawn_and_a_no_finalize_path_never_asks() {
    let root = PathBuf::from(std::env::var_os("HORIZON_TEST_RESOURCES").unwrap());
    let settings = Settings::default();
    let loaded = load_resources(
        root.clone(),
        None,
        &root,
        &settings,
        Instant::now(),
        false,
        true,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = execution_role::ExecutionRole::IntegratedServer;
    let probe = Arc::new(Probe::default());
    assert!(
        mod_api::MODS
            .set(Arc::from(vec![Box::new(probe.clone()) as Box<dyn Hooks>]))
            .is_ok()
    );
    let mut game = Game::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&root),
        None,
    )
    .unwrap();
    game.world = World::new(game.res.facts().air, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let at = [0.5, 64.0, 0.5];
    let follows = |game: &Game<'_>, id: u32| {
        let m = game.mobs.iter().find(|m| m.id == id).unwrap();
        has_modifier(m, Attr::FollowRange, "minecraft:random_spawn_bonus")
    };

    // A mod that skips the finalize: the mob still spawns, without what
    // `Mob.finalizeSpawn` always does.
    probe.mode.store(1, Ordering::Relaxed);
    let id = game
        .spawn_mob_spawned_reason("minecraft:pig", at, Some(Reason::SpawnItemUse))
        .expect("a skipped finalize still spawns the pig");
    assert!(!follows(&game, id), "the follow range bonus was skipped");
    assert_eq!(
        *probe.calls.lock().unwrap(),
        [("minecraft:pig".to_string(), "SPAWN_ITEM_USE".to_string())]
    );

    // A mod that refuses the spawn: no mob at all.
    game.mobs.clear();
    probe.mode.store(2, Ordering::Relaxed);
    assert!(
        game.spawn_mob_spawned_reason("minecraft:pig", at, Some(Reason::SpawnItemUse))
            .is_none()
    );
    assert!(game.mobs.is_empty(), "a cancelled spawn adds no mob");

    // A path vanilla does not finalize (a thrown egg's chicken) is not asked
    // and still gets its finalize, as it did before the hook.
    probe.mode.store(0, Ordering::Relaxed);
    probe.calls.lock().unwrap().clear();
    let id = game
        .spawn_mob("minecraft:chicken", at, true)
        .expect("the chicken spawns");
    assert!(follows(&game, id), "the game still finalizes it");
    assert!(
        probe.calls.lock().unwrap().is_empty(),
        "no reason, no event"
    );
}
