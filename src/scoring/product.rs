use bevy::{
    ecs::{
        component::StorageType,
        lifecycle::{ComponentHook, HookContext},
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::{ecs::DeferredWorldExt, event::OnScore, scoring::Score};

/// [`Score`] [`Component`] that scores the product of all child [`Score`] entities.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_observed_utility::prelude::*;
/// # use approx::assert_relative_eq;
///
/// # let mut app = App::new();
/// # app.add_plugins(ObservedUtilityPlugins::RealTime);
/// # let mut world = app.world_mut();
/// # let mut commands = world.commands();
/// # let scorer =
/// commands
///     .spawn((Product::new(0.1), Score::default()))
///     .with_children(|parent| {
///         parent.spawn((FixedScore::new(0.7), Score::default()));
///         parent.spawn((FixedScore::new(0.3), Score::default()));
///     })
/// #   .id();
/// # commands.trigger(RunScoring::entity(scorer));
/// # world.flush();
/// # assert_relative_eq!(world.get::<Score>(scorer).unwrap().get(), 0.21);
/// ```
#[derive(Reflect, Clone, Copy, PartialEq, Debug, Default)]
#[reflect(Component, PartialEq, Debug, Default)]
pub struct Product {
    /// The threshold for the product of child scores to be considered a success.
    threshold: Score,
    /// Whether to use compensation to prevent the product from being too low.
    use_compensation: bool,
}

impl Product {
    /// Creates a new [`Product`] with the given threshold.
    #[must_use]
    pub fn new(threshold: impl Into<Score>) -> Self {
        Self {
            threshold: threshold.into(),
            use_compensation: false,
        }
    }

    /// Sets whether to use compensation to prevent the product from being too low.
    #[must_use]
    pub fn with_compensation(mut self, compensation: bool) -> Self {
        self.use_compensation = compensation;
        self
    }

    /// Returns the threshold for the product of child scores to be considered a success.
    #[must_use]
    pub fn threshold(&self) -> Score {
        self.threshold
    }

    /// Sets the threshold for the product of child scores to be considered a success.
    pub fn set_threshold(&mut self, threshold: impl Into<Score>) {
        self.threshold = threshold.into();
    }

    /// [`Observer`] for [`Product`] [`Score`] entities that scores based on all child [`Score`] entities.
    fn observer(trigger: On<OnScore>, target: Query<(&Children, &Product)>, mut scores: Query<&mut Score>) {
        let entity = trigger.event().entity;
        let Ok((children, settings)) = target.get(entity) else {
            // The entity is not scoring for product.
            return;
        };

        let mut product: f32 = 1.;
        let mut num_scores = 0;

        for child_score in scores.iter_many(children) {
            product *= child_score.get();
            num_scores += 1;
        }

        if settings.use_compensation && num_scores > 0 {
            let mod_factor = 1. - 1. / (num_scores as f32);
            let makeup = (1. - product) * mod_factor;
            product += makeup * product;
        }

        if product < settings.threshold().get() {
            product = 0.;
        }

        let Ok(mut actor_score) = scores.get_mut(entity) else {
            // The entity is not scoring.
            return;
        };

        actor_score.set(product);
    }
}

impl Component for Product {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = bevy::ecs::component::Immutable;

    fn on_add() -> Option<ComponentHook> {
        Some(|mut world: DeferredWorld, _context: HookContext| {
            #[derive(Resource, Default)]
            struct ProductObserverSpawned;

            world.once::<ProductObserverSpawned>().observe(Self::observer);
        })
    }
}
