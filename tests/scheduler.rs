#![cfg(feature = "tokio")]

use bevy_app::{App, Update};
use bevy_ecs::{
    error::BevyError, prelude::Resource, schedule::IntoScheduleConfigs, system::ResMut,
};
use bevy_wasm_tasks::{Run, Scheduler, TaskContext, TasksPlugin};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Resource, Default, Debug)]
struct ProbeCounts {
    a: usize,
    b: usize,
    once: usize,
    daemon: usize,
}

fn probe_a(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system(Run::AsOftenAsPossible, |ctx: TaskContext| async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        ctx.run(|mut counts: ResMut<ProbeCounts>| {
            counts.a += 1;
        })
        .await;
        Ok(())
    })
}

fn probe_b(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system(Run::AsOftenAsPossible, |ctx: TaskContext| async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        ctx.run(|mut counts: ResMut<ProbeCounts>| {
            counts.b += 1;
        })
        .await;
        Ok(())
    })
}

fn probe_once(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system(Run::Once, |ctx: TaskContext| async move {
        ctx.run(|mut counts: ResMut<ProbeCounts>| {
            counts.once += 1;
        })
        .await;
        Ok(())
    })
}

fn probe_daemon(mut scheduler: Scheduler) -> Result<(), BevyError> {
    scheduler.async_system(Run::Daemon, |ctx: TaskContext| async move {
        ctx.run(|mut counts: ResMut<ProbeCounts>| {
            counts.daemon += 1;
        })
        .await;
        Ok(())
    })
}

#[derive(Default)]
struct PanicProbe {
    attempts: AtomicUsize,
    completed: AtomicUsize,
    errors: AtomicUsize,
}

#[derive(Resource, Default)]
struct PanicProbes {
    as_often: Arc<PanicProbe>,
    max_rate: Arc<PanicProbe>,
    once: Arc<PanicProbe>,
    daemon: Arc<PanicProbe>,
    on_change: Arc<PanicProbe>,
    constructor: Arc<PanicProbe>,
}

fn schedule_poll_panic(scheduler: &mut Scheduler<'_, '_>, run: Run, probe: Arc<PanicProbe>) {
    let task_probe = probe.clone();
    let result = scheduler.async_system(run, move || async move {
        let attempt = task_probe.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            panic!("poll panic probe");
        }
        task_probe.completed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    if result.is_err() {
        probe.errors.fetch_add(1, Ordering::SeqCst);
    }
}

fn panic_as_often(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    schedule_poll_panic(
        &mut scheduler,
        Run::AsOftenAsPossible,
        probes.as_often.clone(),
    );
}

fn panic_max_rate(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    schedule_poll_panic(
        &mut scheduler,
        Run::MaxRate(Duration::ZERO),
        probes.max_rate.clone(),
    );
}

fn panic_once(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    schedule_poll_panic(&mut scheduler, Run::Once, probes.once.clone());
}

fn panic_daemon(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    schedule_poll_panic(&mut scheduler, Run::Daemon, probes.daemon.clone());
}

fn panic_on_change(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    schedule_poll_panic(
        &mut scheduler,
        Run::OnChange { triggered: true },
        probes.on_change.clone(),
    );
}

fn panic_constructor(mut scheduler: Scheduler, probes: bevy_ecs::system::Res<PanicProbes>) {
    let probe = probes.constructor.clone();
    let task_probe = probe.clone();
    let result = scheduler.async_system(Run::AsOftenAsPossible, move || {
        let attempt = task_probe.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            panic!("constructor panic probe");
        }
        async move {
            task_probe.completed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    });
    if result.is_err() {
        probe.errors.fetch_add(1, Ordering::SeqCst);
    }
}

fn pump_updates(app: &mut App, iterations: usize) {
    for _ in 0..iterations {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_recovered(probe: &PanicProbe) {
    assert!(probe.attempts.load(Ordering::SeqCst) >= 2);
    assert!(probe.completed.load(Ordering::SeqCst) >= 1);
    assert_eq!(probe.errors.load(Ordering::SeqCst), 1);
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

#[test]
fn scheduler_run_once_executes_exactly_once() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<ProbeCounts>();
    app.add_systems(Update, probe_once);

    pump_updates(&mut app, 40);

    let counts = app.world().resource::<ProbeCounts>();
    assert_eq!(counts.once, 1, "Run::Once should execute exactly once");
}

#[test]
fn scheduler_daemon_restarts_after_exit() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<ProbeCounts>();
    app.add_systems(Update, probe_daemon);

    pump_updates(&mut app, 40);

    let counts = app.world().resource::<ProbeCounts>();
    assert!(
        counts.daemon >= 3,
        "Run::Daemon should restart repeatedly, got daemon={}",
        counts.daemon
    );
}

#[test]
fn scheduler_as_often_recovers_after_panic() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_as_often);

    pump_updates(&mut app, 80);

    let probes = app.world().resource::<PanicProbes>();
    assert_recovered(&probes.as_often);
}

#[test]
fn scheduler_max_rate_recovers_after_panic() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_max_rate);

    pump_updates(&mut app, 80);

    assert_recovered(&app.world().resource::<PanicProbes>().max_rate);
}

#[test]
fn scheduler_on_change_recovers_after_panic() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_on_change);

    pump_updates(&mut app, 80);

    assert_recovered(&app.world().resource::<PanicProbes>().on_change);
}

#[test]
fn scheduler_constructor_recovers_after_panic() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_constructor);

    pump_updates(&mut app, 80);

    assert_recovered(&app.world().resource::<PanicProbes>().constructor);
}

#[test]
fn scheduler_daemon_restarts_after_panic() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_daemon);

    pump_updates(&mut app, 80);

    let probes = app.world().resource::<PanicProbes>();
    assert!(probes.daemon.attempts.load(Ordering::SeqCst) >= 2);
    assert!(probes.daemon.completed.load(Ordering::SeqCst) >= 1);
    assert_eq!(probes.daemon.errors.load(Ordering::SeqCst), 0);
}

#[test]
fn scheduler_once_surfaces_panic_without_rerunning() {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app.init_resource::<PanicProbes>();
    app.add_systems(Update, panic_once);

    pump_updates(&mut app, 80);

    let probes = app.world().resource::<PanicProbes>();
    assert_eq!(probes.once.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probes.once.completed.load(Ordering::SeqCst), 0);
    assert_eq!(probes.once.errors.load(Ordering::SeqCst), 1);
}
