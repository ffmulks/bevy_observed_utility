use bevy::{
    ecs::{
        component::StorageType,
        lifecycle::{ComponentHook, HookContext},
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::{ecs::DeferredWorldExt, event::OnScore, scoring::Score};

/// [`Score`] [`Component`] that always scores a fixed value.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_observed_utility::prelude::*;
///
/// # let mut app = App::new();
/// # app.add_plugins(ObservedUtilityPlugins::RealTime);
/// # let mut world = app.world_mut();
/// # let mut commands = world.commands();
/// # let scorer =
/// commands
///     .spawn((FixedScore::new(0.5), Score::default()))
/// #   .id();
/// # commands.trigger(RunScoring::entity(scorer));
/// # world.flush();
/// # assert_eq!(world.get::<Score>(scorer).unwrap().get(), 0.5);
/// ```
#[derive(Reflect, Clone, Copy, PartialEq, Debug, Default)]
#[reflect(Component, PartialEq, Debug, Default)]
pub struct FixedScore {
    /// The fixed value to score.
    value: Score,
}

impl FixedScore {
    /// Creates a new [`FixedScore`] with the given value.
    #[must_use]
    pub fn new(value: impl Into<Score>) -> Self {
        Self { value: value.into() }
    }

    /// Returns the fixed value to score.
    #[must_use]
    pub fn value(&self) -> Score {
        self.value
    }

    /// Sets the fixed value to score.
    pub fn set_value(&mut self, value: impl Into<Score>) {
        self.value = value.into();
    }

    /// [`Observer`] for [`FixedScore`] [`Score`] entities that scores itself.
    fn observer(trigger: On<OnScore>, mut target: Query<(&mut Score, &FixedScore)>) {
        let entity = trigger.event().entity;
        let Ok((mut actor_score, settings)) = target.get_mut(entity) else {
            // The entity is not scoring for fixed.
            return;
        };

        *actor_score = settings.value();
    }
}

impl Component for FixedScore {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = bevy::ecs::component::Immutable;

    fn on_add() -> Option<ComponentHook> {
        Some(|mut world: DeferredWorld, _context: HookContext| {
            #[derive(Resource, Default)]
            struct FixedScoreObserverSpawned;

            world.once::<FixedScoreObserverSpawned>().observe(Self::observer);
        })
    }
}
