//! Comprehensive integration tests for bevy_observed_utility
//! Tests the complete observer and trigger architecture after Bevy 0.17 migration

use approx::assert_relative_eq;
use bevy::{ecs::component::ComponentId, prelude::*};
use bevy_observed_utility::prelude::*;

/// Test that the basic scoring lifecycle works with the new trigger architecture
#[test]
fn test_scoring_lifecycle() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    let world = app.world_mut();

    // Create a simple fixed score entity
    let mut commands = world.commands();
    let scorer = commands.spawn((Score::default(), FixedScore::new(0.75))).id();
    world.flush();

    // Trigger scoring
    world.commands().trigger(RunScoring::entity(scorer));
    world.flush();

    // Verify the score was calculated correctly
    assert_eq!(
        0.75,
        world.get::<Score>(scorer).unwrap().get(),
        "Score should be set to 0.75 via the observer"
    );
}

/// Test that scoring works for all entities when using RunScoring::all()
#[test]
fn test_scoring_all_entities() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    let world = app.world_mut();

    // Create multiple score entities
    let mut commands = world.commands();
    let scorer1 = commands.spawn((Score::default(), FixedScore::new(0.5))).id();
    let scorer2 = commands.spawn((Score::default(), FixedScore::new(0.8))).id();
    let scorer3 = commands.spawn((Score::default(), FixedScore::new(0.3))).id();
    world.flush();

    // Trigger scoring for all
    world.commands().trigger(RunScoring::all());
    world.flush();

    // Verify all scores were calculated
    assert_eq!(0.5, world.get::<Score>(scorer1).unwrap().get());
    assert_eq!(0.8, world.get::<Score>(scorer2).unwrap().get());
    assert_eq!(0.3, world.get::<Score>(scorer3).unwrap().get());
}

/// Test hierarchical scoring with parent-child relationships
#[test]
fn test_hierarchical_scoring() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    let world = app.world_mut();

    let mut commands = world.commands();
    let parent = commands
        .spawn((Score::default(), Sum::default()))
        .with_children(|parent| {
            parent.spawn((Score::default(), FixedScore::new(0.3)));
            parent.spawn((Score::default(), FixedScore::new(0.4)));
        })
        .id();
    world.flush();

    // Trigger scoring on the parent (should score children first due to post-order traversal)
    world.commands().trigger(RunScoring::entity(parent));
    world.flush();

    // Parent should have the sum of children
    assert_relative_eq!(
        0.7,
        world.get::<Score>(parent).unwrap().get(),
        epsilon = 0.0001
    );
}

/// Test the picking lifecycle with FirstToScore strategy
#[test]
fn test_picking_first_to_score() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    let world = app.world_mut();

    let my_action = world.register_component::<MyAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let scorer = commands.spawn((Score::new(0.6), MyMarker)).id();

    let actor = commands
        .spawn((
            Picker::new(idle_action).with(scorer, my_action),
            FirstToScore::new(0.5),
        ))
        .add_child(scorer)
        .id();
    world.flush();

    // Run picking
    world.commands().trigger(RunPicking::entity(actor));
    world.flush();

    // Since score (0.6) is above threshold (0.5), my_action should be picked
    assert_eq!(my_action, world.get::<Picker>(actor).unwrap().picked);
}

/// Test the picking lifecycle with Highest strategy
#[test]
fn test_picking_highest() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    let world = app.world_mut();

    let action1 = world.register_component::<Action1>();
    let action2 = world.register_component::<Action2>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let scorer1 = commands.spawn((Score::new(0.3), MyMarker)).id();
    let scorer2 = commands.spawn((Score::new(0.8), MyMarker)).id();

    let actor = commands
        .spawn((
            Picker::new(idle_action).with(scorer1, action1).with(scorer2, action2),
            Highest,
        ))
        .add_children(&[scorer1, scorer2])
        .id();
    world.flush();

    // Run picking
    world.commands().trigger(RunPicking::entity(actor));
    world.flush();

    // action2 should be picked since scorer2 has the highest score (0.8)
    assert_eq!(action2, world.get::<Picker>(actor).unwrap().picked);
}

