use bevy::{
    ecs::{
        component::StorageType,
        lifecycle::{ComponentHook, HookContext},
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::{ecs::DeferredWorldExt, event::OnScore, scoring::Score};

/// [`Score`] [`Component`] that scores all-or-nothing based on the sum of its child [`Score`] entities.
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
///     .spawn((AllOrNothing::new(0.5), Score::default()))
///     .with_children(|parent| {
///         parent.spawn((FixedScore::new(0.7), Score::default()));
///         parent.spawn((FixedScore::new(0.3), Score::default()));
///     })
/// #   .id();
/// # commands.trigger(RunScoring::entity(scorer));
/// # world.flush();
/// # assert_eq!(world.get::<Score>(scorer).unwrap().get(), 0.0);
/// ```
#[derive(Reflect, Clone, Copy, PartialEq, Debug, Default)]
#[reflect(Component, PartialEq, Debug, Default)]
pub struct AllOrNothing {
    /// The threshold for the sum of child scores to be considered a success.
    threshold: Score,
}

impl AllOrNothing {
    /// Creates a new [`AllOrNothing`] with the given threshold.
    #[must_use]
    pub fn new(threshold: impl Into<Score>) -> Self {
        Self {
            threshold: threshold.into(),
        }
    }

    /// Returns the threshold for the sum of child scores to be considered a success.
    #[must_use]
    pub fn threshold(&self) -> Score {
        self.threshold
    }

    /// Sets the threshold for the sum of child scores to be considered a success.
    pub fn set_threshold(&mut self, threshold: impl Into<Score>) {
        self.threshold = threshold.into();
    }

    /// [`Observer`] for [`AllOrNothing`] [`Score`] entities that scores based on all child [`Score`] entities.
    fn observer(trigger: On<OnScore>, target: Query<(&Children, &AllOrNothing)>, mut scores: Query<&mut Score>) {
        let entity = trigger.event().entity;
        let Ok((children, settings)) = target.get(entity) else {
            // The entity is not scoring for all-or-nothing.
            return;
        };

        let mut sum: f32 = 0.;

        for child_score in scores.iter_many(children) {
            if *child_score < settings.threshold() {
                sum = 0.;
                break;
            }
            sum += child_score.get();
        }

        let Ok(mut actor_score) = scores.get_mut(entity) else {
            // The entity is not scoring.
            return;
        };

        actor_score.set(sum);
    }
}

impl Component for AllOrNothing {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = bevy::ecs::component::Immutable;

    fn on_add() -> Option<ComponentHook> {
        Some(|mut world: DeferredWorld, _context: HookContext| {
            #[derive(Resource, Default)]
            struct AllOrNothingObserverSpawned;

            world.once::<AllOrNothingObserverSpawned>().observe(Self::observer);
        })
    }
}
