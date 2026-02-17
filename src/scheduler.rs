use crate::{TaskContext, Tasks};
use bevy_ecs::{
    error::BevyError,
    prelude::World,
    system::{Commands, ResMut, SystemName, SystemParam, SystemState},
};
use futures_util::FutureExt;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    future::Future,
    panic::AssertUnwindSafe,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub enum Run {
    AsOftenAsPossible,
    MaxRate(Duration),
    /// Run exactly once for this callsite and never schedule again.
    Once,
    /// Run continuously as a daemon.
    ///
    /// If the task returns `Ok(())` or `Err(_)`, it is restarted on the next schedule tick.
    /// Exits are logged to stderr.
    Daemon,
    /// Change-triggered scheduling.
    ///
    /// If `triggered` is true, the async work is scheduled to run as soon as possible.
    /// If the job is currently in-flight, this schedules exactly one follow-up run after it
    /// completes (no double-scheduling). If `triggered` later becomes false, the follow-up run
    /// remains scheduled.
    OnChange {
        triggered: bool,
    },
}

#[derive(Default)]
struct AsyncState {
    in_flight: bool,
    pending: bool,
    ran_once: bool,
    last_start: Option<Instant>,
    last_error: Option<BevyError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AsyncSystemKey {
    system_name: String,
    closure_type_id: TypeId,
}

#[derive(Default, bevy_ecs::resource::Resource)]
pub(crate) struct AsyncSystems {
    states: HashMap<AsyncSystemKey, AsyncState>,
}

/// Command-based scheduler for async "systems".
///
/// This defers SystemParam acquisition + setup work to the end of the current schedule,
/// then spawns the returned `'static` future onto the background runtime.
///
/// Keying combines the current system name (`SystemName`) with the closure type used at the
/// callsite. `SystemName` alone is not a reliable unique identifier in Bevy 0.18.
#[derive(SystemParam)]
pub struct Scheduler<'w, 's> {
    #[allow(dead_code)]
    tasks: Tasks<'w>,
    commands: Commands<'w, 's>,
    async_systems: ResMut<'w, AsyncSystems>,
    system_name: SystemName,
}

impl<'w, 's> Scheduler<'w, 's> {
    pub fn async_system<P, F, Fut>(
        &mut self,
        run: Run,
        f: F,
    ) -> bevy_ecs::error::Result<(), BevyError>
    where
        P: SystemParam + 'static,
        for<'pw, 'ps> F: FnOnce(TaskContext, P::Item<'pw, 'ps>) -> Fut + Send + 'static,
        Fut: Future<Output = bevy_ecs::error::Result<(), BevyError>> + Send + 'static,
    {
        let key = AsyncSystemKey {
            system_name: self.system_name.name().to_string(),
            closure_type_id: TypeId::of::<F>(),
        };
        let state = self.async_systems.states.entry(key.clone()).or_default();

        if let Some(err) = state.last_error.take() {
            return Err(err);
        }

        match run {
            Run::AsOftenAsPossible => {
                if state.in_flight {
                    return Ok(());
                }
                state.in_flight = true;
            }
            Run::MaxRate(period) => {
                if state.in_flight {
                    return Ok(());
                }
                if let Some(last_start) = state.last_start {
                    if last_start.elapsed() < period {
                        return Ok(());
                    }
                }
                state.in_flight = true;
                state.last_start = Some(Instant::now());
            }
            Run::Once => {
                if state.ran_once {
                    return Ok(());
                }
                if state.in_flight {
                    return Ok(());
                }
                state.in_flight = true;
                state.ran_once = true;
            }
            Run::Daemon => {
                if state.in_flight {
                    return Ok(());
                }
                state.in_flight = true;
            }
            Run::OnChange { triggered } => {
                if triggered {
                    state.pending = true;
                }
                if state.in_flight {
                    return Ok(());
                }
                if !state.pending {
                    return Ok(());
                }
                state.in_flight = true;
                state.pending = false;
            }
        }

        self.commands.queue(move |world: &mut World| {
            let user_future = {
                let mut state = SystemState::<P>::new(world);
                let fut = {
                    let ctx = world.resource::<TaskContext>().clone();
                    let params = state.get_mut(world);
                    f(ctx, params)
                };
                state.apply(world);
                fut
            };

            let completion_key = key;
            let completion_system_name = completion_key.system_name.clone();
            let completion_run = run;

            let mut state = SystemState::<Tasks>::new(world);
            let tasks = state.get(world);
            let task_context = tasks.task_context();
            let _handle = tasks.spawn_auto(move |_| async move {
                let daemon_mode = matches!(completion_run, Run::Daemon);
                let result = if daemon_mode {
                    match AssertUnwindSafe(user_future).catch_unwind().await {
                        Ok(result) => Ok(result),
                        Err(payload) => Err(DaemonCompletion::Panicked(payload_to_string(payload))),
                    }
                } else {
                    Ok(user_future.await)
                };
                if daemon_mode {
                    task_context.sleep_updates(1).await;
                }
                task_context
                    .run_on_main_thread(move |mt| {
                        let mut systems = mt.world.resource_mut::<AsyncSystems>();
                        let state = systems.states.entry(completion_key).or_default();
                        state.in_flight = false;
                        match completion_run {
                            Run::Daemon => match result {
                                Ok(Ok(())) => eprintln!(
                                    "[bevy-wasm-tasks] async daemon '{}' exited cleanly; restarting",
                                    completion_system_name
                                ),
                                Ok(Err(err)) => eprintln!(
                                    "[bevy-wasm-tasks] async daemon '{}' exited with error: {err}; restarting",
                                    completion_system_name
                                ),
                                Err(DaemonCompletion::Panicked(message)) => eprintln!(
                                    "[bevy-wasm-tasks] async daemon '{}' panicked: {}; restarting",
                                    completion_system_name, message
                                ),
                            },
                            _ => {
                                if let Ok(Err(err)) = result {
                                    state.last_error = Some(err);
                                }
                            }
                        }
                    })
                    .await;
            });
            state.apply(world);
        });
        Ok(())
    }
}

fn payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

enum DaemonCompletion {
    Panicked(String),
}