/// Test action lifecycle: initiation, execution, and completion
#[test]
fn test_action_lifecycle() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    // Add observer to handle action initiation
    app.add_observer(on_action_initiated_insert_default::<TestAction>);
    app.add_observer(on_action_ended_remove::<TestAction>);

    let world = app.world_mut();

    let action_id = world.register_component::<TestAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let actor = commands.spawn((Picker::new(idle_action), CurrentAction(idle_action))).id();
    world.flush();

    // Request the action
    world.commands().trigger(RequestAction::specific(actor, action_id));
    world.flush();

    // Verify action was initiated and component was inserted
    assert!(
        world.get::<TestAction>(actor).is_some(),
        "TestAction component should be inserted on actor"
    );
    assert_eq!(
        action_id,
        world.get::<CurrentAction>(actor).unwrap().0,
        "CurrentAction should be updated to the requested action"
    );

    // Complete the action
    world.commands().trigger(OnActionEnded::completed(actor, action_id));
    world.flush();

    // Verify action was removed
    assert!(
        world.get::<TestAction>(actor).is_none(),
        "TestAction component should be removed after completion"
    );
}

/// Test action cancellation when switching actions
#[test]
fn test_action_cancellation() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    // Create custom observers that check the action component id
    app.add_observer(
        |trigger: On<OnActionInitiated>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            let action1_id = world.component_id::<Action1>().unwrap();
            if action == action1_id {
                commands.entity(actor).insert(Action1::default());
            }
        },
    );
    app.add_observer(
        |trigger: On<OnActionInitiated>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            let action2_id = world.component_id::<Action2>().unwrap();
            if action == action2_id {
                commands.entity(actor).insert(Action2::default());
            }
        },
    );
    app.add_observer(
        |trigger: On<OnActionEnded>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            let action1_id = world.component_id::<Action1>().unwrap();
            if action == action1_id {
                commands.entity(actor).remove::<Action1>();
            }
        },
    );
    app.add_observer(
        |trigger: On<OnActionEnded>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            let action2_id = world.component_id::<Action2>().unwrap();
            if action == action2_id {
                commands.entity(actor).remove::<Action2>();
            }
        },
    );

    let world = app.world_mut();

    let action1_id = world.register_component::<Action1>();
    let action2_id = world.register_component::<Action2>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let actor = commands.spawn((Picker::new(idle_action), CurrentAction(idle_action))).id();
    world.flush();

    // Start action1
    world.commands().trigger(RequestAction::specific(actor, action1_id));
    world.flush();

    assert!(world.get::<Action1>(actor).is_some(), "Action1 should be active");
    assert_eq!(action1_id, world.get::<CurrentAction>(actor).unwrap().0);

    // Switch to action2 (should cancel action1)
    world.commands().trigger(RequestAction::specific(actor, action2_id));
    world.flush();

    assert!(world.get::<Action1>(actor).is_none(), "Action1 should be removed/cancelled");
    assert!(world.get::<Action2>(actor).is_some(), "Action2 should now be active");
    assert_eq!(
        action2_id,
        world.get::<CurrentAction>(actor).unwrap().0,
        "CurrentAction should be updated to action2"
    );
}

/// Test the complete utility AI lifecycle: score -> pick -> act
#[test]
fn test_complete_utility_ai_lifecycle() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    app.add_observer(on_action_initiated_insert_default::<DrinkAction>);
    app.add_observer(on_action_ended_remove::<DrinkAction>);

    let world = app.world_mut();

    let drink_action = world.register_component::<DrinkAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();

    // Create a thirst scorer
    let thirst_scorer = commands.spawn((Score::new(0.8), ThirstMarker)).id();

    // Create an actor with a picker
    let actor = commands
        .spawn((
            Picker::new(idle_action).with(thirst_scorer, drink_action),
            FirstToScore::new(0.5),
            CurrentAction(idle_action),
        ))
        .add_child(thirst_scorer)
        .id();
    world.flush();

    // Run the complete lifecycle
    // 1. Score
    world.commands().trigger(RunScoring::entity(thirst_scorer));
    world.flush();

    // Verify scoring worked
    assert_eq!(0.8, world.get::<Score>(thirst_scorer).unwrap().get());

    // 2. Pick
    world.commands().trigger(RunPicking::entity(actor));
    world.flush();

    // Verify picking worked - should pick drink_action since score (0.8) > threshold (0.5)
    assert_eq!(drink_action, world.get::<Picker>(actor).unwrap().picked);

    // 3. Act - request the picked action
    world.commands().trigger(RequestAction::picked(actor));
    world.flush();

    // Verify action was initiated
    assert!(world.get::<DrinkAction>(actor).is_some(), "DrinkAction should be active");
    assert_eq!(drink_action, world.get::<CurrentAction>(actor).unwrap().0);
}

