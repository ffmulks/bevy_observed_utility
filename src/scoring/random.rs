use std::ops::RangeBounds;

use bevy::{
    ecs::{
        component::StorageType,
        lifecycle::{ComponentHook, HookContext},
        world::DeferredWorld,
    },
    prelude::*,
};
use rand::Rng;

use crate::{
    ecs::DeferredWorldExt,
    event::OnScore,
    scoring::{Score, ScoreRange},
};

/// [`Score`] [`Component`] that scores a random value within a range.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_observed_utility::prelude::*;
/// use rand::{prelude::StdRng, SeedableRng};
///
/// # let mut app = App::new();
/// # app.add_plugins(ObservedUtilityPlugins::RealTime);
/// # let mut world = app.world_mut();
/// # let mut commands = world.commands();
/// # let scorer =
/// commands
///     .spawn((RandomScore::new(StdRng::from_os_rng()), Score::default()))
/// #   .id();
/// # commands.trigger(RunScoring::entity(scorer));
/// # world.flush();
/// ```
pub struct RandomScore {
    /// The random number generator to use.
    pub rng: Box<dyn Rng + Send + Sync + 'static>,
    /// The range of scores to generate.
    pub range: ScoreRange,
}

impl RandomScore {
    /// Creates a new [`RandomScore`] with the given random number generator.
    #[must_use]
    pub fn new(rng: impl Rng + Send + Sync + 'static) -> Self {
        Self {
            rng: Box::new(rng),
            range: ScoreRange::FULL,
        }
    }

    /// Creates a new [`RandomScore`] with the given random number generator and score range.
    #[must_use]
    pub fn with_range(rng: impl Rng + Send + Sync + 'static, range: impl RangeBounds<Score>) -> Self {
        Self {
            rng: Box::new(rng),
            range: ScoreRange::from_bounds(range),
        }
    }

    /// Returns a mutable reference to the random number generator.
    #[must_use]
    pub fn rng_mut(&mut self) -> &mut (impl Rng + Send + Sync + 'static) {
        &mut self.rng
    }

    /// Sets the random number generator.
    pub fn set_rng(&mut self, rng: impl Rng + Send + Sync + 'static) {
        self.rng = Box::new(rng);
    }

    fn observer(trigger: On<OnScore>, mut target: Query<(&mut Score, &mut RandomScore)>) {
        let entity = trigger.event().entity;
        let Ok((mut actor_score, mut settings)) = target.get_mut(entity) else {
            // The entity is not scoring for random.
            return;
        };

        // TODO: We're assuming the range is inclusive, but it might not be.
        let range = settings.range.min_f32()..=settings.range.max_f32();
        let value = settings.rng_mut().random_range(range);

        actor_score.set(value);
    }
}

impl Component for RandomScore {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = bevy::ecs::component::Mutable;

    fn on_add() -> Option<ComponentHook> {
        Some(|mut world: DeferredWorld, _context: HookContext| {
            #[derive(Resource, Default)]
            struct RandomScoreObserverSpawned;

            world.once::<RandomScoreObserverSpawned>().observe(Self::observer);
        })
    }
}
