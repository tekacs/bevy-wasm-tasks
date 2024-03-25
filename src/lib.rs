use bevy_app::{App, Plugin, Update};
use bevy_ecs::{
    prelude::World,
    system::{Res, SystemParam},
};
use context::task::TaskContext;
use futures_util::FutureExt;
use join::JoinHandle;
use runtime::{Runtime, TasksRuntime};
use std::{future::Future, sync::Arc};
use task_channels::TaskChannels;
use ticks::{TicksPlugin, UpdateTicks};

pub mod context;
pub mod join;
pub mod runtime;
pub mod task_channels;
pub mod ticks;

#[cfg(all(feature = "wasm", feature = "tokio"))]
compile_error!(
    "The `wasm` and `tokio` features are mutually exclusive. Please enable only one of them."
);

#[derive(SystemParam)]
pub struct Tasks<'w> {
    runtime: Res<'w, TasksRuntime>,
    task_channels: Res<'w, TaskChannels>,
    ticks: Res<'w, UpdateTicks>,
}

impl<'w> Tasks<'w> {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn runtime_arc(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    #[inline(always)]
    pub fn task_context(&self) -> TaskContext {
        TaskContext {
            tick_rx: self.ticks.tick_rx(),
            task_tx: self.task_channels.task_tx(),
            ticks: self.ticks.ticks(),
        }
    }

    /// Spawn a task which will run using futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    #[cfg(feature = "tokio")]
    pub fn spawn_background_task<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + Send + 'static,
    {
        let context = self.task_context();
        let future = spawnable_task(context);
        let (future, handle) = future.remote_handle();
        self.runtime.0.spawn(future);
        JoinHandle::new(handle)
    }

    /// Spawn a task which will run using futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    #[cfg(all(feature = "wasm", not(feature = "tokio")))]
    pub fn spawn_background_task<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        let context = self.task_context();
        let future = spawnable_task(context);
        let (future, handle) = future.remote_handle();
        wasm_bindgen_futures::spawn_local(future);
        JoinHandle::new(handle)
    }
}

/// The Bevy [`Plugin`] which sets up the [`TasksRuntime`] Bevy resource and registers
/// the [`tick_runtime_update`] exclusive system.
pub struct TasksPlugin {
    /// Callback which is used to create a Tokio runtime when the plugin is installed. The
    /// default value for this field configures a multi-threaded [`Runtime`] with IO and timer
    /// functionality enabled if building for non-wasm32 architectures. On wasm32 the current-thread
    /// scheduler is used instead.
    make_runtime: Box<dyn Fn() -> Arc<Runtime> + Send + Sync + 'static>,
}

impl Default for TasksPlugin {
    /// Configures the plugin to build a new Tokio [`Runtime`] with both IO and timer functionality
    /// enabled. On the wasm32 architecture, the [`Runtime`] will be the current-thread runtime, on all other
    /// architectures the [`Runtime`] will be the multi-thread runtime.
    fn default() -> Self {
        Self {
            make_runtime: Box::new(|| Arc::new(Runtime::default())),
        }
    }
}

impl TasksPlugin {
    /// The Bevy exclusive system which executes the main thread callbacks that background
    /// tasks requested using [`run_on_main_thread`](TaskContext::run_on_main_thread). You
    /// can control which [`CoreStage`] this system executes in by specifying a custom
    /// [`tick_stage`](TasksPlugin::tick_stage) value.
    pub fn run_tasks(world: &mut World) {
        let current_tick = world.get_resource::<UpdateTicks>().unwrap().tick();
        world.resource_scope::<TaskChannels, _>(|world, mut task_channels| {
            task_channels.execute_main_thread_work(world, current_tick);
        });
    }
}

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicksPlugin)
            .init_resource::<TaskChannels>()
            .insert_resource(TasksRuntime::new((self.make_runtime)()))
            .add_systems(Update, Self::run_tasks);
    }
}
