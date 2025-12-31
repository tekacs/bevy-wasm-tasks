use crate::{TaskContext, Tasks};
use bevy_ecs::{
    prelude::World,
    system::{Commands, ResMut, SystemName, SystemParam, SystemState},
};
use std::{
    collections::HashMap,
    future::Future,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub enum Run {
    AsOftenAsPossible,
    MaxRate(Duration),
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
    last_start: Option<Instant>,
}

#[derive(Default, bevy_ecs::resource::Resource)]
pub(crate) struct AsyncSystems {
    states: HashMap<String, AsyncState>,
}

/// Command-based scheduler for async "systems".
///
/// This defers SystemParam acquisition + setup work to the end of the current schedule,
/// then spawns the returned `'static` future onto the background runtime.
///
/// Keying is based on the current system name (`SystemName`), which is assumed to be unique
/// for each system that calls into this API.
#[derive(SystemParam)]
pub struct Scheduler<'w, 's> {
    #[allow(dead_code)]
    tasks: Tasks<'w>,
    commands: Commands<'w, 's>,
    async_systems: ResMut<'w, AsyncSystems>,
    system_name: SystemName<'s>,
}

impl<'w, 's> Scheduler<'w, 's> {
    pub fn async_system<P, F, Fut>(&mut self, run: Run, f: F)
    where
        P: SystemParam + 'static,
        for<'pw, 'ps> F: FnOnce(TaskContext, P::Item<'pw, 'ps>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let key = self.system_name.name().to_string();
        let state = self.async_systems.states.entry(key.clone()).or_default();

        match run {
            Run::AsOftenAsPossible => {
                if state.in_flight {
                    return;
                }
                state.in_flight = true;
            }
            Run::MaxRate(period) => {
                if state.in_flight {
                    return;
                }
                if let Some(last_start) = state.last_start {
                    if last_start.elapsed() < period {
                        return;
                    }
                }
                state.in_flight = true;
                state.last_start = Some(Instant::now());
            }
            Run::OnChange { triggered } => {
                if triggered {
                    state.pending = true;
                }
                if state.in_flight {
                    return;
                }
                if !state.pending {
                    return;
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

            let completion_key = key.clone();
            let completion_run = run;

            let mut state = SystemState::<Tasks>::new(world);
            let tasks = state.get(world);
            let task_context = tasks.task_context();
            let _handle = tasks.spawn_auto(move |_| async move {
                user_future.await;
                task_context
                    .run_on_main_thread(move |mt| {
                        let mut systems = mt.world.resource_mut::<AsyncSystems>();
                        let state = systems.states.entry(completion_key).or_default();
                        state.in_flight = false;
                        let _ = completion_run;
                    })
                    .await;
            });
            state.apply(world);
        });
    }
}