/// Test that OnScore events are triggered with correct entity information
#[test]
fn test_on_score_event_contains_entity() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    // Track which entities received OnScore events
    #[derive(Resource, Default)]
    struct ScoredEntities(Vec<Entity>);

    app.insert_resource(ScoredEntities::default());

    // Add an observer to capture OnScore events
    app.add_observer(
        |trigger: On<OnScore>, mut scored: ResMut<ScoredEntities>| {
            scored.0.push(trigger.event().entity);
        },
    );

    let world = app.world_mut();

    let mut commands = world.commands();
    let scorer = commands.spawn((Score::default(), FixedScore::new(0.5))).id();
    world.flush();

    // Trigger scoring
    world.commands().trigger(RunScoring::entity(scorer));
    world.flush();

    // Verify the event was triggered with the correct entity
    let scored = world.resource::<ScoredEntities>();
    assert!(
        scored.0.contains(&scorer),
        "OnScore event should have been triggered with the scorer entity"
    );
}

/// Test that OnPicked events contain correct entity and action information
#[test]
fn test_on_picked_event_contains_entity_and_action() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    #[derive(Resource, Default)]
    struct PickedActions(Vec<(Entity, ComponentId)>);

    app.insert_resource(PickedActions::default());

    // Add an observer to capture OnPicked events
    app.add_observer(
        |trigger: On<OnPicked>, mut picked: ResMut<PickedActions>| {
            picked.0.push((trigger.event().entity, trigger.event().action));
        },
    );

    let world = app.world_mut();

    let my_action = world.register_component::<MyAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let scorer = commands.spawn((Score::new(0.8), MyMarker)).id();
    let actor = commands
        .spawn((
            Picker::new(idle_action).with(scorer, my_action),
            FirstToScore::new(0.5),
        ))
        .add_child(scorer)
        .id();
    world.flush();

    // Run picking
    world.commands().trigger(RunPicking::entity(actor));
    world.flush();

    // Verify the OnPicked event was triggered with correct entity and action
    let picked = world.resource::<PickedActions>();
    assert_eq!(1, picked.0.len(), "Should have one picked action");
    assert_eq!(actor, picked.0[0].0, "Entity should match the actor");
    assert_eq!(my_action, picked.0[0].1, "Action should be my_action");
}

/// Test that OnActionInitiated events contain correct entity and action information
#[test]
fn test_on_action_initiated_event() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    #[derive(Resource, Default)]
    struct InitiatedActions(Vec<(Entity, ComponentId)>);

    app.insert_resource(InitiatedActions::default());

    // Add an observer to capture OnActionInitiated events
    app.add_observer(
        |trigger: On<OnActionInitiated>, mut initiated: ResMut<InitiatedActions>| {
            initiated.0.push((trigger.event().entity, trigger.event().action));
        },
    );

    let world = app.world_mut();

    let my_action = world.register_component::<MyAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let actor = commands.spawn((Picker::new(idle_action), CurrentAction(idle_action))).id();
    world.flush();

    // Request an action
    world.commands().trigger(RequestAction::specific(actor, my_action));
    world.flush();

    // Verify the OnActionInitiated event was triggered
    let initiated = world.resource::<InitiatedActions>();
    assert_eq!(1, initiated.0.len(), "Should have one initiated action");
    assert_eq!(actor, initiated.0[0].0, "Entity should match the actor");
    assert_eq!(my_action, initiated.0[0].1, "Action should be my_action");
}

