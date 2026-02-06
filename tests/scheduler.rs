#![cfg(feature = "tokio")]

use bevy_app::{App, Update};
use bevy_ecs::{
    error::BevyError, prelude::Resource, schedule::IntoScheduleConfigs, system::ResMut,
};
use bevy_wasm_tasks::{Run, Scheduler, TasksPlugin};
use std::time::Duration;

#[derive(Resource, Default, Debug)]
struct ProbeCounts {
    a: usize,
    b: usize,
}

fn probe_a(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system::<(), _, _>(Run::AsOftenAsPossible, |ctx, _| async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        ctx.run_on_main_thread(|mut mt| {
            mt.run::<ResMut<ProbeCounts>, _, _>(|mut counts| {
                counts.a += 1;
            });
        })
        .await;
        Ok(())
    })
}

fn probe_b(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system::<(), _, _>(Run::AsOftenAsPossible, |ctx, _| async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        ctx.run_on_main_thread(|mut mt| {
            mt.run::<ResMut<ProbeCounts>, _, _>(|mut counts| {
                counts.b += 1;
            });
        })
        .await;
        Ok(())
    })
}

fn pump_updates(app: &mut App, iterations: usize) {
    for _ in 0..iterations {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn scheduler_runs_as_often_as_possible_repeatedly() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<ProbeCounts>();
    app.add_systems(Update, probe_a);

    pump_updates(&mut app, 80);

    let counts = app.world().resource::<ProbeCounts>();
    assert!(
        counts.a >= 3,
        "expected scheduler probe to run multiple times, got a={}",
        counts.a
    );
}

#[test]
fn scheduler_runs_multiple_async_systems_in_chain() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<ProbeCounts>();
    app.add_systems(Update, (probe_a, probe_b).chain());

    pump_updates(&mut app, 100);

    let counts = app.world().resource::<ProbeCounts>();
    assert!(
        counts.a >= 3,
        "probe_a should have run multiple times, got a={}",
        counts.a
    );
    assert!(
        counts.b >= 3,
        "probe_b should have run multiple times, got b={}",
        counts.b
    );
}

#[test]
fn scheduler_runs_multiple_async_systems_without_chain() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<ProbeCounts>();
    app.add_systems(Update, (probe_a, probe_b));

    pump_updates(&mut app, 100);

    let counts = app.world().resource::<ProbeCounts>();
    assert!(
        counts.a >= 3,
        "probe_a should have run multiple times, got a={}",
        counts.a
    );
    assert!(
        counts.b >= 3,
        "probe_b should have run multiple times, got b={}",
        counts.b
    );
}
