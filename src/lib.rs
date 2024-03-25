use bevy_app::{App, Plugin, Update};
use bevy_ecs::{
    prelude::World,
    system::{Res, SystemParam},
};
use std::future::Future;
use task_channels::TaskChannels;
use ticks::{TicksPlugin, UpdateTicks};

pub use context::task::TaskContext;
pub use join::JoinHandle;
pub use runtime::Runtime;

pub mod context;
pub mod join;
pub mod runtime;
pub mod task_channels;
pub mod ticks;

#[derive(SystemParam)]
pub struct Tasks<'w> {
    runtime: Res<'w, Runtime>,
    task_channels: Res<'w, TaskChannels>,
    ticks: Res<'w, UpdateTicks>,
}

impl<'w> Tasks<'w> {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
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
    pub fn spawn_tokio<Task, Output, Spawnable>(
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
        let handle = self.runtime.0.spawn(future);
        JoinHandle::Tokio(handle)
    }

    #[cfg(not(feature = "tokio"))]
    pub fn spawn_tokio<Task, Output, Spawnable>(
        &self,
        _spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + Send + 'static,
    {
        panic!("Tokio runtime is not enabled. Enable the `tokio` feature to use Tokio.");
    }

    /// Spawn a task which will run using futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    #[cfg(feature = "wasm")]
    pub fn spawn_wasm<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        use futures_util::FutureExt;
        let context = self.task_context();
        let future = spawnable_task(context);
        let (future, handle) = future.remote_handle();
        wasm_bindgen_futures::spawn_local(future);
        JoinHandle::RemoteHandle(Some(handle))
    }

    #[cfg(not(feature = "wasm"))]
    pub fn spawn_wasm<Task, Output, Spawnable>(
        &self,
        _spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        panic!("Wasm runtime is not enabled. Enable the `wasm` feature to use Wasm.");
    }

    pub fn spawn_auto<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + Send + 'static,
    {
        if cfg!(feature = "tokio") {
            self.spawn_tokio(spawnable_task)
        } else if cfg!(feature = "wasm") {
            self.spawn_wasm(spawnable_task)
        } else {
            panic!("No runtime is enabled. Enable the `tokio` or `wasm` feature to use a runtime.");
        }
    }
}

/// The Bevy [`Plugin`] which sets up the [`Runtime`] Bevy resource and registers
/// the [`tick_runtime_update`] exclusive system.
pub struct TasksPlugin {
    /// Callback which is used to create a Tokio runtime when the plugin is installed. The
    /// default value for this field configures a multi-threaded [`Runtime`] with IO and timer
    /// functionality enabled if building for non-wasm32 architectures. On wasm32 the current-thread
    /// scheduler is used instead.
    make_runtime: Box<dyn Fn() -> Runtime + Send + Sync + 'static>,
}

impl Default for TasksPlugin {
    /// Configures the plugin to build a new Tokio [`Runtime`] with both IO and timer functionality
    /// enabled. On the wasm32 architecture, the [`Runtime`] will be the current-thread runtime, on all other
    /// architectures the [`Runtime`] will be the multi-thread runtime.
    fn default() -> Self {
        Self {
            make_runtime: Box::new(Runtime::default),
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
            .insert_resource((self.make_runtime)())
            .add_systems(Update, Self::run_tasks);
    }
}