/// Test that OnActionEnded events contain correct entity, action, and reason information
#[test]
fn test_on_action_ended_event() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    #[derive(Resource, Default)]
    struct EndedActions(Vec<(Entity, ComponentId, ActionEndReason)>);

    app.insert_resource(EndedActions::default());

    // Add an observer to capture OnActionEnded events
    app.add_observer(
        |trigger: On<OnActionEnded>, mut ended: ResMut<EndedActions>| {
            ended
                .0
                .push((trigger.event().entity, trigger.event().action, trigger.event().reason));
        },
    );

    app.add_observer(on_action_initiated_insert_default::<TestAction>);
    app.add_observer(on_action_ended_remove::<TestAction>);

    let world = app.world_mut();

    let my_action = world.register_component::<TestAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let actor = commands.spawn((Picker::new(idle_action), CurrentAction(idle_action))).id();
    world.flush();

    // Start an action
    world.commands().trigger(RequestAction::specific(actor, my_action));
    world.flush();

    // Complete the action
    world.commands().trigger(OnActionEnded::completed(actor, my_action));
    world.flush();

    // Verify the OnActionEnded event was triggered with Completed reason
    let ended = world.resource::<EndedActions>();
    // Filter to just the my_action events with Completed reason
    let my_action_completed: Vec<_> = ended
        .0
        .iter()
        .filter(|(_, a, r)| *a == my_action && *r == ActionEndReason::Completed)
        .collect();

    assert!(
        !my_action_completed.is_empty(),
        "Should have at least one completed my_action event"
    );
    assert_eq!(actor, my_action_completed[0].0, "Entity should match the actor");
    assert_eq!(my_action, my_action_completed[0].1, "Action should be my_action");
    assert_eq!(
        ActionEndReason::Completed,
        my_action_completed[0].2,
        "Reason should be Completed"
    );
}

/// Test action cancellation event
#[test]
fn test_on_action_cancelled_event() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    #[derive(Resource, Default)]
    struct EndedActions(Vec<(Entity, ComponentId, ActionEndReason)>);

    app.insert_resource(EndedActions::default());

    app.add_observer(
        |trigger: On<OnActionEnded>, mut ended: ResMut<EndedActions>| {
            ended
                .0
                .push((trigger.event().entity, trigger.event().action, trigger.event().reason));
        },
    );

    app.add_observer(on_action_initiated_insert_default::<Action1>);
    app.add_observer(on_action_initiated_insert_default::<Action2>);
    app.add_observer(on_action_ended_remove::<Action1>);
    app.add_observer(on_action_ended_remove::<Action2>);

    let world = app.world_mut();

    let action1 = world.register_component::<Action1>();
    let action2 = world.register_component::<Action2>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let actor = commands.spawn((Picker::new(idle_action), CurrentAction(idle_action))).id();
    world.flush();

    // Start action1
    world.commands().trigger(RequestAction::specific(actor, action1));
    world.flush();

    // Switch to action2 (cancels action1)
    world.commands().trigger(RequestAction::specific(actor, action2));
    world.flush();

    // Verify action1 was cancelled
    let ended = world.resource::<EndedActions>();
    assert!(
        ended.0.iter().any(|(e, a, r)| *e == actor && *a == action1 && *r == ActionEndReason::Cancelled),
        "Action1 should have been cancelled"
    );
}

/// Test real-time lifecycle automation
/// This test verifies that the RealTime plugin is properly configured and can execute
/// the complete utility AI lifecycle (score -> pick -> act) when triggered manually.
#[test]
fn test_realtime_lifecycle() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::RealTime);

    // Add custom observers that check the action component id
    app.add_observer(
        |trigger: On<OnActionInitiated>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            if let Some(drink_action_id) = world.component_id::<DrinkAction>() {
                if action == drink_action_id {
                    commands.entity(actor).insert(DrinkAction::default());
                }
            }
        },
    );
    app.add_observer(
        |trigger: On<OnActionEnded>, mut commands: Commands, world: &World| {
            let actor = trigger.event().entity;
            let action = trigger.event().action;
            if let Some(drink_action_id) = world.component_id::<DrinkAction>() {
                if action == drink_action_id {
                    commands.entity(actor).remove::<DrinkAction>();
                }
            }
        },
    );

    let world = app.world_mut();

    let drink_action = world.register_component::<DrinkAction>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();
    let thirst_scorer = commands.spawn((Score::default(), FixedScore::new(0.9), ThirstMarker)).id();
    let actor = commands
        .spawn((
            Picker::new(idle_action).with(thirst_scorer, drink_action),
            FirstToScore::new(0.5),
            CurrentAction(idle_action),
        ))
        .add_child(thirst_scorer)
        .id();
    world.flush();

    // Manually trigger the lifecycle to test it works
    // (In real-time mode, this would happen automatically in FixedPostUpdate)
    world.commands().trigger(RunScoring::all());
    world.commands().trigger(RunPicking::all());
    world.flush();

    // Check the score was updated
    let score = world.get::<Score>(thirst_scorer).unwrap().get();
    assert_relative_eq!(0.9, score, epsilon = 0.0001);

    // Check the picker picked the correct action
    let picked = world.get::<Picker>(actor).unwrap().picked;
    assert_eq!(
        drink_action, picked,
        "Picker should have picked drink_action (score=0.9 > threshold=0.5)"
    );

    // Request the picked action
    world.commands().trigger(RequestAction::picked(actor));
    world.flush();

    // Verify the action was initiated
    let current_action = world.get::<CurrentAction>(actor).unwrap().0;
    assert_eq!(
        drink_action, current_action,
        "Actor should have switched to drinking action"
    );
    assert!(world.get::<DrinkAction>(actor).is_some(), "DrinkAction should be active");
}

/// Test multiple actors with different scores and actions
#[test]
fn test_multiple_actors() {
    let mut app = App::new();
    app.add_plugins(ObservedUtilityPlugins::TurnBased);

    app.add_observer(on_action_initiated_insert_default::<Action1>);
    app.add_observer(on_action_initiated_insert_default::<Action2>);

    let world = app.world_mut();

    let action1 = world.register_component::<Action1>();
    let action2 = world.register_component::<Action2>();
    let idle_action = world.register_component::<IdleAction>();

    let mut commands = world.commands();

    // Actor 1: High score, should pick action1
    let scorer1 = commands.spawn((Score::new(0.8), MyMarker)).id();
    let actor1 = commands
        .spawn((
            Picker::new(idle_action).with(scorer1, action1),
            FirstToScore::new(0.5),
            CurrentAction(idle_action),
        ))
        .add_child(scorer1)
        .id();

    // Actor 2: Low score, should stay idle
    let scorer2 = commands.spawn((Score::new(0.2), MyMarker)).id();
    let actor2 = commands
        .spawn((
            Picker::new(idle_action).with(scorer2, action2),
            FirstToScore::new(0.5),
            CurrentAction(idle_action),
        ))
        .add_child(scorer2)
        .id();

    world.flush();

    // Run scoring and picking for all
    world.commands().trigger(RunScoring::all());
    world.commands().trigger(RunPicking::all());
    world.flush();

    // Request actions for both actors
    world.commands().trigger(RequestAction::picked(actor1));
    world.commands().trigger(RequestAction::picked(actor2));
    world.flush();

    // Verify actor1 picked action1
    assert_eq!(action1, world.get::<CurrentAction>(actor1).unwrap().0);
    assert!(world.get::<Action1>(actor1).is_some());

    // Verify actor2 stayed idle (score too low)
    assert_eq!(idle_action, world.get::<CurrentAction>(actor2).unwrap().0);
    assert!(world.get::<Action2>(actor2).is_none());
}

// Helper components for tests
#[derive(Component)]
struct MyAction;

#[derive(Component)]
struct IdleAction;

#[derive(Component, Default)]
struct TestAction;

#[derive(Component, Default)]
struct Action1;

#[derive(Component, Default)]
struct Action2;

#[derive(Component, Default)]
struct DrinkAction;

#[derive(Component)]
struct MyMarker;

#[derive(Component)]
struct ThirstMarker;
